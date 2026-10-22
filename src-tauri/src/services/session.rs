//! Signing in and the account. The server does the work; `Cloud` keeps the session.

use reqwest::Method;
use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;

use crate::dto::{AuthProvider, Me, Session, UpdateProfileInput};
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
        AuthProvider::Minecraft => return Err(minecraft_not_ready()),
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
        AuthProvider::Minecraft => Err(minecraft_not_ready()),
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

fn open<R: Runtime>(app: &AppHandle<R>, url: &str) -> AppResult<()> {
    app.opener().open_url(url, None::<&str>).map_err(|e| {
        tracing::error!(error = %e, "couldn't open the browser");
        AppError::unknown("Couldn't open your browser")
    })
}

fn minecraft_not_ready() -> AppError {
    AppError::unknown("Minecraft sign-in isn't available yet. Use Discord for now.")
}
