//! Sharing an instance and publishing new versions of it: upload the files that aren't
//! on a mod site and the shared config files, build a manifest from the folder's files
//! and their share settings, send it, then mark the files as published. Nothing changes
//! locally until the server has accepted it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use reqwest::Method;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Runtime};

use super::work::Reporter;
use crate::cloud::files::UploadFile;
use crate::db::{self, content::ContentRow, instances::InstanceRow};
use crate::dto::{
    ConfigGroup, ContentKind, ContentSource, CreateRevision, CreateSharedInstance, InstanceSummary,
    LeftOutFile, LeftOutReason, ManualDownload, MemberRole, PackInfo, PublishInput, PublishItem,
    PublishPlan, RevisionSummary, ShareInstanceInput, ShareMode, SharedInstance, Side, Storage,
    WorkStage, from_wire, to_wire,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;
use crate::sync::adopt;
use crate::sync::manifest::{
    self, Entry, MAX_ICON_BYTES, MAX_OVERRIDES, MAX_OVERRIDES_TOTAL, Manifest, MinecraftSpec,
    Override, PackBase, Refusal, SCHEMA_VERSION,
};
use crate::sync::pack::{self, PackIndex};

async fn row(state: &AppState, id: &str) -> AppResult<InstanceRow> {
    db::instances::get(&state.db, id).await?.ok_or_else(|| {
        AppError::not_found("That instance isn't there anymore. It may have just been removed.")
    })
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

pub async fn prepare<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
) -> AppResult<PublishPlan> {
    let row = row(state, id).await?;
    if row.shared_instance_id.is_some() && row.role() == Some(MemberRole::Viewer) {
        return Err(AppError::forbidden(
            "Only the owner and editors can publish",
        ));
    }
    // Read the folder now, after any scan already running: a just-tracked instance may
    // not have been scanned yet, and the plan must match what's on disk.
    super::instances::rescan(state, app, id).await?;
    // Files not looked up yet would all count as uploads.
    super::instances::look_up_now(state, app, id).await;
    let rows = db::content::list(&state.db, id).await?;
    let unchecked = live(&rows)
        .filter(|r| r.source == "local" && r.resolved_at.is_none())
        .count() as u32;
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
    let configs = plan_configs(state, &row, &Reporter::new(app, id)).await?;
    let exclude = config_exclude(state, id).await?;
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
        unchecked,
        pack: configs.base.as_ref().map(|b| PackInfo {
            name: b.name.clone(),
            version_label: b.version_label.clone(),
            source: b.source,
            files: configs.shared.iter().filter(|(o, _)| o.in_pack).count() as u32,
        }),
        left_out: configs.left_out,
        configs: config_groups(&configs.candidates, &exclude),
        versions: row
            .synced_manifest()
            .and_then(|m| crate::sync::state::version_change(&row, &m)),
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

/// Folders whose files are shared besides `sync_include`: configs, and the scripts and
/// assets packs keep next to them.
const SHARED_DIRS: [&str; 4] = ["config", "defaultconfigs", "kubejs", "scripts"];

/// The files a share may include, not hashed: `(path, file, size)`. Those under the
/// shared folders and `sync_include`, and the modpack's own files that are here.
async fn config_candidates(
    state: &AppState,
    row: &InstanceRow,
    pack: Option<&PackIndex>,
) -> AppResult<Vec<(String, PathBuf, u64)>> {
    let include: Option<String> =
        sqlx::query_scalar("SELECT sync_include FROM instance WHERE id = ?")
            .bind(&row.id)
            .fetch_optional(&state.db)
            .await?;
    let mut patterns: Vec<String> = include
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    patterns.extend(SHARED_DIRS.iter().map(|d| format!("{d}/**")));
    patterns.extend(pack.into_iter().flat_map(|p| p.keys().cloned()));
    let game = PathBuf::from(&row.game_dir);
    tokio::task::spawn_blocking(move || candidates_in(&game, &patterns))
        .await
        .map_err(AppError::from)
}

fn candidates_in(game: &Path, patterns: &[String]) -> Vec<(String, PathBuf, u64)> {
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
    files
        .into_iter()
        .filter_map(|(rel, file)| {
            let size = std::fs::metadata(&file).ok()?.len();
            Some((rel, file, size))
        })
        .collect()
}

/// What the owner switches on and off: a top folder's subfolder ("config/create/",
/// usually one per mod; "kubejs/assets/"), or the file itself when it's loose
/// ("config/a.json", "options.txt").
pub fn config_group(path: &str) -> String {
    let mut parts = path.splitn(3, '/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(top), Some(sub), Some(_)) => format!("{top}/{sub}/"),
        _ => path.to_owned(),
    }
}

/// The config folders and files this instance doesn't share.
async fn config_exclude(state: &AppState, id: &str) -> AppResult<Vec<String>> {
    let json: Option<String> = sqlx::query_scalar("SELECT sync_exclude FROM instance WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    Ok(json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default())
}

async fn save_config_exclude(state: &AppState, id: &str, exclude: &[String]) -> AppResult<()> {
    sqlx::query("UPDATE instance SET sync_exclude = ? WHERE id = ?")
        .bind(serde_json::to_string(exclude)?)
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// The config files the owner shares (switched-off ones left out), not hashed and with no
/// limit: for exporting a pack.
pub async fn shared_config_files(
    state: &AppState,
    row: &InstanceRow,
) -> AppResult<Vec<(String, PathBuf)>> {
    let exclude = config_exclude(state, &row.id).await?;
    Ok(config_candidates(state, row, None)
        .await?
        .into_iter()
        .filter(|(rel, _, size)| {
            !exclude.contains(&config_group(rel)) && manifest::upload_refusal(rel, *size).is_none()
        })
        .map(|(rel, file, _)| (rel, file))
        .collect())
}

/// What sharing does with the config files.
struct ConfigPlan {
    /// The modpack the instance is from, when its download could be read.
    base: Option<PackBase>,
    /// Shared, with the local file: `in_pack` ones come from the pack, the rest upload.
    shared: Vec<(Override, PathBuf)>,
    /// Not shared, and why. Shown before anything uploads.
    left_out: Vec<LeftOutFile>,
    /// Every file that could be, for the dialog's groups: `(path, size)`.
    candidates: Vec<(String, u64)>,
}

/// Sort the config files: exactly as the modpack has them (referenced, not uploaded),
/// uploadable, or left out. A file the owner changed that can't be uploaded falls back
/// to the pack's own copy, so friends at least get that.
async fn plan_configs<R: Runtime>(
    state: &AppState,
    row: &InstanceRow,
    work: &Reporter<R>,
) -> AppResult<ConfigPlan> {
    let exclude = config_exclude(state, &row.id).await?;
    let mut base = pack::base_for(state, row).await;
    let mut index = None;
    if let Some(b) = &base {
        let total = if pack::needs_download(&state.paths, b) {
            b.size_bytes
        } else {
            0
        };
        let mut got = 0;
        let mut progress = |n: u64| {
            got += n;
            work.report(WorkStage::Downloading, got.min(total), total);
        };
        match pack::index(state, b, &mut progress).await {
            Ok(i) => index = Some(i),
            Err(e) => {
                tracing::info!(pack = %b.name, error = %e, "couldn't read the modpack; uploading its files");
                base = None;
            }
        }
    }
    let candidates = config_candidates(state, row, index.as_ref()).await?;
    let wanted: Vec<(String, PathBuf, u64)> = candidates
        .iter()
        .filter(|(rel, ..)| !exclude.contains(&config_group(rel)))
        .cloned()
        .collect();
    let count = wanted.len() as u64;
    let done = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&done);
    let hashing = tokio::task::spawn_blocking(move || {
        wanted
            .into_iter()
            .map(|(rel, file, size)| {
                let hashes = crate::scan::hash::hash_file(&file);
                counter.fetch_add(1, Ordering::Relaxed);
                hashes.map(|h| (rel, file, size, h.sha256))
            })
            .collect::<std::io::Result<Vec<_>>>()
    });
    tokio::pin!(hashing);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(200));
    let hashed = loop {
        tokio::select! {
            hashed = &mut hashing => break hashed??,
            _ = tick.tick() => work.report(WorkStage::Reading, done.load(Ordering::Relaxed), count),
        }
    };

    let mut plan = ConfigPlan {
        base: base.clone(),
        shared: Vec::new(),
        left_out: Vec::new(),
        candidates: candidates
            .into_iter()
            .map(|(rel, _, size)| (rel, size))
            .collect(),
    };
    let mut uploaded = 0u64;
    let full =
        |shared: usize, uploaded: u64| shared > MAX_OVERRIDES || uploaded > MAX_OVERRIDES_TOTAL;
    for (rel, file, size, sha256) in hashed {
        let from_pack = index.as_ref().and_then(|i| i.get(&rel)).cloned();
        let as_is = from_pack.as_ref().is_some_and(|p| p.sha256 == sha256);
        let reason = if as_is {
            None
        } else {
            manifest::upload_refusal(&rel, size)
        };
        let shared = match (as_is, reason, from_pack) {
            (true, ..) => Some((sha256, size, true, None)),
            // Changed here, but not a file that can be uploaded: the pack's own copy.
            (false, Some(_), Some(p)) => {
                Some((p.sha256, p.size_bytes, true, Some(LeftOutReason::PackCopy)))
            }
            (false, Some(reason), None) => {
                plan.left_out.push(LeftOutFile {
                    path: rel.clone(),
                    reason: match reason {
                        Refusal::Folder => LeftOutReason::Folder,
                        Refusal::FileType => LeftOutReason::FileType,
                        Refusal::TooBig => LeftOutReason::TooBig,
                    },
                });
                None
            }
            (false, None, _) => Some((sha256, size, false, None)),
        };
        let Some((sha256, size_bytes, in_pack, note)) = shared else {
            continue;
        };
        let upload = if in_pack { 0 } else { size_bytes };
        if full(plan.shared.len() + 1, uploaded + upload) {
            plan.left_out.push(LeftOutFile {
                path: rel,
                reason: LeftOutReason::OverLimit,
            });
            continue;
        }
        uploaded += upload;
        if let Some(reason) = note {
            plan.left_out.push(LeftOutFile {
                path: rel.clone(),
                reason,
            });
        }
        plan.shared.push((
            Override {
                path: rel,
                sha256,
                size_bytes,
                in_pack,
            },
            file,
        ));
    }
    if !plan.left_out.is_empty() {
        tracing::info!(
            files = plan.left_out.len(),
            "config files not shared as they are"
        );
    }
    Ok(plan)
}

/// The config folders and loose files the dialog lists, and whether each is shared.
fn config_groups(candidates: &[(String, u64)], exclude: &[String]) -> Vec<ConfigGroup> {
    let mut groups: BTreeMap<String, ConfigGroup> = BTreeMap::new();
    for (rel, size) in candidates {
        let key = config_group(rel);
        let group = groups.entry(key.clone()).or_insert_with(|| ConfigGroup {
            shared: !exclude.contains(&key),
            key,
            files: 0,
            size_bytes: 0,
        });
        group.files += 1;
        group.size_bytes += size;
    }
    groups.into_values().collect()
}

/// Every file under `dir`, with paths relative to the game folder ("config/a/b.json").
/// Folder links (symlinks, junctions) aren't followed: one pointing back up would never end.
fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = format!("{rel}/{name}");
        if kind.is_symlink() {
            continue;
        } else if kind.is_dir() {
            walk(&entry.path(), &child, out);
        } else if kind.is_file() {
            out.push((child, entry.path()));
        }
    }
}

