use tauri::{AppHandle, Runtime, State};

use crate::dto::{AuthProvider, Me, Session, UpdateProfileInput};
use crate::error::AppResult;
use crate::services::session;
use crate::state::AppState;

/// Signed out with `online: true` when this build has no server, so the UI shows the
/// normal signed-out state rather than an "offline" banner.
#[tauri::command]
pub async fn get_session(state: State<'_, AppState>) -> AppResult<Session> {
    session::get(&state).await
}

#[tauri::command]
pub async fn sign_in<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    provider: AuthProvider,
) -> AppResult<Session> {
    session::sign_in(&state, &app, provider).await
}

#[tauri::command]
pub async fn sign_out(state: State<'_, AppState>) -> AppResult<()> {
    state.cloud.sign_out().await
}

#[tauri::command]
pub async fn link_identity<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    provider: AuthProvider,
) -> AppResult<Me> {
    session::link(&state, &app, provider).await
}

#[tauri::command]
pub async fn unlink_identity(state: State<'_, AppState>, provider: AuthProvider) -> AppResult<Me> {
    session::unlink(&state, provider).await
}

#[tauri::command]
pub async fn update_profile(
    state: State<'_, AppState>,
    patch: UpdateProfileInput,
) -> AppResult<Me> {
    session::update_profile(&state, &patch).await
}

#[tauri::command]
pub async fn reset_friend_code(state: State<'_, AppState>) -> AppResult<Me> {
    session::reset_friend_code(&state).await
}

#[tauri::command]
pub async fn delete_account(state: State<'_, AppState>) -> AppResult<()> {
    session::delete_account(&state).await
}
