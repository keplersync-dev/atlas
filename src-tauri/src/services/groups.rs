//! Groups. The list comes from the server and falls back to the offline cache; changes
//! need the server. After a change the UI hears `groups.changed`; when shared instances
//! come or go with it, the server sends `shared.changed` too.

use atlas_protocol::api::{CreateGroup, CreateGroupInvite, SetGroupRole, Transfer, UpdateGroup};
use atlas_protocol::codes::{self, GROUP_CODE_LEN};
use reqwest::Method;
use tauri::{AppHandle, Runtime};

use crate::dto::{
    CreateGroupInput, CreateGroupInviteInput, Group, GroupDetail, GroupInvite, GroupInvitePreview,
    GroupMemberRole, Id, UpdateGroupInput,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;

pub const CACHE_KEY: &str = "groups";

pub async fn list(state: &AppState) -> AppResult<Vec<Group>> {
    state.cloud.cached_get(CACHE_KEY, "/groups").await
}

/// An id as a path segment. Ids are UUIDs; anything else is refused before it can change
/// the path.
fn segment(id: &str) -> AppResult<&str> {
    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        Ok(id)
    } else {
        Err(AppError::not_found("That group isn't available"))
    }
}

fn group_path(id: &str) -> AppResult<String> {
    Ok(format!("/groups/{}", segment(id)?))
}

/// An invite code from what someone typed or pasted: the code ("K3M9P-Q2XRT", any case)
/// or a whole invite link (its last part is the code).
pub fn invite_code(input: &str) -> AppResult<String> {
    let last = input
        .trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    codes::normalize(last, GROUP_CODE_LEN).ok_or_else(|| {
        AppError::invalid_input("Group invites are 10 letters and numbers, or an invite link")
    })
}

fn changed<R: Runtime>(app: &AppHandle<R>, id: Option<&str>) {
    emit(
        app,
        AtlasEvent::GroupsChanged {
            group_id: id.map(str::to_owned),
        },
    );
}

pub async fn get(state: &AppState, id: &Id) -> AppResult<GroupDetail> {
    state
        .cloud
        .call(Method::GET, &group_path(id)?, None::<&()>)
        .await
}

pub async fn create<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    input: CreateGroupInput,
) -> AppResult<Group> {
    let body = CreateGroup {
        name: input.name,
        icon: input.icon.flatten(),
    };
    let group: Group = state
        .cloud
        .call(Method::POST, "/groups", Some(&body))
        .await?;
    changed(app, Some(&group.id));
    Ok(group)
}

pub async fn update<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &Id,
    patch: UpdateGroupInput,
) -> AppResult<Group> {
    let body = UpdateGroup {
        name: patch.name,
        icon: patch.icon,
    };
    let group = state
        .cloud
        .call(Method::PATCH, &group_path(id)?, Some(&body))
        .await?;
    changed(app, Some(id));
    Ok(group)
}

pub async fn delete<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &Id) -> AppResult<()> {
    state
        .cloud
        .call::<()>(Method::DELETE, &group_path(id)?, None::<&()>)
        .await?;
    changed(app, Some(id));
    Ok(())
}

pub async fn create_invite(
    state: &AppState,
    group_id: &Id,
    input: CreateGroupInviteInput,
) -> AppResult<GroupInvite> {
    let body = CreateGroupInvite {
        max_uses: input.max_uses,
        expires_in_hours: input.expires_in_hours,
    };
    let path = format!("{}/invites", group_path(group_id)?);
    state.cloud.call(Method::POST, &path, Some(&body)).await
}

pub async fn revoke_invite(state: &AppState, group_id: &Id, code: &str) -> AppResult<()> {
    let path = format!("{}/invites/{}", group_path(group_id)?, invite_code(code)?);
    state
        .cloud
        .call::<()>(Method::DELETE, &path, None::<&()>)
        .await
}

pub async fn preview_invite(state: &AppState, code: &str) -> AppResult<GroupInvitePreview> {
    let path = format!("/group-invites/{}", invite_code(code)?);
    state.cloud.call(Method::GET, &path, None::<&()>).await
}

/// Joining can bring shared instances with it: the group's invites.
pub async fn join<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    code: &str,
) -> AppResult<Group> {
    let path = format!("/group-invites/{}/accept", invite_code(code)?);
    let group: Group = state.cloud.call(Method::POST, &path, None::<&()>).await?;
    changed(app, Some(&group.id));
    Ok(group)
}

pub async fn set_role<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    group_id: &Id,
    user_id: &Id,
    role: GroupMemberRole,
) -> AppResult<Group> {
    let path = format!("{}/members/{}", group_path(group_id)?, segment(user_id)?);
    let group = state
        .cloud
        .call(Method::PATCH, &path, Some(&SetGroupRole { role }))
        .await?;
    changed(app, Some(group_id));
    Ok(group)
}

/// Remove someone, or leave (your own id). Leaving can take shared instances away.
pub async fn remove_member<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    group_id: &Id,
    user_id: &Id,
) -> AppResult<()> {
    let path = format!("{}/members/{}", group_path(group_id)?, segment(user_id)?);
    state
        .cloud
        .call::<()>(Method::DELETE, &path, None::<&()>)
        .await?;
    changed(app, Some(group_id));
    Ok(())
}

pub async fn transfer<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    group_id: &Id,
    user_id: &Id,
) -> AppResult<Group> {
    let path = format!("{}/transfer", group_path(group_id)?);
    let body = Transfer {
        user_id: user_id.clone(),
    };
    let group = state.cloud.call(Method::POST, &path, Some(&body)).await?;
    changed(app, Some(group_id));
    Ok(group)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_codes_and_links() {
        for input in [
            "K3M9PQ2XRT",
            "k3m9p-q2xrt",
            " https://atlas.example/g/K3M9PQ2XRT ",
            "atlas://group/K3M9PQ2XRT/",
        ] {
            assert_eq!(super::invite_code(input).unwrap(), "K3M9PQ2XRT", "{input}");
        }
        assert!(super::invite_code("https://atlas.example/g/").is_err());
        assert!(super::invite_code("7KQ2-9XMB").is_err(), "a friend code");
    }
}