/// Upload what the version needs that the server doesn't have yet: files on no mod site
/// that are shared, and the config files that aren't the modpack's own. Returns the
/// configs as overrides, and the modpack they're based on.
async fn send_files<R: Runtime>(
    state: &AppState,
    row: &InstanceRow,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
    confirmed_rights: bool,
    work: &Reporter<R>,
) -> AppResult<(Vec<Override>, Option<PackBase>)> {
    let uploads: Vec<&ContentRow> = live(rows)
        .filter(|r| modes.get(&r.id) != Some(&ShareMode::Private) && is_upload(r))
        .collect();
    let storage: Storage = state
        .cloud
        .call(Method::GET, "/me/storage", None::<&()>)
        .await?;
    let ConfigPlan {
        base,
        shared: configs,
        ..
    } = plan_configs(state, row, work).await?;

    let game = PathBuf::from(&row.game_dir);
    // (file, sha256, size, name, rights confirmed): the checkbox covers mods and packs;
    // config files are the owner's own settings.
    let mut wanted: Vec<(PathBuf, String, u64, String, bool)> = uploads
        .iter()
        .map(|r| {
            (
                game.join(&r.path),
                r.sha256.clone(),
                r.size_bytes.max(0) as u64,
                r.name.clone(),
                confirmed_rights,
            )
        })
        .chain(configs.iter().filter(|(o, _)| !o.in_pack).map(|(o, file)| {
            (
                file.clone(),
                o.sha256.clone(),
                o.size_bytes,
                o.path.clone(),
                false,
            )
        }))
        .collect();
    let mut seen = HashSet::new();
    wanted.retain(|w| seen.insert(w.1.clone()));
    let stored: HashSet<String> = state
        .cloud
        .stored(wanted.iter().map(|w| w.1.clone()).collect())
        .await?
        .into_iter()
        .collect();
    // Files uploaded for an earlier version were confirmed then; only new ones need it,
    // the same rule the dialog uses.
    let new_uploads: Vec<&&ContentRow> = uploads
        .iter()
        .filter(|r| !stored.contains(&r.sha256))
        .collect();
    if !new_uploads.is_empty() && !confirmed_rights {
        return Err(AppError::invalid_input(
            "Confirm you have the right to share the files being uploaded",
        ));
    }
    if let Some(big) = new_uploads
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
    wanted.retain(|w| !stored.contains(&w.1));
    let new_bytes: u64 = wanted.iter().map(|w| w.2).sum();
    if storage.used_bytes + new_bytes > storage.quota_bytes {
        return Err(AppError::new(
            crate::error::ErrorCode::QuotaExceeded,
            "Not enough storage left. Set some files to Only me.",
        ));
    }
    // Files read when the dialog opened may have changed since (the launcher updated a mod).
    if let Some((_, _, _, name, _)) = wanted
        .iter()
        .find(|(file, _, size, ..)| std::fs::metadata(file).map(|m| m.len()).ok() != Some(*size))
    {
        return Err(AppError::conflict(format!(
            "{name} changed since you opened this. Close it and publish again."
        )));
    }
    // Progress in bytes, so it moves within a big file too.
    let total = new_bytes;
    let sent = Arc::new(AtomicU64::new(0));
    work.report(WorkStage::Uploading, 0, total);
    let files: Vec<UploadFile> = wanted
        .iter()
        .map(|(path, sha256, size, name, rights)| UploadFile {
            path: path.clone(),
            sha256: sha256.clone(),
            size: *size,
            name: name.clone(),
            rights_confirmed: *rights,
        })
        .collect();
    let counter = Arc::clone(&sent);
    let uploads = state.cloud.upload_all(&files, move |n| {
        counter.fetch_add(n, Ordering::Relaxed);
    });
    tokio::pin!(uploads);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(200));
    loop {
        tokio::select! {
            uploaded = &mut uploads => break uploaded?,
            _ = tick.tick() => {
                work.report(WorkStage::Uploading, sent.load(Ordering::Relaxed).min(total), total)
            }
        }
    }
    work.report(WorkStage::Uploading, total, total);
    if !wanted.is_empty() {
        tracing::info!(files = wanted.len(), bytes = new_bytes, "uploaded files");
    }
    Ok((configs.into_iter().map(|(o, _)| o).collect(), base))
}

