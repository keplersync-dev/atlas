// T0 stubs: remove this allow as the commands get real bodies (phase T1).
#![allow(unused_variables)]

use crate::dto::{DetectedInstance, Id, Launcher, LauncherKind};
use crate::error::{AppError, AppResult};

/// T1: rows from the `launcher` table plus each adapter's instance count.
#[tauri::command]
pub async fn list_launchers() -> AppResult<Vec<Launcher>> {
    Ok(Vec::new())
}

/// T1: run every adapter's detect(), upsert, emit launchers.changed.
#[tauri::command]
pub async fn detect_launchers() -> AppResult<Vec<Launcher>> {
    Ok(Vec::new())
}

#[tauri::command]
pub async fn add_launcher(kind: LauncherKind, root_path: String) -> AppResult<Launcher> {
    Err(AppError::not_built())
}

#[tauri::command]
pub async fn remove_launcher(id: Id) -> AppResult<()> {
    Err(AppError::not_found("Launcher not found"))
}

/// T1: every instance in every known launcher, tracked or not.
#[tauri::command]
pub async fn list_detected_instances() -> AppResult<Vec<DetectedInstance>> {
    Ok(Vec::new())
}
