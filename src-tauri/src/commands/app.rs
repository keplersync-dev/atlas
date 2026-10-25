use tauri::{AppHandle, Runtime, State};

use crate::desktop;
use crate::dto::{AppSettings, AppSettingsPatch, AvailableUpdate};
use crate::error::AppResult;
use crate::services::updates;
use crate::state::AppState;

#[tauri::command]
pub async fn get_app_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> AppResult<AppSettings> {
    Ok(desktop::settings(&app, &state))
}

#[tauri::command]
pub async fn update_app_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    patch: AppSettingsPatch,
) -> AppResult<AppSettings> {
    desktop::update_settings(&app, &state, patch).await
}

/// The page has drawn its first frame: show the window (it starts hidden so it never
/// shows a blank page), unless Atlas started with Windows and stays in the tray.
#[tauri::command]
pub async fn window_ready<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    desktop::window_ready(&app, &state);
    Ok(())
}

#[tauri::command]
pub async fn check_for_update<R: Runtime>(app: AppHandle<R>) -> AppResult<Option<AvailableUpdate>> {
    updates::check(&app).await
}

/// Doesn't return when it works: Atlas restarts into the new version.
#[tauri::command]
pub async fn install_update<R: Runtime>(app: AppHandle<R>) -> AppResult<()> {
    updates::install(&app).await
}

#[tauri::command]
pub async fn open_release_page<R: Runtime>(app: AppHandle<R>) -> AppResult<()> {
    updates::open_release_page(&app)
}