/// Upload the instance's picture so friends see it too; returns the manifest's `icon`.
/// A picture that can't be read or sent is left out rather than stopping the share.
async fn send_icon(state: &AppState, row: &InstanceRow) -> Option<String> {
    let icon = row.icon.as_deref()?;
    let bytes = if let Some(path) = icon.strip_prefix("file:") {
        tokio::fs::read(path).await.ok()?
    } else if icon.starts_with("https://") {
        let res = state.downloads.get(icon).send().await.ok()?;
        if res.content_length().unwrap_or(0) > MAX_ICON_BYTES {
            return None;
        }
        res.error_for_status().ok()?.bytes().await.ok()?.to_vec()
    } else {
        return None;
    };
    if bytes.len() as u64 > MAX_ICON_BYTES || !crate::scan::icons::is_image(&bytes) {
        tracing::info!(size = bytes.len(), "instance picture left out of the share");
        return None;
    }
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let file = crate::scan::icons::shared_icon_file(&state.paths.icons, &sha256);
    if !file.is_file() && tokio::fs::write(&file, &bytes).await.is_err() {
        return None;
    }
    match state
        .cloud
        .upload(UploadFile {
            path: file,
            sha256: sha256.clone(),
            size: bytes.len() as u64,
            name: "The instance picture".into(),
            rights_confirmed: false,
        })
        .await
    {
        Ok(()) => Some(format!("sha256:{sha256}")),
        Err(e) => {
            tracing::warn!(error = %e, "couldn't upload the instance picture");
            None
        }
    }
}

