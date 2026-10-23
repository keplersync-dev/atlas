//! The Atlas server client. Holds the session: the refresh token (Credential Manager), the
//! access token (memory only) and the signed-in user (cached in `setting` so the account
//! still shows while offline). The UI never talks to the server itself.

pub mod files;
pub mod loopback;
pub mod realtime;
pub mod tokens;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use atlas_protocol::api::{
    AuthResult, ErrorBody, ErrorCode as WireCode, ExchangeRequest, Me, Tokens,
};
use atlas_protocol::pkce;
use atlas_protocol::ws::PresenceUpdate;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::SqlitePool;
use tokio::sync::{oneshot, watch};
use url::Url;

use crate::db;
use crate::dto::Session;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::AtlasEvent;
use tokens::TokenStore;

const USER_SETTING: &str = "atlas_user";
/// Refresh this long before the access token runs out.
const REFRESH_MARGIN: Duration = Duration::from_secs(60);

struct Access {
    token: String,
    expires_at: Instant,
}

type Emitter = Box<dyn Fn(AtlasEvent) + Send + Sync>;

pub struct Cloud {
    db: SqlitePool,
    /// Server root, e.g. https://atlas.example. None = this build has no server.
    base: Option<Url>,
    http: reqwest::Client,
    /// For file transfers: no overall timeout (files can be 100 MB) and no redirects
    /// (download links are read from the server's redirect).
    blob_http: reqwest::Client,
    tokens: Arc<dyn TokenStore>,
    /// Also makes refreshing single-flight: requests wait for the one refresh in progress.
    access: tokio::sync::Mutex<Option<Access>>,
    online: AtomicBool,
    emit: OnceLock<Emitter>,
    /// The browser sign-in in progress. Starting another one cancels it.
    attempt: Mutex<Option<oneshot::Sender<()>>>,
    /// The WebSocket task while signed in (realtime.rs).
    realtime: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// What this PC is doing, sent to friends over the WebSocket.
    presence: watch::Sender<PresenceUpdate>,
}

impl Cloud {
    pub fn new(db: SqlitePool, mut base: Option<Url>, tokens: Arc<dyn TokenStore>) -> Self {
        // "https://host/atlas" → "https://host/atlas/", so "v1/…" joins under it.
        if let Some(url) = &mut base
            && !url.path().ends_with('/')
        {
            url.set_path(&format!("{}/", url.path()));
        }
        let http = reqwest::Client::builder()
            .user_agent(concat!("Atlas/", env!("CARGO_PKG_VERSION"), " (Windows)"))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("http client");
        let blob_http = reqwest::Client::builder()
            .user_agent(concat!("Atlas/", env!("CARGO_PKG_VERSION"), " (Windows)"))
            .connect_timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("http client");
        Self {
            db,
            base,
            http,
            blob_http,
            tokens,
            access: tokio::sync::Mutex::new(None),
            online: AtomicBool::new(true),
            emit: OnceLock::new(),
            attempt: Mutex::new(None),
            realtime: Mutex::new(None),
            presence: watch::Sender::new(PresenceUpdate::online()),
        }
    }

    /// The server from ATLAS_SERVER_URL: at run time (local development), else at build time.
    /// Development builds fall back to the local server.
    pub fn server_url_from_env() -> Option<Url> {
        let value = std::env::var("ATLAS_SERVER_URL")
            .ok()
            .or_else(|| option_env!("ATLAS_SERVER_URL").map(str::to_owned))
            .filter(|v| !v.trim().is_empty())
            .or_else(|| cfg!(debug_assertions).then(|| "http://localhost:8080".to_owned()))?;
        Url::parse(&value)
            .inspect_err(|e| tracing::error!(%value, error = %e, "bad ATLAS_SERVER_URL"))
            .ok()
    }

    /// Where events about the session and connection go (the UI, in the app).
    pub fn on_event(&self, f: impl Fn(AtlasEvent) + Send + Sync + 'static) {
        let _ = self.emit.set(Box::new(f));
    }

    pub fn is_configured(&self) -> bool {
        self.base.is_some()
    }

