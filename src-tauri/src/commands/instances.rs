use tauri::{AppHandle, Runtime, State};

use crate::dto::{
    Id, InstanceDetail, InstanceSummary, MemberRole, ShareMode, TrackInstanceRef,
    UpdateInstanceInput,
};
use crate::error::AppResult;
use crate::events::{AtlasEvent, emit};
use crate::services::{instances, sync};
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
pub async fn track_instances<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    refs: Vec<TrackInstanceRef>,
) -> AppResult<Vec<InstanceSummary>> {
    instances::track(&state, &app, &refs).await
}

#[tauri::command]
pub async fn track_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    game_dir: String,
) -> AppResult<InstanceSummary> {
    instances::track_folder(&state, &app, &game_dir).await
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

/// Re-read launcher metadata and the files now (the watcher also does this on its own).
#[tauri::command]
pub async fn rescan_instance<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
) -> AppResult<InstanceSummary> {
    instances::rescan(&state, &app, &id).await
}

/// Renames the file to/from ".disabled". Refused while the game is running.
#[tauri::command]
pub async fn set_content_enabled<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
    content_id: Id,
    enabled: bool,
) -> AppResult<()> {
    instances::set_content_enabled(&state, &app, &instance_id, &content_id, enabled).await
}

/// Owners/editors: how a file is shared. Reaches friends with the next publish.
#[tauri::command]
pub async fn set_share_mode<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
    content_id: Id,
    mode: ShareMode,
) -> AppResult<()> {
    instances::set_share_mode(&state, &app, &instance_id, &content_id, mode).await
}

#[tauri::command]
pub async fn open_in_launcher(state: State<'_, AppState>, id: Id) -> AppResult<()> {
    instances::open_in_launcher(&state, &id).await
}

/// Close the instance's launcher, as its own close button would: a new Modrinth App
/// instance waits for that to get its picture.
#[tauri::command]
pub async fn close_launcher(state: State<'_, AppState>, id: Id) -> AppResult<()> {
    instances::close_launcher(&state, &id).await
}

/// Download a new instance's files without waiting to give the launcher its picture.
#[tauri::command]
pub async fn skip_launcher_picture(state: State<'_, AppState>, id: Id) -> AppResult<()> {
    state.skip_picture.lock().unwrap().insert(id);
    Ok(())
}

#[tauri::command]
pub async fn open_instance_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
) -> AppResult<()> {
    instances::open_folder(&state, &app, &id).await
}

/// Undo what a backup recorded. Returns right away with status "syncing".
#[tauri::command]
pub async fn restore_backup<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
    backup_id: Id,
) -> AppResult<InstanceSummary> {
    sync::restore(&state, &app, &instance_id, &backup_id).await?;
    instances::summary_by_id(&state, &instance_id).await
}

/// Stop the instance's update, install or restore, if it hasn't started changing files.
/// The job then ends as usual (`instance.progress` with status "ready").
#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>, instance_id: Id) -> AppResult<()> {
    if state.jobs.cancel(&instance_id) {
        Ok(())
    } else {
        Err(crate::error::AppError::conflict("Nothing is running on it"))
    }
}

/// Download or remove an optional item from the shared version (waits until done).
#[tauri::command]
pub async fn set_optional_installed<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
    content_id: Id,
    installed: bool,
) -> AppResult<()> {
    sync::set_optional_installed(&state, &app, &instance_id, &content_id, installed).await
}

// ── Development only ───────────────────────────────────────────────────

/// Debug builds: link an instance to a folder of manifests (see sync/source.rs).
/// From the devtools console:
///   __TAURI_INTERNALS__.invoke("dev_link_manifest", { instanceId, folder, role: "viewer", adoptRevision: 1 })
#[tauri::command]
pub async fn dev_link_manifest<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
    folder: String,
    role: MemberRole,
    adopt_revision: Option<u32>,
) -> AppResult<InstanceSummary> {
    sync::dev_link(&state, &instance_id, &folder, role, adopt_revision).await?;
    emit(
        &app,
        AtlasEvent::InstanceChanged {
            instance_id: instance_id.clone(),
        },
    );
    instances::summary_by_id(&state, &instance_id).await
}

/// Debug builds: undo dev_link_manifest. The folder stays as it is.
#[tauri::command]
pub async fn dev_unlink_manifest<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
) -> AppResult<InstanceSummary> {
    sync::unlink(&state, &instance_id).await?;
    emit(
        &app,
        AtlasEvent::InstanceChanged {
            instance_id: instance_id.clone(),
        },
    );
    instances::summary_by_id(&state, &instance_id).await
}
