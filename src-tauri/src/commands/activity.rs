// Not built yet: these need the Atlas server.
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
