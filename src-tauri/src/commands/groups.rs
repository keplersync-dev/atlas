use tauri::{AppHandle, Runtime, State};

use crate::dto::{
    CreateGroupInput, CreateGroupInviteInput, Group, GroupDetail, GroupInvite, GroupInvitePreview,
    GroupMemberRole, Id, UpdateGroupInput,
};
use crate::error::AppResult;
use crate::services::groups;
use crate::state::AppState;

#[tauri::command]
pub async fn list_groups(state: State<'_, AppState>) -> AppResult<Vec<Group>> {
    groups::list(&state).await
}

#[tauri::command]
pub async fn get_group(state: State<'_, AppState>, id: Id) -> AppResult<GroupDetail> {
    groups::get(&state, &id).await
}

#[tauri::command]
pub async fn create_group<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    input: CreateGroupInput,
) -> AppResult<Group> {
    groups::create(&state, &app, input).await
}

#[tauri::command]
pub async fn update_group<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
    patch: UpdateGroupInput,
) -> AppResult<Group> {
    groups::update(&state, &app, &id, patch).await
}

#[tauri::command]
pub async fn delete_group<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: Id,
) -> AppResult<()> {
    groups::delete(&state, &app, &id).await
}

#[tauri::command]
pub async fn create_group_invite(
    state: State<'_, AppState>,
    group_id: Id,
    input: CreateGroupInviteInput,
) -> AppResult<GroupInvite> {
    groups::create_invite(&state, &group_id, input).await
}

#[tauri::command]
pub async fn revoke_group_invite(
    state: State<'_, AppState>,
    group_id: Id,
    code: String,
) -> AppResult<()> {
    groups::revoke_invite(&state, &group_id, &code).await
}

#[tauri::command]
pub async fn preview_group_invite(
    state: State<'_, AppState>,
    code: String,
) -> AppResult<GroupInvitePreview> {
    groups::preview_invite(&state, &code).await
}

#[tauri::command]
pub async fn join_group<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    code: String,
) -> AppResult<Group> {
    groups::join(&state, &app, &code).await
}

#[tauri::command]
pub async fn set_group_member_role<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    group_id: Id,
    user_id: Id,
    role: GroupMemberRole,
) -> AppResult<Group> {
    groups::set_role(&state, &app, &group_id, &user_id, role).await
}

#[tauri::command]
pub async fn remove_group_member<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    group_id: Id,
    user_id: Id,
) -> AppResult<()> {
    groups::remove_member(&state, &app, &group_id, &user_id).await
}

#[tauri::command]
pub async fn transfer_group<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    group_id: Id,
    user_id: Id,
) -> AppResult<Group> {
    groups::transfer(&state, &app, &group_id, &user_id).await
}
