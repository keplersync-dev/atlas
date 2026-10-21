// T0 stubs: remove this allow as the commands get real bodies (phases T4, T7).
#![allow(unused_variables)]

use crate::dto::{
    Id, InstanceSummary, InviteInput, InviteRole, PublishInput, PublishPlan, ShareInstanceInput,
    SharedInstance, SharedInstanceDetail, SyncOptions, SyncPreview, UpdateSharedInstanceInput,
};
use crate::error::{AppError, AppResult};

fn no_instance() -> AppError {
    AppError::not_found("Instance not found")
}

#[tauri::command]
pub async fn list_shared_instances() -> AppResult<Vec<SharedInstance>> {
    Err(AppError::not_signed_in())
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
pub async fn share_instance(instance_id: Id, input: ShareInstanceInput) -> AppResult<SharedInstance> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn publish_instance(instance_id: Id, input: PublishInput) -> AppResult<InstanceSummary> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn preview_sync(instance_id: Id) -> AppResult<SyncPreview> {
    Err(no_instance())
}

#[tauri::command]
pub async fn sync_instance(instance_id: Id, options: SyncOptions) -> AppResult<InstanceSummary> {
    Err(no_instance())
}

#[tauri::command]
pub async fn revert_local_changes(instance_id: Id) -> AppResult<InstanceSummary> {
    Err(no_instance())
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