/// The manifest for these files and settings, with these config files.
fn build(
    row: &InstanceRow,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
    description: Option<String>,
    (overrides, base): (Vec<Override>, Option<PackBase>),
    icon: Option<String>,
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
        icon,
        minecraft: MinecraftSpec {
            version,
            loader,
            loader_version: row.loader_version.clone(),
        },
        recommended_memory_mb: None,
        content,
        // Only named when some files come from it.
        base: base.filter(|_| overrides.iter().any(|o| o.in_pack)),
        overrides,
    };
    manifest::validate(&m).map_err(|e| {
        tracing::warn!(error = %e, "built a manifest that doesn't validate");
        AppError::invalid_input(format!("Atlas can't share this instance. {e}"))
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

/// Share it. Runs as the instance's job, so an update or restore can't run on it meanwhile.
pub async fn share<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: ShareInstanceInput,
) -> AppResult<SharedInstance> {
    let mut shared = None;
    let slot = &mut shared;
    crate::jobs::run(state, app, id, move |_| async move {
        *slot = Some(share_now(state, app, id, input).await?);
        Ok(())
    })
    .await?;
    Ok(shared.expect("set when the job succeeds"))
}

async fn share_now<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: ShareInstanceInput,
) -> AppResult<SharedInstance> {
    let row = row(state, id).await?;
    if row.shared_instance_id.is_some() {
        return Err(AppError::conflict("This instance is already shared"));
    }
    if let Some(exclude) = &input.config_exclude {
        save_config_exclude(state, id, exclude).await?;
    }
    let rows = db::content::list(&state.db, id).await?;
    let modes = modes(&rows, &input.share_modes);
    let description = input.description.flatten();
    // Check the manifest builds before uploading anything.
    build(
        &row,
        &rows,
        &modes,
        description.clone(),
        (Vec::new(), None),
        None,
    )?;
    let work = Reporter::new(app, id);
    let configs = send_files(
        state,
        &row,
        &rows,
        &modes,
        input.confirmed_upload_rights,
        &work,
    )
    .await?;
    let icon = send_icon(state, &row).await;
    let m = build(&row, &rows, &modes, description.clone(), configs, icon)?;
    work.report(WorkStage::Saving, 0, 0);
    let body = CreateSharedInstance {
        name: row.name.clone(),
        description,
        manifest: m.clone(),
        friend_ids: input.friend_ids,
        group_ids: input.group_ids,
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

/// Publish a new version. Runs as the instance's job, like [`share`].
pub async fn publish<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: PublishInput,
) -> AppResult<InstanceSummary> {
    crate::jobs::run(state, app, id, |_| publish_now(state, app, id, input)).await?;
    super::instances::summary_by_id(state, id).await
}

async fn publish_now<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: PublishInput,
) -> AppResult<()> {
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
    if let Some(exclude) = &input.config_exclude {
        save_config_exclude(state, id, exclude).await?;
    }
    let rows = db::content::list(&state.db, id).await?;
    let modes = modes(&rows, &input.share_modes);
    // The shared instance's own description (what editing it changes), not the copy in
    // the last version, so publishing never brings back an old one.
    let description = match super::shared::cached(state, &shared).await? {
        Some(s) => s.description,
        None => row.synced_manifest().and_then(|m| m.description),
    };
    build(
        &row,
        &rows,
        &modes,
        description.clone(),
        (Vec::new(), None),
        None,
    )?;
    let work = Reporter::new(app, id);
    let configs = send_files(
        state,
        &row,
        &rows,
        &modes,
        input.confirmed_upload_rights,
        &work,
    )
    .await?;
    // The owner's picture is the shared one; editors keep it.
    let icon = if row.role() == Some(MemberRole::Owner) {
        send_icon(state, &row).await
    } else {
        row.synced_manifest().and_then(|m| m.icon)
    };
    let m = build(&row, &rows, &modes, description, configs, icon)?;
    if row.synced_manifest_hash.as_deref() == Some(manifest::hash(&m).as_str()) {
        return Err(AppError::invalid_input("Nothing to publish"));
    }
    work.report(WorkStage::Saving, 0, 0);
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configs_are_chosen_by_mod_folder_or_loose_file() {
        assert_eq!(config_group("config/create/server.toml"), "config/create/");
        assert_eq!(config_group("config/create/a/b.json"), "config/create/");
        assert_eq!(config_group("config/sodium.json"), "config/sodium.json");
        assert_eq!(config_group("options.txt"), "options.txt");
    }
}
