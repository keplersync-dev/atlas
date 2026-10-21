// Not built yet: these need the Atlas server.
#![allow(unused_variables)]

use crate::dto::{
    CreateGroupInput, CreateGroupInviteInput, Group, GroupDetail, GroupInvite, GroupInvitePreview,
    GroupMemberRole, Id, UpdateGroupInput,
};
use crate::error::{AppError, AppResult};

#[tauri::command]
pub async fn list_groups() -> AppResult<Vec<Group>> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn get_group(id: Id) -> AppResult<GroupDetail> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn create_group(input: CreateGroupInput) -> AppResult<Group> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn update_group(id: Id, patch: UpdateGroupInput) -> AppResult<Group> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn delete_group(id: Id) -> AppResult<()> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn create_group_invite(
    group_id: Id,
    input: CreateGroupInviteInput,
) -> AppResult<GroupInvite> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn revoke_group_invite(group_id: Id, code: String) -> AppResult<()> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn preview_group_invite(code: String) -> AppResult<GroupInvitePreview> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn join_group(code: String) -> AppResult<Group> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn set_group_member_role(
    group_id: Id,
    user_id: Id,
    role: GroupMemberRole,
) -> AppResult<Group> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn remove_group_member(group_id: Id, user_id: Id) -> AppResult<()> {
    Err(AppError::not_signed_in())
}

#[tauri::command]
pub async fn transfer_group(group_id: Id, user_id: Id) -> AppResult<Group> {
    Err(AppError::not_signed_in())
}
