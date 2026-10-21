use tauri::{AppHandle, Runtime, State};

use crate::dto::{
    Id, InstanceDetail, InstanceSummary, ShareMode, TrackInstanceRef, UpdateInstanceInput,
};
use crate::error::{AppError, AppResult};
use crate::services::instances;
use crate::state::AppState;

#[tauri::command]
pub async fn list_instances(state: State<'_, AppState>) -> AppResult<Vec<InstanceSummary>> {
    instances::list(&state).await
}

#[tauri::command]
pub async fn get_instance(state: State<'_, AppState>, id: Id) -> AppResult<InstanceDetail> {
    instances::detail(&state, &id).await
}

#[tauri::command]
pub async fn track_instances(
    state: State<'_, AppState>,
    refs: Vec<TrackInstanceRef>,
) -> AppResult<Vec<InstanceSummary>> {
    instances::track(&state, &refs).await
}

#[tauri::command]
pub async fn track_folder(state: State<'_, AppState>, game_dir: String) -> AppResult<InstanceSummary> {
    instances::track_folder(&state, &game_dir).await
}

#[tauri::command]
pub async fn untrack_instance(state: State<'_, AppState>, id: Id) -> AppResult<()> {
    instances::untrack(&state, &id).await
}

#[tauri::command]
pub async fn update_instance(
    state: State<'_, AppState>,
    id: Id,
    patch: UpdateInstanceInput,
) -> AppResult<InstanceSummary> {
    instances::update(&state, &id, &patch).await
}

/// T1 re-reads launcher metadata; T2 adds the file scan.
#[tauri::command]
pub async fn rescan_instance(state: State<'_, AppState>, id: Id) -> AppResult<InstanceSummary> {
    Ok(instances::refresh(&state, &id).await?.0)
}

#[tauri::command]
pub async fn open_in_launcher(state: State<'_, AppState>, id: Id) -> AppResult<()> {
    instances::open_in_launcher(&state, &id).await
}

#[tauri::command]
pub async fn open_instance_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
) -> AppResult<()> {
    instances::open_folder(&state, &app, &id).await
}

// ── Built in later phases ──────────────────────────────────────────────

/// T2: rename to/from ".disabled".
#[tauri::command]
#[allow(unused_variables)]
pub async fn set_content_enabled(instance_id: Id, content_id: Id, enabled: bool) -> AppResult<()> {
    Err(AppError::not_built())
}

/// T4: sync engine.
#[tauri::command]
#[allow(unused_variables)]
pub async fn restore_backup(instance_id: Id, backup_id: Id) -> AppResult<InstanceSummary> {
    Err(AppError::not_built())
}

/// T2: share settings live on content rows, which the scanner creates.
#[tauri::command]
#[allow(unused_variables)]
pub async fn set_share_mode(instance_id: Id, content_id: Id, mode: ShareMode) -> AppResult<()> {
    Err(AppError::not_built())
}

/// T4: sync engine (downloads).
#[tauri::command]
#[allow(unused_variables)]
pub async fn set_optional_installed(
    instance_id: Id,
    content_id: Id,
    installed: bool,
) -> AppResult<()> {
    Err(AppError::not_built())
}
