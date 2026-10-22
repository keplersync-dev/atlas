//! Sharing an instance and publishing new versions of it: upload the files that aren't
//! on a mod site and the shared config files, build a manifest from the folder's files
//! and their share settings, send it, then mark the files as published. Nothing changes
//! locally until the server has accepted it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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
use crate::sync::manifest::{
    self, Entry, MAX_OVERRIDES, MAX_OVERRIDES_TOTAL, Manifest, MinecraftSpec, Override,
    SCHEMA_VERSION,
};

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

/// On no mod site: shared means uploaded to Atlas.
fn is_upload(r: &ContentRow) -> bool {
    matches!(
        from_wire(&r.source),
        Some(ContentSource::Local | ContentSource::Upload) | None
    )
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
    let to_check: Vec<String> = live(&rows)
        .filter(|r| is_upload(r))
        .map(|r| r.sha256.clone())
        .collect();
    let stored: HashSet<String> = match state.cloud.stored(to_check).await {
        Ok(found) => found.into_iter().collect(),
        Err(e) => {
            tracing::info!(error = %e, "couldn't check which files are uploaded");
            HashSet::new()
        }
    };
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
                needs_upload: is_upload(r),
                already_uploaded: stored.contains(&r.sha256),
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
    let storage = match state
        .cloud
        .call(Method::GET, "/me/storage", None::<&()>)
        .await
    {
        Ok(storage) => storage,
        Err(_) => state
            .cloud
            .session()
            .await?
            .user
            .map(|me| me.storage)
            .unwrap_or(Storage {
                used_bytes: 0,
                quota_bytes: 0,
                max_file_bytes: 0,
            }),
    };
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

/// Config files to share: those matching the instance's `sync_include` patterns
/// ("config/**" by default) that the rules allow, up to the count and size limits.
async fn collect_configs(
    state: &AppState,
    row: &InstanceRow,
) -> AppResult<Vec<(Override, PathBuf)>> {
    let include: Option<String> =
        sqlx::query_scalar("SELECT sync_include FROM instance WHERE id = ?")
            .bind(&row.id)
            .fetch_optional(&state.db)
            .await?;
    let patterns: Vec<String> = include
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_else(|| vec!["config/**".to_owned()]);
    let game = PathBuf::from(&row.game_dir);
    tokio::task::spawn_blocking(move || configs_in(&game, &patterns))
        .await
        .map_err(|e| AppError::unknown(e.to_string()))?
}

fn configs_in(game: &Path, patterns: &[String]) -> AppResult<Vec<(Override, PathBuf)>> {
    let mut files = Vec::new();
    for pattern in patterns {
        match pattern.strip_suffix("/**") {
            Some(dir) => walk(&game.join(dir), dir, &mut files),
            None => {
                let file = game.join(pattern);
                if file.is_file() {
                    files.push((pattern.clone(), file));
                }
            }
        }
    }
    files.sort();
    files.dedup();
    let (mut out, mut total, mut skipped) = (Vec::new(), 0u64, 0usize);
    for (rel, file) in files {
        let size = std::fs::metadata(&file)
            .map(|m| m.len())
            .unwrap_or(u64::MAX);
        if !manifest::is_shareable_config(&rel, size)
            || out.len() >= MAX_OVERRIDES
            || total + size > MAX_OVERRIDES_TOTAL
        {
            skipped += 1;
            continue;
        }
        let hashes = crate::scan::hash::hash_file(&file)?;
        total += size;
        out.push((
            Override {
                path: rel,
                sha256: hashes.sha256,
                size_bytes: size,
            },
            file,
        ));
    }
    if skipped > 0 {
        tracing::info!(
            skipped,
            "config files left out of the share (format, size or limits)"
        );
    }
    Ok(out)
}

/// Every file under `dir`, with paths relative to the game folder ("config/a/b.json").
fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let child = format!("{rel}/{name}");
        if path.is_dir() {
            walk(&path, &child, out);
        } else if path.is_file() {
            out.push((child, path));
        }
    }
}

