//! Shared instances from the server: the list (cached for offline use and for summaries'
//! latest revision), membership changes, and keeping local instances in step: unlink
//! one when its shared instance is gone, report which revision a folder is on.

use std::path::Path;
use std::time::Duration;

use reqwest::Method;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Runtime};

use crate::db::{self, instances::InstanceRow};
use crate::dto::{
    AddMembers, Id, InviteInput, InviteRole, MemberStatus, SetMemberRole, ShareToGroup,
    SharedInstance, SharedInstanceDetail, Synced, Transfer, UpdateSharedInstance,
    UpdateSharedInstanceInput,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;
use crate::sync::manifest::MAX_ICON_BYTES;

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
    show_icons(state, &mut list).await;
    Ok(list)
}

pub async fn get(state: &AppState, id: &Id) -> AppResult<SharedInstanceDetail> {
    let path = format!("/shared-instances/{}", segment(id)?);
    let mut detail: SharedInstanceDetail =
        state.cloud.call(Method::GET, &path, None::<&()>).await?;
    fill_local(&mut detail.shared, &db::instances::list(&state.db).await?);
    show_icons(state, std::slice::from_mut(&mut detail.shared)).await;
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
    let mut shared: Option<SharedInstance> = None;
    if !input.friend_ids.is_empty() {
        let body = AddMembers {
            user_ids: input.friend_ids,
            role: input.role,
        };
        let path = format!("/shared-instances/{}/members", segment(id)?);
        shared = Some(state.cloud.call(Method::POST, &path, Some(&body)).await?);
    }
    for group_id in input.group_ids {
        let body = ShareToGroup {
            group_id,
            role: input.role,
        };
        let path = format!("/shared-instances/{}/groups", segment(id)?);
        shared = Some(state.cloud.call(Method::POST, &path, Some(&body)).await?);
    }
    let Some(shared) = shared else {
        return Err(AppError::invalid_input("Pick someone to share it with"));
    };
    finish(state, app, id, shared).await
}

/// Stop sharing with a group. Members who only had it through the group lose it.
pub async fn unshare_from_group<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &Id,
    group_id: &Id,
) -> AppResult<SharedInstance> {
    let path = format!(
        "/shared-instances/{}/groups/{}",
        segment(id)?,
        segment(group_id)?
    );
    let shared = state.cloud.call(Method::DELETE, &path, None::<&()>).await?;
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
    show_icons(state, std::slice::from_mut(&mut shared)).await;
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

/// Shared pictures come from the server as "sha256:<hex>". Show the copy in the icon
/// cache, fetching it first when online. No picture, or one that can't be fetched, leaves
/// `icon` empty and the UI draws a block.
async fn show_icons(state: &AppState, list: &mut [SharedInstance]) {
    let online = state.cloud.is_online();
    let wanted: Vec<Option<String>> = list
        .iter()
        .map(|s| icon_sha256(s).map(str::to_owned))
        .collect();
    let found = futures_util::future::join_all(list.iter().zip(&wanted).map(|(s, sha)| async {
        let sha = sha.as_deref()?;
        let file = crate::scan::icons::shared_icon_file(&state.paths.icons, sha);
        if !file.is_file()
            && online
            && let Err(e) = fetch_icon(state, &s.id, sha, &file).await
        {
            tracing::info!(shared = %s.id, error = %e, "couldn't fetch a shared picture");
        }
        file.is_file()
            .then(|| crate::util::asset::url(&format!("file:{}", file.display())))
    }))
    .await;
    for (s, icon) in list.iter_mut().zip(found) {
        s.icon = icon;
    }
}

/// The picture's sha256, as the server sends it: "sha256:<hex>".
fn icon_sha256(s: &SharedInstance) -> Option<&str> {
    s.icon
        .as_deref()
        .and_then(|i| i.strip_prefix("sha256:"))
        .filter(|sha| sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The picture of a shared instance as the server sends it, from the icon cache (fetched
/// first if needed). None when it has none or it can't be fetched.
pub async fn icon_bytes(state: &AppState, shared: &SharedInstance) -> Option<Vec<u8>> {
    let sha = icon_sha256(shared)?;
    let file = crate::scan::icons::shared_icon_file(&state.paths.icons, sha);
    if !file.is_file()
        && let Err(e) = fetch_icon(state, &shared.id, sha, &file).await
    {
        tracing::info!(shared = %shared.id, error = %e, "couldn't fetch a shared picture");
    }
    tokio::fs::read(&file).await.ok()
}

async fn fetch_icon(state: &AppState, shared: &str, sha256: &str, file: &Path) -> AppResult<()> {
    let url = state.cloud.file_url(shared, sha256).await?;
    let get = state.cloud.file_http().get(url).send();
    let res = tokio::time::timeout(Duration::from_secs(10), get)
        .await
        .map_err(|_| AppError::offline())?
        .and_then(|r| r.error_for_status())
        .map_err(|_| AppError::offline())?;
    let bytes = res.bytes().await.map_err(|_| AppError::offline())?;
    if bytes.len() as u64 > MAX_ICON_BYTES
        || hex::encode(Sha256::digest(&bytes)) != sha256
        || !crate::scan::icons::is_image(&bytes)
    {
        return Err(AppError::unknown("not the picture it should be"));
    }
    let tmp = file.with_extension("part");
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, file).await?;
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
