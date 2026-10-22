//! Sharing an instance and publishing new versions of it: build a manifest from the
//! folder's files and their share settings, send it, then mark the files as published.
//! Nothing changes locally until the server has accepted it.

use std::collections::HashMap;

use reqwest::Method;
use tauri::{AppHandle, Runtime};

use crate::db::{self, content::ContentRow, instances::InstanceRow};
use crate::dto::{
    ContentKind, ContentSource, CreateRevision, CreateSharedInstance, InstanceSummary,
    ManualDownload, MemberRole, PublishInput, PublishItem, PublishPlan, RevisionSummary,
    ShareInstanceInput, ShareMode, SharedInstance, Side, Storage, from_wire, to_wire,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;
use crate::sync::adopt;
use crate::sync::manifest::{self, Entry, Manifest, MinecraftSpec, SCHEMA_VERSION};

async fn row(state: &AppState, id: &str) -> AppResult<InstanceRow> {
    db::instances::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))
}

fn mode_of(r: &ContentRow) -> ShareMode {
    from_wire(&r.share_mode).unwrap_or(ShareMode::Required)
}

/// Files that are there and switched on: the ones a publish looks at.
fn live(rows: &[ContentRow]) -> impl Iterator<Item = &ContentRow> {
    rows.iter().filter(|r| r.installed && r.enabled)
}

fn manual_download(r: &ContentRow) -> Option<ManualDownload> {
    let page = r.page_url.clone()?;
    (from_wire(&r.source) == Some(ContentSource::Curseforge) && r.download_url.is_none()).then(
        || ManualDownload {
            content_name: r.name.clone(),
            file_name: r.path.rsplit('/').next().unwrap_or(&r.path).to_owned(),
            page_url: page,
        },
    )
}

pub async fn prepare(state: &AppState, id: &str) -> AppResult<PublishPlan> {
    let row = row(state, id).await?;
    if row.shared_instance_id.is_some() && row.role() == Some(MemberRole::Viewer) {
        return Err(AppError::forbidden(
            "Only the owner and editors can publish",
        ));
    }
    let rows = db::content::list(&state.db, id).await?;
    let items = live(&rows)
        .map(|r| {
            let dto = r.to_dto();
            PublishItem {
                content_id: r.id.clone(),
                name: r.name.clone(),
                icon_url: dto.icon_url,
                kind: dto.kind,
                source: dto.source,
                path: r.path.clone(),
                size_bytes: r.size_bytes.max(0) as u64,
                is_new: r.published_share_mode.is_none(),
                share_mode: mode_of(r),
                published_share_mode: r.published_share_mode.as_deref().and_then(from_wire),
                needs_upload: dto.source == ContentSource::Local,
                already_uploaded: false,
            }
        })
        .collect();
    let mut removed: Vec<String> = rows
        .iter()
        .filter(|r| r.is_managed && r.installed && !r.enabled)
        .map(|r| r.name.clone())
        .collect();
    if let Some(synced) = row.synced_manifest() {
        for e in &synced.content {
            if e.required && !rows.iter().any(|r| r.path.eq_ignore_ascii_case(&e.path)) {
                removed.push(e.name.clone());
            }
        }
    }
    let storage = state
        .cloud
        .session()
        .await?
        .user
        .map(|me| me.storage)
        .unwrap_or(Storage {
            used_bytes: 0,
            quota_bytes: 0,
            max_file_bytes: 0,
        });
    Ok(PublishPlan {
        base_revision: row.synced_revision.map(|r| r.max(0) as u32),
        items,
        removed,
        manual_downloads: live(&rows)
            .filter(|r| mode_of(r) != ShareMode::Private)
            .filter_map(manual_download)
            .collect(),
        storage,
    })
}

/// The settings that will be published: each file's own, overridden by the dialog's.
fn modes(rows: &[ContentRow], chosen: &HashMap<String, ShareMode>) -> HashMap<String, ShareMode> {
    rows.iter()
        .map(|r| {
            (
                r.id.clone(),
                chosen.get(&r.id).copied().unwrap_or(mode_of(r)),
            )
        })
        .collect()
}

