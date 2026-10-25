//! The WebSocket to the server while signed in: friends' presence, "something changed"
//! hints and new activity come in; this PC's presence goes out. Reconnects with backoff;
//! every (re)connect refetches the friends list, since messages may have been missed.

use std::sync::Arc;
use std::time::Duration;

use atlas_protocol::ws::{ClientMessage, HEARTBEAT_SECONDS, PresenceUpdate, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{AUTHORIZATION, USER_AGENT};

use super::Cloud;
use crate::dto::SharedInstance;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::AtlasEvent;
use crate::services::shared::CACHE_KEY as SHARED_CACHE_KEY;

const MAX_BACKOFF: Duration = Duration::from_secs(30);

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Why a connection ended.
enum End {
    /// Dropped or failed: try again.
    Lost,
    /// The server ended this session: stay signed out.
    Revoked,
}

impl Cloud {
    /// Start the socket if signed in and it isn't running already.
    pub fn start_realtime(self: &Arc<Self>) {
        if !self.is_signed_in() {
            return;
        }
        let mut task = self.realtime.lock().unwrap();
        if task.as_ref().is_some_and(|t| !t.inner().is_finished()) {
            return;
        }
        let cloud = Arc::clone(self);
        *task = Some(tauri::async_runtime::spawn(async move {
            cloud.keep_connected().await
        }));
    }

    /// Stop the socket. Called from the socket task itself (the server ended the session),
    /// it lets the task finish signing out instead of cutting it off.
    pub fn stop_realtime(&self) {
        if let Some(task) = self.realtime.lock().unwrap().take()
            && tokio::task::try_id() != Some(task.inner().id())
        {
            task.abort();
        }
    }

    /// Tell friends what this PC is doing. Sent now if connected, else on connect.
    pub fn set_presence(&self, update: PresenceUpdate) {
        self.presence.send_if_modified(|current| {
            let changed = *current != update;
            *current = update;
            changed
        });
    }

    async fn keep_connected(&self) {
        let mut backoff = Duration::from_secs(1);
        while self.is_signed_in() {
            match self.connect().await {
                Ok(socket) => {
                    backoff = Duration::from_secs(1);
                    match self.run(socket).await {
                        End::Revoked => {
                            if let Err(e) = self.forget_session().await {
                                tracing::warn!(error = %e, "couldn't sign out locally");
                            }
                            return;
                        }
                        End::Lost => tracing::info!("connection to the Atlas server lost"),
                    }
                }
                // Retrying won't help until Atlas is updated.
                Err(e) if matches!(e.code, ErrorCode::NotSignedIn | ErrorCode::ClientOutdated) => {
                    return;
                }
                Err(e) => tracing::debug!(error = %e, "couldn't connect to the Atlas server"),
            }
            self.set_online(false);
            let jitter = Duration::from_millis(u64::from(rand::random::<u16>() % 1000));
            tokio::time::sleep(backoff + jitter).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }

    async fn connect(&self) -> AppResult<Socket> {
        let mut url = self.url("/ws")?;
        let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme)
            .map_err(|_| AppError::unknown("Bad server address"))?;
        // A refused handshake may mean the access token just expired: refresh next time.
        for force_refresh in [false, true] {
            let token = self.access_token(force_refresh).await?;
            let mut req = url
                .as_str()
                .into_client_request()
                .map_err(|_| AppError::unknown("Bad server address"))?;
            let headers = req.headers_mut();
            headers.insert(AUTHORIZATION, bearer(&token)?);
            headers.insert(
                USER_AGENT,
                concat!("Atlas/", env!("CARGO_PKG_VERSION"), " (Windows)")
                    .parse()
                    .expect("valid header"),
            );
            match tokio_tungstenite::connect_async(req).await {
                Ok((socket, _)) => return Ok(socket),
                Err(tokio_tungstenite::tungstenite::Error::Http(res))
                    if res.status() == 401 && !force_refresh =>
                {
                    continue;
                }
                Err(tokio_tungstenite::tungstenite::Error::Http(res)) if res.status() == 426 => {
                    self.emit(AtlasEvent::UpdateRequired {});
                    return Err(AppError::new(
                        ErrorCode::ClientOutdated,
                        "This version of Atlas is too old",
                    ));
                }
                Err(e) => {
                    tracing::debug!(error = %e, "WebSocket handshake failed");
                    return Err(AppError::offline());
                }
            }
        }
        Err(AppError::not_signed_in())
    }

    async fn run(&self, socket: Socket) -> End {
        let (mut sink, mut stream) = socket.split();
        let mut presence = self.presence.subscribe();
        let mut ping = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECONDS));
        ping.tick().await;

        loop {
            tokio::select! {
                incoming = stream.next() => {
                    let text = match incoming {
                        Some(Ok(Message::Text(text))) => text,
                        Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return End::Lost,
                        Some(Ok(_)) => continue,
                    };
                    match ServerMessage::parse(&text) {
                        ServerMessage::Hello { .. } => {
                            tracing::info!("connected to the Atlas server");
                            self.set_online(true);
                            self.emit(AtlasEvent::FriendsChanged {});
                            let now = presence.borrow_and_update().clone();
                            if send(&mut sink, ClientMessage::PresenceSet(now)).await.is_err() {
                                return End::Lost;
                            }
                        }
                        ServerMessage::SessionRevoked {} => return End::Revoked,
                        other => self.forward(other).await,
                    }
                }
                changed = presence.changed() => {
                    if changed.is_err() {
                        return End::Lost;
                    }
                    let now = presence.borrow_and_update().clone();
                    if send(&mut sink, ClientMessage::PresenceSet(now)).await.is_err() {
                        return End::Lost;
                    }
                }
                _ = ping.tick() => {
                    if send(&mut sink, ClientMessage::Ping {}).await.is_err() {
                        return End::Lost;
                    }
                }
            }
        }
    }

    /// Server hints → UI events. A shared instance change refreshes the cached list
    /// first, so instance summaries already see the new latest revision.
    async fn forward(&self, msg: ServerMessage) {
        let event = match msg {
            ServerMessage::PresenceChanged { user_id, presence } => {
                AtlasEvent::PresenceChanged { user_id, presence }
            }
            ServerMessage::FriendsChanged {} => AtlasEvent::FriendsChanged {},
            ServerMessage::MeChanged {} => {
                // Saving the new `Me` tells the UI (`session.changed`).
                self.restore().await;
                return;
            }
            ServerMessage::NotificationNew { item } => AtlasEvent::ActivityNew { item },
            ServerMessage::SharedChanged {
                shared_instance_id, ..
            } => {
                let refreshed = self
                    .cached_get::<Vec<SharedInstance>>(SHARED_CACHE_KEY, "/shared-instances")
                    .await;
                match refreshed {
                    // A changed role (the owner made them an editor) applies straight away.
                    Ok(list) => {
                        if let Err(e) = crate::db::instances::sync_roles(&self.db, &list).await {
                            tracing::warn!(error = %e, "couldn't update roles");
                        }
                    }
                    Err(e) => tracing::info!(error = %e, "couldn't refresh shared instances"),
                }
                AtlasEvent::SharedChanged { shared_instance_id }
            }
            ServerMessage::GroupsChanged { group_id } => AtlasEvent::GroupsChanged {
                group_id: Some(group_id),
            },
            ServerMessage::Hello { .. }
            | ServerMessage::SessionRevoked {}
            | ServerMessage::Pong {}
            | ServerMessage::Unknown => return,
        };
        self.emit(event);
    }
}

async fn send<S>(sink: &mut S, msg: ClientMessage) -> Result<(), S::Error>
where
    S: futures_util::Sink<Message> + Unpin,
{
    let text = serde_json::to_string(&msg).expect("messages serialize");
    sink.send(Message::Text(text.into())).await
}

fn bearer(token: &str) -> AppResult<tokio_tungstenite::tungstenite::http::HeaderValue> {
    format!("Bearer {token}")
        .parse()
        .map_err(|_| AppError::not_signed_in())
}
