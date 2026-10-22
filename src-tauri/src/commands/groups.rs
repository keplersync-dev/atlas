// Not built yet: the server has no groups. Signed in, these say so; signed out, they
// ask you to sign in.
#![allow(unused_variables)]

use super::not_ready;
use crate::dto::{
    CreateGroupInput, CreateGroupInviteInput, Group, GroupDetail, GroupInvite, GroupInvitePreview,
    GroupMemberRole, Id, UpdateGroupInput,
};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use tauri::State;

/// Empty while signed in: groups aren't on the server yet.
#[tauri::command]
pub async fn list_groups(state: State<'_, AppState>) -> AppResult<Vec<Group>> {
    if !state.cloud.is_signed_in() {
        return Err(AppError::not_signed_in());
    }
    Ok(Vec::new())
}

#[tauri::command]
pub async fn get_group(state: State<'_, AppState>, id: Id) -> AppResult<GroupDetail> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn create_group(state: State<'_, AppState>, input: CreateGroupInput) -> AppResult<Group> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn update_group(
    state: State<'_, AppState>,
    id: Id,
    patch: UpdateGroupInput,
) -> AppResult<Group> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn delete_group(state: State<'_, AppState>, id: Id) -> AppResult<()> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn create_group_invite(
    state: State<'_, AppState>,
    group_id: Id,
    input: CreateGroupInviteInput,
) -> AppResult<GroupInvite> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn revoke_group_invite(
    state: State<'_, AppState>,
    group_id: Id,
    code: String,
) -> AppResult<()> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn preview_group_invite(
    state: State<'_, AppState>,
    code: String,
) -> AppResult<GroupInvitePreview> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn join_group(state: State<'_, AppState>, code: String) -> AppResult<Group> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn set_group_member_role(
    state: State<'_, AppState>,
    group_id: Id,
    user_id: Id,
    role: GroupMemberRole,
) -> AppResult<Group> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn remove_group_member(
    state: State<'_, AppState>,
    group_id: Id,
    user_id: Id,
) -> AppResult<()> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}

#[tauri::command]
pub async fn transfer_group(
    state: State<'_, AppState>,
    group_id: Id,
    user_id: Id,
) -> AppResult<Group> {
    Err(not_ready(
        &state,
        "Groups aren't available yet. They're coming in a later update.",
    ))
}
