use tauri::State;

use crate::dto::{ActivityIds, ActivityItem};
use crate::error::AppResult;
use crate::services::friends;
use crate::state::AppState;

#[tauri::command]
pub async fn list_activity(state: State<'_, AppState>) -> AppResult<Vec<ActivityItem>> {
    friends::activity(&state).await
}

#[tauri::command]
pub async fn mark_activity_read(state: State<'_, AppState>, ids: ActivityIds) -> AppResult<()> {
    friends::mark_read(&state, ids).await
}
