// T0 stubs: remove this allow as the commands get real bodies (phases T1–T4).
// Nothing can be tracked yet, so "not found" is the true answer for every id.
#![allow(unused_variables)]

use crate::dto::{
    Id, InstanceDetail, InstanceSummary, ShareMode, TrackInstanceRef, UpdateInstanceInput,
};
use crate::error::{AppError, AppResult};

fn no_instance() -> AppError {
    AppError::not_found("Instance not found")
}

/// T1: every tracked instance as a summary (no file or network access).
#[tauri::command]
pub async fn list_instances() -> AppResult<Vec<InstanceSummary>> {
    Ok(Vec::new())
}

#[tauri::command]
pub async fn get_instance(id: Id) -> AppResult<InstanceDetail> {
    Err(no_instance())
}

#[tauri::command]
pub async fn track_instances(refs: Vec<TrackInstanceRef>) -> AppResult<Vec<InstanceSummary>> {
    Err(AppError::not_built())
}

#[tauri::command]
pub async fn track_folder(game_dir: String) -> AppResult<InstanceSummary> {
    Err(AppError::not_built())
}

#[tauri::command]
pub async fn untrack_instance(id: Id) -> AppResult<()> {
    Err(no_instance())
}

#[tauri::command]
pub async fn update_instance(id: Id, patch: UpdateInstanceInput) -> AppResult<InstanceSummary> {
    Err(no_instance())
}

#[tauri::command]
pub async fn rescan_instance(id: Id) -> AppResult<InstanceSummary> {
    Err(no_instance())
}

#[tauri::command]
pub async fn open_in_launcher(id: Id) -> AppResult<()> {
    Err(no_instance())
}

#[tauri::command]
pub async fn open_instance_folder(id: Id) -> AppResult<()> {
    Err(no_instance())
}

#[tauri::command]
pub async fn set_content_enabled(instance_id: Id, content_id: Id, enabled: bool) -> AppResult<()> {
    Err(no_instance())
}

#[tauri::command]
pub async fn restore_backup(instance_id: Id, backup_id: Id) -> AppResult<InstanceSummary> {
    Err(no_instance())
}

#[tauri::command]
pub async fn set_share_mode(instance_id: Id, content_id: Id, mode: ShareMode) -> AppResult<()> {
    Err(no_instance())
}

#[tauri::command]
pub async fn set_optional_installed(
    instance_id: Id,
    content_id: Id,
    installed: bool,
) -> AppResult<()> {
    Err(no_instance())
}