    pub fn is_signed_in(&self) -> bool {
        self.base.is_some() && self.tokens.get().is_some()
    }

    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }

    /// The saved user shows even when the server can't be reached (or none is set), so a
    /// server outage never looks like being signed out.
    pub async fn session(&self) -> AppResult<Session> {
        let user = if self.tokens.get().is_some() {
            db::settings::get::<Me>(&self.db, USER_SETTING).await?
        } else {
            None
        };
        Ok(Session {
            user,
            online: self.is_configured() && self.is_online(),
        })
    }

    /// At startup: refresh the signed-in user. Offline is fine; the cached user shows.
    pub async fn restore(&self) {
        if !self.is_signed_in() {
            return;
        }
        match self.call::<Me>(Method::GET, "/me", None::<&()>).await {
            Ok(me) => {
                if let Err(e) = self.set_me(&me).await {
                    tracing::warn!(error = %e, "couldn't cache the signed-in user");
                }
            }
            Err(e) => tracing::info!(error = %e, "couldn't refresh the session"),
        }
    }

    // ── Signing in ─────────────────────────────────────────────────────

    /// Sign in with Discord in the browser. `open` opens a URL there.
    pub async fn sign_in_discord(
        &self,
        open: impl FnOnce(&str) -> AppResult<()>,
    ) -> AppResult<Session> {
        let (code, verifier) = self.browser_flow(open).await?;
        let body = ExchangeRequest { code, verifier };
        let result: AuthResult = self
            .send(Method::POST, "/auth/exchange", None, Some(&body))
            .await?;
        self.start_session(&result.tokens).await;
        self.set_me(&result.user).await?;
        self.session().await
    }

    /// Add a Discord account to the signed-in account.
    pub async fn link_discord(&self, open: impl FnOnce(&str) -> AppResult<()>) -> AppResult<Me> {
        if !self.is_signed_in() {
            return Err(AppError::not_signed_in());
        }
        let (code, verifier) = self.browser_flow(open).await?;
        let me: Me = self
            .call(
                Method::POST,
                "/me/identities",
                Some(&ExchangeRequest { code, verifier }),
            )
            .await?;
        self.set_me(&me).await?;
        Ok(me)
    }

    /// Open the server's start page in the browser and wait for the one-time code to come
    /// back to a loopback listener. Returns `(code, PKCE verifier)`.
    async fn browser_flow(
        &self,
        open: impl FnOnce(&str) -> AppResult<()>,
    ) -> AppResult<(String, String)> {
        self.check_reachable().await?;
        let verifier = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
        let listener = loopback::bind().await?;
        let mut url = self.url("/auth/discord/start")?;
        url.query_pairs_mut()
            .append_pair("port", &listener.port.to_string())
            .append_pair("challenge", &pkce::challenge(&verifier));

        let (cancel, cancelled) = oneshot::channel();
        if let Some(previous) = self.attempt.lock().unwrap().replace(cancel) {
            let _ = previous.send(());
        }
        open(url.as_str())?;
        let code = tokio::select! {
            code = listener.wait_for_code() => code,
            _ = cancelled => Err(AppError::invalid_input("Sign-in was restarted")),
        };
        Ok((code?, verifier))
    }

    /// Is the server up? Asked before opening the browser, which would otherwise show its
    /// own "can't reach this page".
    async fn check_reachable(&self) -> AppResult<()> {
        let health = self
            .base
            .as_ref()
            .and_then(|b| b.join("healthz").ok())
            .ok_or_else(|| AppError::unknown("Bad server address"))?;
        let up = self
            .http
            .get(health)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success());
        self.set_online(up);
        if up { Ok(()) } else { Err(AppError::offline()) }
    }

    async fn start_session(&self, tokens: &Tokens) {
        self.tokens.set(&tokens.refresh_token);
        *self.access.lock().await = Some(Access {
            token: tokens.access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(tokens.expires_in),
        });
    }

    /// Save the signed-in user and tell the UI.
    pub async fn set_me(&self, me: &Me) -> AppResult<()> {
        db::settings::set(&self.db, USER_SETTING, me).await?;
        self.emit(AtlasEvent::SessionChanged {});
        Ok(())
    }

    /// Sign out here and on the server (if it can be reached).
    pub async fn sign_out(&self) -> AppResult<()> {
        if self.is_signed_in()
            && let Err(e) = self
                .call::<()>(Method::POST, "/auth/logout", None::<&()>)
                .await
        {
            tracing::info!(error = %e, "couldn't end the session on the server");
        }
        self.forget_session().await
    }

    /// Drop the session locally: token, access token, cached user and lists.
    pub async fn forget_session(&self) -> AppResult<()> {
        self.stop_realtime();
        self.tokens.clear();
        *self.access.lock().await = None;
        db::settings::delete(&self.db, USER_SETTING).await?;
        db::cloud_cache::clear(&self.db).await?;
        self.emit(AtlasEvent::SessionChanged {});
        Ok(())
    }

    // ── Requests ───────────────────────────────────────────────────────

    /// GET a list and remember it; offline, answer with the last copy if there is one.
    pub async fn cached_get<T: Serialize + DeserializeOwned>(
        &self,
        key: &str,
        path: &str,
    ) -> AppResult<T> {
        match self.call::<T>(Method::GET, path, None::<&()>).await {
            Ok(value) => {
                db::cloud_cache::set(&self.db, key, &value).await?;
                Ok(value)
            }
            Err(e) if e.code == ErrorCode::Offline => {
                db::cloud_cache::get(&self.db, key).await?.ok_or(e)
            }
            Err(e) => Err(e),
        }
    }

    /// An authenticated request to `/v1{path}`. Refreshes the access token when needed,
    /// and once more if the server says it has expired.
    pub async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> AppResult<T> {
        let token = self.access_token(false).await?;
        match self.send(method.clone(), path, Some(&token), body).await {
            Err(e) if e.code == ErrorCode::NotSignedIn => {
                let token = self.access_token(true).await?;
                self.send(method, path, Some(&token), body).await
            }
            other => other,
        }
    }

    async fn access_token(&self, force_refresh: bool) -> AppResult<String> {
        let mut access = self.access.lock().await;
        if !force_refresh
            && let Some(a) = &*access
            && a.expires_at > Instant::now() + REFRESH_MARGIN
        {
            return Ok(a.token.clone());
        }
        let refresh_token = self.tokens.get().ok_or_else(AppError::not_signed_in)?;
        let body = atlas_protocol::api::RefreshRequest { refresh_token };
        match self
            .send::<Tokens>(Method::POST, "/auth/refresh", None, Some(&body))
            .await
        {
            Ok(tokens) => {
                self.tokens.set(&tokens.refresh_token);
                *access = Some(Access {
                    token: tokens.access_token.clone(),
                    expires_at: Instant::now() + Duration::from_secs(tokens.expires_in),
                });
                Ok(tokens.access_token)
            }
            Err(e) if e.code == ErrorCode::NotSignedIn => {
                // The session ended on the server (signed out elsewhere, or deleted).
                *access = None;
                drop(access);
                self.forget_session().await?;
                Err(AppError::not_signed_in())
            }
            Err(e) => Err(e),
        }
    }

    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        bearer: Option<&str>,
        body: Option<&impl Serialize>,
    ) -> AppResult<T> {
        let mut req = self.http.request(method, self.url(path)?);
        if let Some(token) = bearer {
            req = req.bearer_auth(token);
        }
        if let Some(body) = body {
            req = req.json(body);
        }
        let res = match req.send().await {
            Ok(res) => res,
            Err(e) if e.is_connect() || e.is_timeout() => {
                self.set_online(false);
                return Err(AppError::offline());
            }
            Err(e) => {
                tracing::warn!(error = %e, "request to the Atlas server failed");
                self.set_online(false);
                return Err(AppError::offline());
            }
        };
        let status = res.status();
        // A proxy in front of a server that's down or restarting.
        if matches!(
            status,
            StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
        ) {
            self.set_online(false);
            return Err(AppError::offline());
        }
        self.set_online(true);
        let bytes = res.bytes().await.map_err(|e| {
            tracing::warn!(error = %e, "couldn't read the server's answer");
            AppError::unknown("The Atlas server's answer was cut off")
        })?;
        if !status.is_success() {
            return Err(server_error(status, &bytes));
        }
        let json = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).map_err(|e| {
                tracing::error!(error = %e, "unexpected answer from the server");
                AppError::unknown("The Atlas server sent something unexpected")
            })?
        };
        serde_json::from_value(json).map_err(|e| {
            tracing::error!(error = %e, "unexpected answer from the server");
            AppError::unknown("The Atlas server sent something unexpected")
        })
    }

    fn url(&self, path: &str) -> AppResult<Url> {
        let base = self.base.as_ref().ok_or_else(|| {
            AppError::unknown("This copy of Atlas isn't connected to an Atlas server")
        })?;
        base.join(&format!("v1{path}"))
            .map_err(|_| AppError::unknown("Bad server address"))
    }

    fn set_online(&self, online: bool) {
        if self.online.swap(online, Ordering::Relaxed) != online {
            self.emit(AtlasEvent::ConnectionChanged { online });
        }
    }

    fn emit(&self, event: AtlasEvent) {
        if let Some(emit) = self.emit.get() {
            emit(event);
        }
    }
}

