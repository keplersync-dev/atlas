//! Signing in and the account. The server does the work; `Cloud` keeps the session.

use reqwest::Method;
use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;

use crate::dto::{
    AuthProvider, Me, NewReport, ReportReason, ReportTarget, Session, UpdateProfileInput,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub async fn get(state: &AppState) -> AppResult<Session> {
    state.cloud.session().await
}

pub async fn sign_in<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    provider: AuthProvider,
) -> AppResult<Session> {
    let session = match provider {
        AuthProvider::Discord => state.cloud.sign_in_discord(|url| open(app, url)).await?,
        AuthProvider::Minecraft => state.cloud.sign_in_minecraft(|url| open(app, url)).await?,
    };
    super::presence::refresh(state).await;
    state.cloud.start_realtime();
    Ok(session)
}

pub async fn link<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    provider: AuthProvider,
) -> AppResult<Me> {
    match provider {
        AuthProvider::Discord => state.cloud.link_discord(|url| open(app, url)).await,
        AuthProvider::Minecraft => state.cloud.link_minecraft(|url| open(app, url)).await,
    }
}

pub async fn unlink(state: &AppState, provider: AuthProvider) -> AppResult<Me> {
    let path = format!("/me/identities/{}", crate::dto::to_wire(provider));
    let me: Me = state.cloud.call(Method::DELETE, &path, None::<&()>).await?;
    state.cloud.set_me(&me).await?;
    Ok(me)
}

pub async fn update_profile(state: &AppState, patch: &UpdateProfileInput) -> AppResult<Me> {
    let me: Me = state.cloud.call(Method::PATCH, "/me", Some(patch)).await?;
    state.cloud.set_me(&me).await?;
    Ok(me)
}

pub async fn reset_friend_code(state: &AppState) -> AppResult<Me> {
    let me: Me = state
        .cloud
        .call(Method::POST, "/me/friend-code/reset", None::<&()>)
        .await?;
    state.cloud.set_me(&me).await?;
    Ok(me)
}

pub async fn delete_account(state: &AppState) -> AppResult<()> {
    state
        .cloud
        .call::<()>(Method::DELETE, "/me", None::<&()>)
        .await?;
    state.cloud.forget_session().await
}

/// Tell Atlas about a file, shared instance, group or person that breaks the rules.
pub async fn report(state: &AppState, input: NewReport) -> AppResult<()> {
    state
        .cloud
        .call::<()>(Method::POST, "/reports", Some(&input))
        .await
}

/// An uploaded file in a shared instance, by its row here: the server wants its sha256 and
/// the shared instance it's in.
pub async fn report_file(
    state: &AppState,
    instance_id: &str,
    content_id: &str,
    reason: ReportReason,
    details: Option<String>,
) -> AppResult<()> {
    let row = crate::db::instances::get(&state.db, instance_id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))?;
    let shared = super::shared::server_id(&row)
        .ok_or_else(|| AppError::conflict("This instance isn't shared"))?
        .to_owned();
    let rows = crate::db::content::list(&state.db, instance_id).await?;
    let file = rows
        .iter()
        .find(|r| r.id == content_id)
        .ok_or_else(|| AppError::not_found("That file isn't in this instance any more"))?;
    let input = NewReport {
        target_type: ReportTarget::Blob,
        target_id: file.sha256.clone(),
        reason,
        details,
        shared_instance_id: Some(shared),
    };
    report(state, input).await
}

/// The Terms of Service or Privacy Policy, in the browser, from this Atlas's server.
pub fn open_legal_page<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    page: &str,
) -> AppResult<()> {
    let path = match page {
        "terms" => "/terms",
        "privacy" => "/privacy",
        _ => return Err(AppError::invalid_input("No such page")),
    };
    open(app, state.cloud.page_url(path)?.as_str())
}

fn open<R: Runtime>(app: &AppHandle<R>, url: &str) -> AppResult<()> {
    app.opener().open_url(url, None::<&str>).map_err(|e| {
        tracing::error!(error = %e, "couldn't open the browser");
        AppError::unknown("Couldn't open your browser")
    })
}
