use tauri::{AppHandle, Runtime, State};

use crate::dto::{
    CreateGroup, CreateGroupInvite, Group, GroupDetail, GroupInvite, GroupInvitePreview, Id,
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
    input: CreateGroup,
) -> AppResult<Group> {
    groups::create(&state, &app, input).await
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
    input: CreateGroupInvite,
) -> AppResult<GroupInvite> {
    groups::create_invite(&state, &group_id, input).await
}

#[tauri::command]
pub async fn preview_group_invite(
    state: State<'_, AppState>,
    code: String,
) -> AppResult<GroupInvitePreview> {
    groups::preview_invite(&state, &code).await
}

#[tauri::command]
pub async fn invite_friends_to_group<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    group_id: Id,
    friend_ids: Vec<Id>,
) -> AppResult<()> {
    groups::invite_friends(&state, &app, &group_id, friend_ids).await
}

#[tauri::command]
pub async fn list_group_invites(state: State<'_, AppState>) -> AppResult<Vec<GroupInvitePreview>> {
    groups::my_invites(&state).await
}

#[tauri::command]
pub async fn decline_group_invite<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    code: String,
) -> AppResult<()> {
    groups::decline_invite(&state, &app, &code).await
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