/// The server's `{ error: { code, message } }` as the UI's error.
fn server_error(status: StatusCode, bytes: &[u8]) -> AppError {
    let Ok(ErrorBody { error }) = serde_json::from_slice::<ErrorBody>(bytes) else {
        tracing::warn!(%status, "server error without an error body");
        // Every route answers errors with a body, so a bare 404 is a route the server
        // doesn't have: it's older than this app.
        if status == StatusCode::NOT_FOUND {
            return AppError::unknown(
                "The Atlas server doesn't support this yet. It needs updating.",
            );
        }
        return AppError::unknown("The Atlas server had a problem. Try again later.");
    };
    let code = match error.code {
        WireCode::InvalidInput => ErrorCode::InvalidInput,
        WireCode::NotSignedIn => ErrorCode::NotSignedIn,
        WireCode::Forbidden => ErrorCode::Forbidden,
        WireCode::NotFound => ErrorCode::NotFound,
        WireCode::Conflict => ErrorCode::Conflict,
        WireCode::QuotaExceeded => ErrorCode::QuotaExceeded,
        WireCode::RateLimited => ErrorCode::RateLimited,
        WireCode::ClientOutdated | WireCode::Unknown => ErrorCode::Unknown,
    };
    AppError::new(code, error.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Signed in earlier, with the user saved, against `server`.
    async fn signed_in(server: Option<Url>) -> Cloud {
        let tokens = Arc::new(tokens::MemoryStore::default());
        tokens.set("refresh-token");
        let cloud = Cloud::new(db::open_in_memory().await.unwrap(), server, tokens);
        let me: Me = serde_json::from_value(serde_json::json!({
            "id": "u1", "username": "alex", "displayName": "Alex", "avatarUrl": null,
            "minecraftUuid": null, "minecraftUsername": null, "friendCode": "7KQ29XMB",
            "friendLink": "https://atlas.example/f/7KQ29XMB", "identities": [],
            "appearOffline": false, "usernameChangeAvailableAt": null,
            "storage": { "usedBytes": 0, "quotaBytes": 0, "maxFileBytes": 0 }
        }))
        .unwrap();
        cloud.set_me(&me).await.unwrap();
        cloud
    }

    /// A local address nothing listens on.
    fn closed_port() -> Url {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        Url::parse(&format!("http://127.0.0.1:{port}")).unwrap()
    }

    #[tokio::test]
    async fn a_server_that_is_down_keeps_you_signed_in() {
        let cloud = signed_in(Some(closed_port())).await;
        cloud.restore().await;
        let session = cloud.session().await.unwrap();
        assert_eq!(session.user.unwrap().user.username, "alex");
        assert!(!session.online);
        assert!(cloud.tokens.get().is_some(), "the saved sign-in is kept");

        let err = cloud.check_reachable().await.unwrap_err();
        assert_eq!(err.code, ErrorCode::Offline);
        assert!(err.message.contains("Atlas server"), "{}", err.message);
    }

    #[tokio::test]
    async fn no_server_address_still_shows_the_saved_user() {
        let cloud = signed_in(None).await;
        let session = cloud.session().await.unwrap();
        assert!(session.user.is_some());
        assert!(!session.online);
    }
}
