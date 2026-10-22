// Some of these aren't built yet: they need the Atlas server.
#![allow(unused_variables)]

use crate::dto::{
    Id, InstanceSummary, InviteInput, InviteRole, PublishInput, PublishPlan, ShareInstanceInput,
    SharedInstance, SharedInstanceDetail, SyncOptions, SyncPreview, UpdateSharedInstanceInput,
};
use tauri::{AppHandle, Runtime, State};

use crate::error::{AppError, AppResult};
use crate::services::{instances, sync};
use crate::state::AppState;

/// Empty while signed in: sharing isn't on the server yet.
#[tauri::command]
pub async fn list_shared_instances(state: State<'_, AppState>) -> AppResult<Vec<SharedInstance>> {
    if !state.cloud.is_signed_in() {
        return Err(AppError::not_signed_in());
    }
    Ok(Vec::new())
}

#[tauri::command]
pub async fn get_shared_instance(id: Id) -> AppResult<SharedInstanceDetail> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn accept_invite(
    shared_instance_id: Id,
    launcher_id: Id,
    install_optional: Vec<String>,
) -> AppResult<InstanceSummary> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn decline_invite(shared_instance_id: Id) -> AppResult<()> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn prepare_publish(instance_id: Id) -> AppResult<PublishPlan> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn share_instance(
    instance_id: Id,
    input: ShareInstanceInput,
) -> AppResult<SharedInstance> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn publish_instance(instance_id: Id, input: PublishInput) -> AppResult<InstanceSummary> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn preview_sync(state: State<'_, AppState>, instance_id: Id) -> AppResult<SyncPreview> {
    sync::preview(&state, &instance_id).await
}

/// Backs up, then writes the latest version. Returns right away with status "syncing";
/// `instance.progress` events follow.
#[tauri::command]
pub async fn sync_instance<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
    options: SyncOptions,
) -> AppResult<InstanceSummary> {
    sync::sync(&state, &app, &instance_id, options).await?;
    instances::summary_by_id(&state, &instance_id).await
}

#[tauri::command]
pub async fn revert_local_changes<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
) -> AppResult<InstanceSummary> {
    sync::revert(&state, &app, &instance_id).await?;
    instances::summary_by_id(&state, &instance_id).await
}

#[tauri::command]
pub async fn update_shared_instance(
    id: Id,
    patch: UpdateSharedInstanceInput,
) -> AppResult<SharedInstance> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn invite_to_shared_instance(id: Id, input: InviteInput) -> AppResult<SharedInstance> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn set_shared_member_role(
    id: Id,
    user_id: Id,
    role: InviteRole,
) -> AppResult<SharedInstance> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn remove_shared_member(id: Id, user_id: Id) -> AppResult<SharedInstance> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn unshare_from_group(id: Id, group_id: Id) -> AppResult<SharedInstance> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn transfer_shared_instance(id: Id, user_id: Id) -> AppResult<SharedInstance> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn leave_shared_instance(id: Id) -> AppResult<()> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn archive_shared_instance(id: Id) -> AppResult<()> {
    Err(AppError::not_signed_in())
}
