use reqwest::Method;
use tauri::{AppHandle, Runtime, State};

use crate::dto::{Friend, FriendRequest, Id, OpenedLink, PublicUser, SendFriendRequest};
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

/// Who has this Minecraft name, or this friend link.
#[tauri::command]
pub async fn preview_friend(
    state: State<'_, AppState>,
    who: SendFriendRequest,
) -> AppResult<PublicUser> {
    friends::preview(&state, &who).await
}

#[tauri::command]
pub async fn send_friend_request<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    who: SendFriendRequest,
) -> AppResult<FriendRequest> {
    friends::send_request(&state, &app, who).await
}

#[tauri::command]
pub async fn send_friend_request_to<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    user_id: Id,
) -> AppResult<FriendRequest> {
    friends::send_request_to(&state, &app, &user_id).await
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

#[tauri::command]
pub async fn unblock_user<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    user_id: Id,
) -> AppResult<()> {
    friends::change(&state, &app, Method::DELETE, "/blocks", &user_id).await
}

#[tauri::command]
pub async fn list_blocked(state: State<'_, AppState>) -> AppResult<Vec<crate::dto::PublicUser>> {
    friends::blocked(&state).await
}

/// The friend or group link Atlas was opened with, if the UI hasn't taken it yet.
#[tauri::command]
pub fn take_opened_link(state: State<'_, AppState>) -> Option<OpenedLink> {
    state.opened_link.lock().unwrap().take()
}