/// The manifest for these files and settings. Files only on this PC can't be shared yet
/// (uploading comes later), so those must be set to Only me.
fn build(
    row: &InstanceRow,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
    description: Option<String>,
) -> AppResult<Manifest> {
    let (Some(version), Some(loader)) = (row.minecraft_version.clone(), row.loader()) else {
        return Err(AppError::invalid_input(
            "Atlas couldn't tell which Minecraft version and loader this folder uses",
        ));
    };
    let mut content = Vec::new();
    for r in live(rows) {
        let mode = modes.get(&r.id).copied().unwrap_or(ShareMode::Required);
        if mode == ShareMode::Private {
            continue;
        }
        let source = from_wire(&r.source).unwrap_or(ContentSource::Local);
        if source == ContentSource::Local || source == ContentSource::Upload {
            return Err(AppError::invalid_input(format!(
                "{} isn't on Modrinth or CurseForge. Uploading files like it isn't available yet, so set it to Only me for now.",
                r.name
            )));
        }
        content.push(Entry {
            kind: from_wire(&r.kind).unwrap_or(ContentKind::Mod),
            source,
            project_id: r.project_id.clone(),
            version_id: r.version_id.clone(),
            name: r.name.clone(),
            version_label: r.version_label.clone(),
            path: r.path.clone(),
            sha1: r.sha1.clone(),
            sha256: r.sha256.clone(),
            size_bytes: r.size_bytes.max(0) as u64,
            download_url: r.download_url.clone(),
            page_url: r.page_url.clone(),
            side: from_wire(&r.side).unwrap_or(Side::Both),
            required: mode == ShareMode::Required,
        });
    }
    let m = Manifest {
        schema_version: SCHEMA_VERSION,
        name: row.name.clone(),
        description,
        icon: None,
        minecraft: MinecraftSpec {
            version,
            loader,
            loader_version: row.loader_version.clone(),
        },
        recommended_memory_mb: None,
        content,
        overrides: Vec::new(),
    };
    manifest::validate(&m).map_err(|e| {
        tracing::warn!(error = %e, "built a manifest that doesn't validate");
        AppError::invalid_input(format!("Atlas can't share this instance: {e}"))
    })?;
    Ok(m)
}

/// After the server took a version: save the settings, mark what's published, link it.
async fn apply_published(
    state: &AppState,
    row: &InstanceRow,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
    revision: u32,
    m: &Manifest,
) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    for r in rows {
        if let Some(mode) = modes.get(&r.id) {
            sqlx::query("UPDATE instance_content SET share_mode = ? WHERE id = ?")
                .bind(to_wire(*mode))
                .bind(&r.id)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    adopt::adopt(&state.db, &row.id, revision, m).await?;
    // "Only me" files are recorded as published that way, so they don't count as new
    // files to publish next time.
    for r in live(rows) {
        if modes.get(&r.id) == Some(&ShareMode::Private) {
            sqlx::query(
                "UPDATE instance_content SET is_managed = 0, published_share_mode = 'private'
                 WHERE id = ?",
            )
            .bind(&r.id)
            .execute(&state.db)
            .await?;
        }
    }
    Ok(())
}

pub async fn share<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: ShareInstanceInput,
) -> AppResult<SharedInstance> {
    let row = row(state, id).await?;
    if row.shared_instance_id.is_some() {
        return Err(AppError::conflict("This instance is already shared"));
    }
    if !input.group_ids.is_empty() {
        return Err(AppError::invalid_input(
            "Sharing with groups isn't available yet",
        ));
    }
    let rows = db::content::list(&state.db, id).await?;
    let modes = modes(&rows, &input.share_modes);
    let description = input.description.flatten();
    let m = build(&row, &rows, &modes, description.clone())?;
    let body = CreateSharedInstance {
        name: row.name.clone(),
        description,
        manifest: m.clone(),
        friend_ids: input.friend_ids,
        group_ids: Vec::new(),
        role: input.role,
    };
    let shared: SharedInstance = state
        .cloud
        .call(Method::POST, "/shared-instances", Some(&body))
        .await?;

    db::instances::set_link(&state.db, id, Some((&shared.id, MemberRole::Owner))).await?;
    apply_published(state, &row, &rows, &modes, shared.latest_revision, &m).await?;
    sqlx::query("UPDATE instance SET sync_report_due = 0 WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    super::shared::changed(state, app, &shared.id).await;
    let mut out = shared;
    out.local_instance_id = Some(id.to_owned());
    Ok(out)
}

pub async fn publish<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: PublishInput,
) -> AppResult<InstanceSummary> {
    let row = row(state, id).await?;
    let shared = super::shared::server_id(&row)
        .ok_or_else(|| AppError::conflict("This instance isn't shared"))?
        .to_owned();
    if !matches!(row.role(), Some(MemberRole::Owner | MemberRole::Editor)) {
        return Err(AppError::forbidden(
            "Only the owner and editors can publish",
        ));
    }
    let base = row
        .synced_revision
        .ok_or_else(|| AppError::conflict("Update to the latest version first"))?
        .max(0) as u32;
    let rows = db::content::list(&state.db, id).await?;
    let modes = modes(&rows, &input.share_modes);
    let description = row.synced_manifest().and_then(|m| m.description);
    let m = build(&row, &rows, &modes, description)?;
    if row.synced_manifest_hash.as_deref() == Some(manifest::hash(&m).as_str()) {
        return Err(AppError::invalid_input("Nothing to publish"));
    }
    let body = CreateRevision {
        base_revision: base,
        manifest: m.clone(),
        changelog: input.changelog,
    };
    let path = format!(
        "/shared-instances/{}/revisions",
        super::shared::segment(&shared)?
    );
    let published: RevisionSummary = state.cloud.call(Method::POST, &path, Some(&body)).await?;

    apply_published(state, &row, &rows, &modes, published.revision, &m).await?;
    sqlx::query("UPDATE instance SET sync_report_due = 0 WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    super::shared::changed(state, app, &shared).await;
    emit(
        app,
        AtlasEvent::InstanceChanged {
            instance_id: id.to_owned(),
        },
    );
    super::instances::summary_by_id(state, id).await
}
