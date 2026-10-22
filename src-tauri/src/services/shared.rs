//! Shared instances from the server: the list (cached for offline use and for summaries'
//! latest revision), membership changes, and keeping local instances in step: unlink
//! one when its shared instance is gone, report which revision a folder is on.

use reqwest::Method;
use tauri::{AppHandle, Runtime};

use crate::db::{self, instances::InstanceRow};
use crate::dto::{
    AddMembers, Id, InviteInput, InviteRole, MemberStatus, SetMemberRole, SharedInstance,
    SharedInstanceDetail, Synced, Transfer, UpdateSharedInstance, UpdateSharedInstanceInput,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;

/// Where the last fetched list lives in `cloud_cache`.
pub const CACHE_KEY: &str = "shared_instances";

/// The last fetched copy of one shared instance (no network).
pub async fn cached(state: &AppState, id: &str) -> AppResult<Option<SharedInstance>> {
    let list: Option<Vec<SharedInstance>> = db::cloud_cache::get(&state.db, CACHE_KEY).await?;
    Ok(list.and_then(|l| l.into_iter().find(|s| s.id == id)))
}

/// Shared instances on the server, linked to their local copies. When the list is fresh,
/// local copies of instances that are gone (archived, or access lost) become local-only,
/// and owed sync reports are sent.
pub async fn list<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
) -> AppResult<Vec<SharedInstance>> {
    let mut list: Vec<SharedInstance> = state
        .cloud
        .cached_get(CACHE_KEY, "/shared-instances")
        .await?;
    let rows = db::instances::list(&state.db).await?;
    if state.cloud.is_online() {
        for row in &rows {
            let Some(shared) = server_id(row) else {
                continue;
            };
            if !list
                .iter()
                .any(|s| s.id == shared && s.my_status == MemberStatus::Active)
            {
                tracing::info!(instance = %row.id, "shared instance gone; now local-only");
                unlink(state, app, &row.id).await?;
            } else if row.sync_report_due {
                report_synced(state, row).await;
            }
        }
    }
    let rows = db::instances::list(&state.db).await?;
    list.retain(|s| s.my_status != MemberStatus::Declined);
    for s in &mut list {
        fill_local(s, &rows);
    }
    Ok(list)
}

pub async fn get(state: &AppState, id: &Id) -> AppResult<SharedInstanceDetail> {
    let path = format!("/shared-instances/{}", segment(id)?);
    let mut detail: SharedInstanceDetail =
        state.cloud.call(Method::GET, &path, None::<&()>).await?;
    fill_local(&mut detail.shared, &db::instances::list(&state.db).await?);
    Ok(detail)
}

pub async fn decline<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &Id) -> AppResult<()> {
    let path = format!("/shared-instances/{}/decline", segment(id)?);
    state
        .cloud
        .call::<()>(Method::POST, &path, None::<&()>)
        .await?;
    changed(state, app, id).await;
    Ok(())
}

pub async fn update<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &Id,
    patch: UpdateSharedInstanceInput,
) -> AppResult<SharedInstance> {
    let body = UpdateSharedInstance {
        name: patch.name,
        description: patch.description,
    };
    let path = format!("/shared-instances/{}", segment(id)?);
    let shared = state.cloud.call(Method::PATCH, &path, Some(&body)).await?;
    finish(state, app, id, shared).await
}

pub async fn invite<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &Id,
    input: InviteInput,
) -> AppResult<SharedInstance> {
    if !input.group_ids.is_empty() {
        return Err(AppError::invalid_input(
            "Sharing with groups isn't available yet",
        ));
    }
    let body = AddMembers {
        user_ids: input.friend_ids,
        role: input.role,
    };
    let path = format!("/shared-instances/{}/members", segment(id)?);
    let shared = state.cloud.call(Method::POST, &path, Some(&body)).await?;
    finish(state, app, id, shared).await
}

pub async fn set_role<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &Id,
    user_id: &Id,
    role: InviteRole,
) -> AppResult<SharedInstance> {
    let path = format!(
        "/shared-instances/{}/members/{}",
        segment(id)?,
        segment(user_id)?
    );
    let shared = state
        .cloud
        .call(Method::PATCH, &path, Some(&SetMemberRole { role }))
        .await?;
    finish(state, app, id, shared).await
}

pub async fn remove_member<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &Id,
    user_id: &Id,
) -> AppResult<SharedInstance> {
    let path = format!(
        "/shared-instances/{}/members/{}",
        segment(id)?,
        segment(user_id)?
    );
    state
        .cloud
        .call::<()>(Method::DELETE, &path, None::<&()>)
        .await?;
    let detail = get(state, id).await?;
    finish(state, app, id, detail.shared).await
}

