// T0 stubs: remove this allow as the commands get real bodies (phase T6).
#![allow(unused_variables)]

use crate::dto::{ActivityIds, ActivityItem};
use crate::error::{AppError, AppResult};

#[tauri::command]
pub async fn list_activity() -> AppResult<Vec<ActivityItem>> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn mark_activity_read(ids: ActivityIds) -> AppResult<()> {
    Err(AppError::not_signed_in())
}
