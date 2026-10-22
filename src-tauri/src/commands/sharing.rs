use tauri::{AppHandle, Runtime, State};

use super::not_ready;
use crate::dto::{
    Id, InstanceSummary, InviteInput, InviteRole, PublishInput, PublishPlan, ShareInstanceInput,
    SharedInstance, SharedInstanceDetail, SyncOptions, SyncPreview, UpdateSharedInstanceInput,
};
use crate::error::AppResult;
use crate::services::{accept, instances, publish, shared, sync};
use crate::state::AppState;

#[tauri::command]
pub async fn list_shared_instances<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> AppResult<Vec<SharedInstance>> {
    shared::list(&state, &app).await
}

#[tauri::command]
pub async fn get_shared_instance(
    state: State<'_, AppState>,
    id: Id,
) -> AppResult<SharedInstanceDetail> {
    shared::get(&state, &id).await
}

/// Into a new instance in a launcher Atlas can create instances in. Returns once the
/// files start arriving (status "syncing").
#[tauri::command]
pub async fn accept_invite<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    shared_instance_id: Id,
    launcher_id: Id,
    install_optional: Vec<String>,
) -> AppResult<InstanceSummary> {
    accept::into_new(
        &state,
        &app,
        &shared_instance_id,
        &launcher_id,
        install_optional,
    )
    .await
}

/// Into an instance already tracked here (any launcher).
#[tauri::command]
pub async fn accept_invite_into<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    shared_instance_id: Id,
    instance_id: Id,
    install_optional: Vec<String>,
) -> AppResult<InstanceSummary> {
    accept::into_existing(
        &state,
        &app,
        &shared_instance_id,
        &instance_id,
        install_optional,
    )
    .await
}

#[tauri::command]
pub async fn decline_invite<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    shared_instance_id: Id,
) -> AppResult<()> {
    shared::decline(&state, &app, &shared_instance_id).await
}

#[tauri::command]
pub async fn prepare_publish(
    state: State<'_, AppState>,
    instance_id: Id,
) -> AppResult<PublishPlan> {
    publish::prepare(&state, &instance_id).await
}

#[tauri::command]
pub async fn share_instance<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
    input: ShareInstanceInput,
) -> AppResult<SharedInstance> {
    publish::share(&state, &app, &instance_id, input).await
}

#[tauri::command]
pub async fn publish_instance<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    instance_id: Id,
    input: PublishInput,
) -> AppResult<InstanceSummary> {
    publish::publish(&state, &app, &instance_id, input).await
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
pub async fn update_shared_instance<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
    patch: UpdateSharedInstanceInput,
) -> AppResult<SharedInstance> {
    shared::update(&state, &app, &id, patch).await
}

#[tauri::command]
pub async fn invite_to_shared_instance<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
    input: InviteInput,
) -> AppResult<SharedInstance> {
    shared::invite(&state, &app, &id, input).await
}

#[tauri::command]
pub async fn set_shared_member_role<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
    user_id: Id,
    role: InviteRole,
) -> AppResult<SharedInstance> {
    shared::set_role(&state, &app, &id, &user_id, role).await
}

#[tauri::command]
pub async fn remove_shared_member<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
    user_id: Id,
) -> AppResult<SharedInstance> {
    shared::remove_member(&state, &app, &id, &user_id).await
}

/// Groups aren't on the server yet.
#[tauri::command]
pub async fn unshare_from_group(
    state: State<'_, AppState>,
    #[allow(unused_variables)] id: Id,
    #[allow(unused_variables)] group_id: Id,
) -> AppResult<SharedInstance> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn transfer_shared_instance<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
    user_id: Id,
) -> AppResult<SharedInstance> {
    shared::transfer(&state, &app, &id, &user_id).await
}

#[tauri::command]
pub async fn leave_shared_instance<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
) -> AppResult<()> {
    shared::leave(&state, &app, &id).await
}

#[tauri::command]
pub async fn archive_shared_instance<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
) -> AppResult<()> {
    shared::archive(&state, &app, &id).await
}