pub async fn transfer<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &Id,
    user_id: &Id,
) -> AppResult<SharedInstance> {
    let path = format!("/shared-instances/{}/transfer", segment(id)?);
    let body = Transfer {
        user_id: user_id.clone(),
    };
    let shared = state.cloud.call(Method::POST, &path, Some(&body)).await?;
    finish(state, app, id, shared).await
}

/// Leave it. Your copy stays, as a local-only instance.
pub async fn leave<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &Id) -> AppResult<()> {
    let me = state
        .cloud
        .session()
        .await?
        .user
        .ok_or_else(AppError::not_signed_in)?;
    let path = format!(
        "/shared-instances/{}/members/{}",
        segment(id)?,
        segment(&me.user.id)?
    );
    state
        .cloud
        .call::<()>(Method::DELETE, &path, None::<&()>)
        .await?;
    unlink_shared(state, app, id).await?;
    changed(state, app, id).await;
    Ok(())
}

/// Archive it for everyone. Every member's copy (yours too) stays as a local instance.
pub async fn archive<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &Id) -> AppResult<()> {
    let path = format!("/shared-instances/{}", segment(id)?);
    state
        .cloud
        .call::<()>(Method::DELETE, &path, None::<&()>)
        .await?;
    unlink_shared(state, app, id).await?;
    changed(state, app, id).await;
    Ok(())
}

/// Tell the server which revision this folder is on. Kept as owed if it can't be sent.
pub async fn report_synced(state: &AppState, row: &InstanceRow) {
    let (Some(shared), Some(revision)) = (server_id(row), row.synced_revision) else {
        return;
    };
    let path = format!("/shared-instances/{shared}/synced");
    let body = Synced {
        revision: revision.max(0) as u32,
    };
    let sent = state
        .cloud
        .call::<()>(Method::PUT, &path, Some(&body))
        .await;
    let due = match &sent {
        Ok(()) => false,
        Err(e) => {
            tracing::info!(instance = %row.id, error = %e, "sync report not sent yet");
            true
        }
    };
    let saved = sqlx::query("UPDATE instance SET sync_report_due = ? WHERE id = ?")
        .bind(due)
        .bind(&row.id)
        .execute(&state.db)
        .await;
    if let Err(e) = saved {
        tracing::warn!(error = %e, "couldn't record the sync report");
    }
}

/// Refresh the cached list (so summaries see the new latest revision) and tell the UI.
pub async fn changed<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) {
    if let Err(e) = state
        .cloud
        .cached_get::<Vec<SharedInstance>>(CACHE_KEY, "/shared-instances")
        .await
    {
        tracing::info!(error = %e, "couldn't refresh shared instances");
    }
    emit(
        app,
        AtlasEvent::SharedChanged {
            shared_instance_id: id.to_owned(),
        },
    );
    if let Ok(rows) = db::instances::list(&state.db).await {
        for row in rows.iter().filter(|r| server_id(r) == Some(id)) {
            emit(
                app,
                AtlasEvent::InstanceChanged {
                    instance_id: row.id.clone(),
                },
            );
        }
    }
}

async fn finish<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    mut shared: SharedInstance,
) -> AppResult<SharedInstance> {
    changed(state, app, id).await;
    fill_local(&mut shared, &db::instances::list(&state.db).await?);
    Ok(shared)
}

async fn unlink_shared<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
) -> AppResult<()> {
    for row in db::instances::list(&state.db).await? {
        if server_id(&row) == Some(id) {
            unlink(state, app, &row.id).await?;
        }
    }
    Ok(())
}

async fn unlink<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    instance_id: &str,
) -> AppResult<()> {
    super::sync::unlink(state, instance_id).await?;
    emit(
        app,
        AtlasEvent::InstanceChanged {
            instance_id: instance_id.to_owned(),
        },
    );
    Ok(())
}

fn fill_local(shared: &mut SharedInstance, rows: &[InstanceRow]) {
    shared.local_instance_id = rows
        .iter()
        .find(|r| server_id(r) == Some(shared.id.as_str()))
        .map(|r| r.id.clone());
}

/// The server's id for an instance's shared instance (dev-folder links have none).
pub fn server_id(row: &InstanceRow) -> Option<&str> {
    row.shared_instance_id
        .as_deref()
        .filter(|id| !id.starts_with("dev:"))
}

/// An id as a path segment. Ids are UUIDs; anything else is refused before it can change
/// the path.
pub fn segment(id: &str) -> AppResult<&str> {
    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        Ok(id)
    } else {
        Err(AppError::not_found("That shared instance isn't available"))
    }
}
