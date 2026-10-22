use reqwest::Method;
use tauri::{AppHandle, Runtime, State};

use crate::dto::{Friend, FriendRequest, Id, OpenedLink, PublicUser};
use crate::error::AppResult;
use crate::services::friends;
use crate::state::AppState;

#[tauri::command]
pub async fn list_friends(state: State<'_, AppState>) -> AppResult<Vec<Friend>> {
    friends::list(&state).await
}

#[tauri::command]
pub async fn list_friend_requests(state: State<'_, AppState>) -> AppResult<Vec<FriendRequest>> {
    friends::requests(&state).await
}

#[tauri::command]
pub async fn preview_friend_code(
    state: State<'_, AppState>,
    code: String,
) -> AppResult<PublicUser> {
    friends::preview_code(&state, &code).await
}

#[tauri::command]
pub async fn send_friend_request<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    code: String,
) -> AppResult<FriendRequest> {
    friends::send_request(&state, &app, &code).await
}

#[tauri::command]
pub async fn respond_to_friend_request<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    user_id: Id,
    accept: bool,
) -> AppResult<()> {
    friends::respond(&state, &app, &user_id, accept).await
}

#[tauri::command]
pub async fn cancel_friend_request<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    user_id: Id,
) -> AppResult<()> {
    friends::change(&state, &app, Method::DELETE, "/friends/requests", &user_id).await
}

#[tauri::command]
pub async fn remove_friend<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    user_id: Id,
) -> AppResult<()> {
    friends::change(&state, &app, Method::DELETE, "/friends", &user_id).await
}

#[tauri::command]
pub async fn block_user<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    user_id: Id,
) -> AppResult<()> {
    friends::change(&state, &app, Method::POST, "/blocks", &user_id).await
}

/// The friend or group link Atlas was opened with, if the UI hasn't taken it yet.
#[tauri::command]
pub fn take_opened_link(state: State<'_, AppState>) -> Option<OpenedLink> {
    state.opened_link.lock().unwrap().take()
}
