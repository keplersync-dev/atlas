use tauri::{AppHandle, Runtime, State};

use crate::dto::{AuthProvider, Id, Me, NewReport, ReportReason, Session, UpdateProfileInput};
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

#[tauri::command]
pub async fn report(state: State<'_, AppState>, input: NewReport) -> AppResult<()> {
    session::report(&state, input).await
}

/// Everything the server stores about this account, saved in Downloads. Returns the path.
#[tauri::command]
pub async fn export_my_data<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> AppResult<String> {
    session::export_data(&app, &state).await
}

/// "terms" or "privacy", from this Atlas's server, in the browser.
#[tauri::command]
pub async fn open_legal_page<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    page: String,
) -> AppResult<()> {
    session::open_legal_page(&app, &state, &page)
}

/// An uploaded file in a shared instance, by its content row.
#[tauri::command]
pub async fn report_file(
    state: State<'_, AppState>,
    instance_id: Id,
    content_id: Id,
    reason: ReportReason,
    details: Option<String>,
) -> AppResult<()> {
    session::report_file(&state, &instance_id, &content_id, reason, details).await
}
