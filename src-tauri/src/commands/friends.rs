// T0 stubs: remove this allow as the commands get real bodies (phase T6).
#![allow(unused_variables)]

use crate::dto::{Friend, FriendRequest, Id, PublicUser};
use crate::error::{AppError, AppResult};

#[tauri::command]
pub async fn list_friends() -> AppResult<Vec<Friend>> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn list_friend_requests() -> AppResult<Vec<FriendRequest>> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn preview_friend_code(code: String) -> AppResult<PublicUser> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn send_friend_request(code: String) -> AppResult<FriendRequest> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn respond_to_friend_request(user_id: Id, accept: bool) -> AppResult<()> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn cancel_friend_request(user_id: Id) -> AppResult<()> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn remove_friend(user_id: Id) -> AppResult<()> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn block_user(user_id: Id) -> AppResult<()> {
    Err(AppError::not_signed_in())
}
