// T0 stubs: remove this allow as the commands get real bodies (phase T5).
#![allow(unused_variables)]

use crate::dto::{AuthProvider, Me, Session, UpdateProfileInput};
use crate::error::{AppError, AppResult};

/// Signed out until accounts exist (T5). `online: true` so the UI shows the normal
/// signed-out state rather than an "offline" banner.
#[tauri::command]
pub async fn get_session() -> AppResult<Session> {
    Ok(Session {
        user: None,
        online: true,
    })
}

#[tauri::command]
pub async fn sign_in(provider: AuthProvider) -> AppResult<Session> {
    Err(AppError::unknown("Signing in isn't available yet"))
}

#[tauri::command]
pub async fn sign_out() -> AppResult<()> {
    Ok(())
}

#[tauri::command]
pub async fn link_identity(provider: AuthProvider) -> AppResult<Me> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn unlink_identity(provider: AuthProvider) -> AppResult<Me> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn update_profile(patch: UpdateProfileInput) -> AppResult<Me> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn reset_friend_code() -> AppResult<Me> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn delete_account() -> AppResult<()> {
    Err(AppError::not_signed_in())
}
