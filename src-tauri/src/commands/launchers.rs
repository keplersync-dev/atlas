use tauri::{AppHandle, Runtime, State};

use crate::dto::{DetectedInstance, Id, Launcher, LauncherKind};
use crate::error::AppResult;
use crate::services::launchers;
use crate::state::AppState;

#[tauri::command]
pub async fn list_launchers(state: State<'_, AppState>) -> AppResult<Vec<Launcher>> {
    launchers::list(&state).await
}

#[tauri::command]
pub async fn detect_launchers<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> AppResult<Vec<Launcher>> {
    launchers::detect(&state, &app).await
}

#[tauri::command]
pub async fn add_launcher<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    kind: LauncherKind,
    root_path: String,
) -> AppResult<Launcher> {
    launchers::add(&state, &app, kind, &root_path).await
}

#[tauri::command]
pub async fn remove_launcher<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
) -> AppResult<()> {
    launchers::remove(&state, &app, &id).await
}

#[tauri::command]
pub async fn list_detected_instances(
    state: State<'_, AppState>,
) -> AppResult<Vec<DetectedInstance>> {
    launchers::detected_instances(&state).await
}

/// Pick a Modrinth pack and give it to this launcher (Prism or the Modrinth App) to
/// install. False if the user cancelled.
#[tauri::command]
pub async fn import_mrpack<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    launcher_id: Id,
) -> AppResult<bool> {
    crate::services::mrpack::import(&app, &state, &launcher_id).await
}