/// Upload what the version needs that the server doesn't have yet: files on no mod site
/// that are shared, and the config files. Returns the configs as overrides.
async fn send_files(
    state: &AppState,
    row: &InstanceRow,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
    confirmed_rights: bool,
) -> AppResult<Vec<Override>> {
    let uploads: Vec<&ContentRow> = live(rows)
        .filter(|r| modes.get(&r.id) != Some(&ShareMode::Private) && is_upload(r))
        .collect();
    if !uploads.is_empty() && !confirmed_rights {
        return Err(AppError::invalid_input(
            "Confirm you have the right to share the files being uploaded",
        ));
    }
    let storage: Storage = state
        .cloud
        .call(Method::GET, "/me/storage", None::<&()>)
        .await?;
    if let Some(big) = uploads
        .iter()
        .find(|r| r.size_bytes.max(0) as u64 > storage.max_file_bytes)
    {
        return Err(AppError::new(
            crate::error::ErrorCode::QuotaExceeded,
            format!(
                "{} is larger than the per-file limit. Set it to Only me.",
                big.name
            ),
        ));
    }
    let configs = collect_configs(state, row).await?;

    let game = PathBuf::from(&row.game_dir);
    let mut wanted: Vec<(PathBuf, String, u64, String)> = uploads
        .iter()
        .map(|r| {
            (
                game.join(&r.path),
                r.sha256.clone(),
                r.size_bytes.max(0) as u64,
                r.name.clone(),
            )
        })
        .chain(
            configs
                .iter()
                .map(|(o, file)| (file.clone(), o.sha256.clone(), o.size_bytes, o.path.clone())),
        )
        .collect();
    let mut seen = HashSet::new();
    wanted.retain(|w| seen.insert(w.1.clone()));
    let stored: HashSet<String> = state
        .cloud
        .stored(wanted.iter().map(|w| w.1.clone()).collect())
        .await?
        .into_iter()
        .collect();
    wanted.retain(|w| !stored.contains(&w.1));
    let new_bytes: u64 = wanted.iter().map(|w| w.2).sum();
    if storage.used_bytes + new_bytes > storage.quota_bytes {
        return Err(AppError::new(
            crate::error::ErrorCode::QuotaExceeded,
            "Not enough storage left. Set some files to Only me.",
        ));
    }
    for (file, sha256, size, name) in &wanted {
        state.cloud.upload(file, sha256, *size, name).await?;
    }
    if !wanted.is_empty() {
        tracing::info!(files = wanted.len(), bytes = new_bytes, "uploaded files");
    }
    Ok(configs.into_iter().map(|(o, _)| o).collect())
}

/// The manifest for these files and settings, with these config files.
fn build(
    row: &InstanceRow,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
    description: Option<String>,
    overrides: Vec<Override>,
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
        let upload = is_upload(r);
        content.push(Entry {
            kind: from_wire(&r.kind).unwrap_or(ContentKind::Mod),
            source: if upload {
                ContentSource::Upload
            } else {
                from_wire(&r.source).unwrap_or(ContentSource::Modrinth)
            },
            // Uploads are known by their hash, not a project.
            project_id: r.project_id.clone().filter(|_| !upload),
            version_id: r.version_id.clone().filter(|_| !upload),
            name: r.name.clone(),
            version_label: r.version_label.clone(),
            path: r.path.clone(),
            sha1: r.sha1.clone(),
            sha256: r.sha256.clone(),
            size_bytes: r.size_bytes.max(0) as u64,
            download_url: r.download_url.clone().filter(|_| !upload),
            page_url: r.page_url.clone().filter(|_| !upload),
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
        overrides,
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
    // Check the manifest builds before uploading anything.
    build(&row, &rows, &modes, description.clone(), Vec::new())?;
    let overrides = send_files(state, &row, &rows, &modes, input.confirmed_upload_rights).await?;
    let m = build(&row, &rows, &modes, description.clone(), overrides)?;
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
    build(&row, &rows, &modes, description.clone(), Vec::new())?;
    let overrides = send_files(state, &row, &rows, &modes, input.confirmed_upload_rights).await?;
    let m = build(&row, &rows, &modes, description, overrides)?;
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
