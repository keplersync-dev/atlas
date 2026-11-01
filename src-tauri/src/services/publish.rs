//! Sharing an instance and publishing new versions of it: upload the files that aren't
//! on a mod site and the shared config files, build a manifest from the folder's files
//! and their share settings, send it, then mark the files as published. Nothing changes
//! locally until the server has accepted it.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use reqwest::Method;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Runtime};

use super::share_files;
use super::work::Reporter;
use crate::cloud::files::UploadFile;
use crate::db::file_hashes::FileHash;
use crate::db::{self, content::ContentRow, instances::InstanceRow};
use crate::dto::{
    ContentKind, ContentSource, CreateRevision, CreateSharedInstance, FileRules, InstanceSummary,
    LeftOutFile, LeftOutReason, ManualDownload, MemberRole, PackInfo, PublishInput, PublishItem,
    PublishPlan, RevisionSummary, ShareFile, ShareInstanceInput, ShareMode, SharedInstance, Side,
    Storage, WorkStage, from_wire, to_wire,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;
use crate::sync::adopt;
use crate::sync::manifest::{
    self, Entry, MAX_ICON_BYTES, MAX_OVERRIDES, MAX_OVERRIDES_TOTAL, Manifest, MinecraftSpec,
    Override, PackBase, Refusal, SCHEMA_VERSION,
};
use crate::sync::pack;
use crate::util::time::iso_from_millis;

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

/// Modrinth and CurseForge couldn't be asked about it (offline, or they failed), so
/// whether it has to be uploaded isn't known yet.
fn not_checked(r: &ContentRow) -> bool {
    r.source == "local" && r.resolved_at.is_none()
}

/// Files on no mod site that the modpack names by CurseForge id: CurseForge can't find
/// some files by fingerprint, but the pack lists them, so they aren't uploads.
async fn look_up_in_pack<R: Runtime>(state: &AppState, app: &AppHandle<R>, row: &InstanceRow) {
    let has_local = db::content::list(&state.db, &row.id)
        .await
        .is_ok_and(|rows| live(&rows).any(|r| r.source == "local"));
    if !has_local {
        return;
    }
    let Some(base) = pack::base_for(state, row).await else {
        return;
    };
    let work = Reporter::new(app, &row.id);
    let total = if pack::needs_download(&state.paths, &base) {
        base.size_bytes
    } else {
        0
    };
    let mut got = 0;
    let mut progress = |n: u64| {
        got += n;
        work.report(WorkStage::Downloading, got.min(total), total);
    };
    let found = match pack::curseforge_file_ids(state, &base, &mut progress).await {
        Ok(ids) => crate::resolve::resolve_from_pack(&state.db, &state.cloud, &row.id, &ids).await,
        Err(e) => Err(e),
    };
    match found {
        Ok(0) => {}
        Ok(n) => {
            tracing::info!(pack = %base.name, files = n, "found files by the modpack's list");
            emit(
                app,
                AtlasEvent::InstanceChanged {
                    instance_id: row.id.clone(),
                },
            );
        }
        Err(e) => {
            tracing::info!(pack = %base.name, error = %e, "couldn't look files up in the modpack's list")
        }
    }
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
    let started = std::time::Instant::now();
    let mut took = Vec::new();
    let mut lap = |step: &'static str| took.push((step, started.elapsed().as_millis() as u64));
    super::instances::rescan(state, app, id).await?;
    lap("rescan");
    // Files not looked up yet would all count as uploads.
    super::instances::look_up_now(state, app, id).await;
    lap("look up");
    look_up_in_pack(state, app, &row).await;
    lap("modpack list");
    let mut rows = db::content::list(&state.db, id).await?;
    if live(&rows).any(not_checked) {
        // Once more before telling the owner: the sites may only have hiccuped.
        super::instances::look_up_now(state, app, id).await;
        rows = db::content::list(&state.db, id).await?;
    }
    let to_check: Vec<String> = live(&rows)
        .filter(|r| is_upload(r))
        .map(|r| r.sha256.clone())
        .collect();
    lap("look up again");
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
                unchecked: not_checked(r),
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
    lap("uploaded check");
    let configs = plan_configs(state, &row, &Reporter::new(app, id)).await?;
    lap("other files");
    tracing::info!(instance = %row.name, ?took, "prepared a share (ms since start)");
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
        pack: configs.base.as_ref().map(|b| PackInfo {
            name: b.name.clone(),
            version_label: b.version_label.clone(),
            source: b.source,
            files: configs.shared.iter().filter(|(o, _)| o.in_pack).count() as u32,
        }),
        left_out: configs.left_out,
        files: configs.files,
        rules: file_rules(state, id).await?,
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

/// The tree's ticks, as saved with the last share or publish.
async fn file_rules(state: &AppState, id: &str) -> AppResult<FileRules> {
    let saved: Option<(String, String, String)> = sqlx::query_as(
        "SELECT sync_include, sync_exclude, sync_first_install FROM instance WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let (include, exclude, first) = saved.unwrap_or_default();
    let list = |json: &str| serde_json::from_str::<Vec<String>>(json).unwrap_or_default();
    Ok(share_files::from_saved(
        list(&include),
        list(&exclude),
        list(&first),
    ))
}

async fn save_file_rules(state: &AppState, id: &str, rules: &FileRules) -> AppResult<()> {
    sqlx::query(
        "UPDATE instance SET sync_include = ?, sync_exclude = ?, sync_first_install = ? WHERE id = ?",
    )
    .bind(serde_json::to_string(&rules.include)?)
    .bind(serde_json::to_string(&rules.exclude)?)
    .bind(serde_json::to_string(&rules.first_install)?)
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// The files the owner shares besides mods and packs (unticked ones left out), not
/// hashed and with no limit: for exporting a pack.
pub async fn shared_config_files(
    state: &AppState,
    row: &InstanceRow,
) -> AppResult<Vec<(String, PathBuf)>> {
    let rules = file_rules(state, &row.id).await?;
    let game = PathBuf::from(&row.game_dir);
    let listed = tokio::task::spawn_blocking(move || share_files::list(&game)).await?;
    Ok(listed
        .into_iter()
        .filter(|l| {
            share_files::shared(&rules, &l.path, false)
                && manifest::upload_refusal(&l.path, l.size).is_none()
        })
        .map(|l| (l.path, l.file))
        .collect())
}

/// What sharing does with the files besides mods and packs.
struct ConfigPlan {
    /// The modpack the instance is from, when its download could be read.
    base: Option<PackBase>,
    /// Shared, with the local file: `in_pack` ones come from the pack, the rest upload.
    shared: Vec<(Override, PathBuf)>,
    /// Ticked but not shared as they are here, and why. Shown before anything uploads.
    left_out: Vec<LeftOutFile>,
    /// Every file in the tree.
    files: Vec<ShareFile>,
}

fn left_out_reason(r: Refusal) -> LeftOutReason {
    match r {
        Refusal::Folder => LeftOutReason::Folder,
        Refusal::Login => LeftOutReason::Login,
        Refusal::Program => LeftOutReason::Program,
        Refusal::FileType => LeftOutReason::FileType,
        Refusal::TooBig => LeftOutReason::TooBig,
    }
}

/// Sort the ticked files: exactly as the modpack has them (referenced, not uploaded),
/// uploadable, or left out. A file the owner changed that can't be uploaded falls back
/// to the pack's own copy, so friends at least get that.
async fn plan_configs<R: Runtime>(
    state: &AppState,
    row: &InstanceRow,
    work: &Reporter<R>,
) -> AppResult<ConfigPlan> {
    let rules = file_rules(state, &row.id).await?;
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
    let in_pack = |rel: &str| index.as_ref().is_some_and(|i| i.contains_key(rel));
    let progress = |done, total| work.report(WorkStage::Reading, done, total);
    let (listed, hashed) = read_files(state, row, &rules, index.as_ref(), &progress).await?;
    let files: Vec<ShareFile> = listed
        .iter()
        .map(|l| ShareFile {
            path: l.path.clone(),
            size_bytes: l.size,
            modified: l.modified.and_then(iso_from_millis),
            from_pack: in_pack(&l.path),
            changed: false,
            refusal: manifest::upload_refusal(&l.path, l.size).map(left_out_reason),
        })
        .collect();
    let game = PathBuf::from(&row.game_dir);

    let mut plan = ConfigPlan {
        base: base.clone(),
        shared: Vec::new(),
        left_out: Vec::new(),
        files,
    };
    let mut uploaded = 0u64;
    let mut changed = HashSet::new();
    let full =
        |shared: usize, uploaded: u64| shared > MAX_OVERRIDES || uploaded > MAX_OVERRIDES_TOTAL;
    for FileHash {
        path: rel,
        size_bytes: size,
        sha256,
        settings: mine,
        ..
    } in hashed
    {
        let file = game.join(&rel);
        let from_pack = index.as_ref().and_then(|i| i.get(&rel)).cloned();
        // Set the same as the pack's copy: friends get that copy, nothing uploads.
        let as_is = from_pack
            .as_ref()
            .is_some_and(|p| p.sha256 == sha256 || (mine.is_some() && p.settings == mine));
        let reason = if as_is {
            None
        } else {
            manifest::upload_refusal(&rel, size)
        };
        if from_pack.is_some() && !as_is {
            changed.insert(rel.clone());
        }
        let shared = match (as_is, reason, from_pack) {
            (true, _, p) => p.map(|p| (p.sha256, p.size_bytes, true, None)),
            // Changed here, but not a file that can be uploaded: the pack's own copy.
            (false, Some(_), Some(p)) => {
                Some((p.sha256, p.size_bytes, true, Some(LeftOutReason::PackCopy)))
            }
            (false, Some(reason), None) => {
                plan.left_out.push(LeftOutFile {
                    path: rel.clone(),
                    reason: left_out_reason(reason),
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
                keep: share_files::keep(&rules, &rel),
                path: rel,
                sha256,
                size_bytes,
                in_pack,
            },
            file,
        ));
    }
    for f in &mut plan.files {
        f.changed = changed.contains(&f.path);
    }
    if !plan.left_out.is_empty() {
        tracing::info!(
            files = plan.left_out.len(),
            "ticked files not shared as they are"
        );
    }
    Ok(plan)
}

/// List the instance's files and hash the ticked ones, reusing the hashes saved last
/// time for files whose size and date haven't changed, then save them for next time.
/// `progress` gets (files read, files to read).
async fn read_files(
    state: &AppState,
    row: &InstanceRow,
    rules: &FileRules,
    index: Option<&pack::PackIndex>,
    progress: &(dyn Fn(u64, u64) + Sync),
) -> AppResult<(Vec<share_files::Listed>, Vec<FileHash>)> {
    let in_pack = |rel: &str| index.is_some_and(|i| i.contains_key(rel));
    let game = PathBuf::from(&row.game_dir);
    let listed = tokio::task::spawn_blocking(move || share_files::list(&game)).await?;
    let wanted: Vec<share_files::Listed> = listed
        .iter()
        .filter(|l| share_files::shared(rules, &l.path, in_pack(&l.path)))
        .cloned()
        .collect();
    let saved = Arc::new(db::file_hashes::list(&state.db, &row.id).await?);
    let count = wanted.len() as u64;
    let done = Arc::new(AtomicU64::new(0));
    let (counter, known) = (Arc::clone(&done), Arc::clone(&saved));
    let hashing =
        tokio::task::spawn_blocking(move || share_files::hashes(wanted, &known, &counter));
    tokio::pin!(hashing);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(200));
    let hashed = loop {
        tokio::select! {
            hashed = &mut hashing => break hashed??,
            _ = tick.tick() => progress(done.load(Ordering::Relaxed), count),
        }
    };
    let same = hashed.len() == saved.len() && hashed.iter().all(|h| saved.get(&h.path) == Some(h));
    if !same {
        db::file_hashes::replace(&state.db, &row.id, &hashed).await?;
    }
    Ok((listed, hashed))
}

/// The latest rescan per instance: a read ahead waits for rescans to stop.
static READ_AHEAD: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(Default::default);
static ONE_READ_AHEAD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// `read_ahead` once the instance has been quiet for a while: rescans come in bursts,
/// and Share's own rescan shouldn't race Share's own read.
pub fn read_ahead_soon(state: &AppState, id: &str) {
    let seen = {
        let mut latest = READ_AHEAD.lock().unwrap_or_else(|e| e.into_inner());
        let n = latest.entry(id.to_owned()).or_default();
        *n += 1;
        *n
    };
    let (state, id) = (state.clone(), id.to_owned());
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        let current = READ_AHEAD
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .copied();
        if current == Some(seen) {
            // One at a time: after startup every instance asks at once.
            let _one = ONE_READ_AHEAD.lock().await;
            read_ahead(&state, &id).await;
        }
    });
}

/// Read an instance's shared files ahead of Share, so the dialog opens on saved hashes.
/// Skipped while a game runs (it's busy writing them), for instances the user can't
/// publish, and when the modpack isn't read yet (Share downloads it first).
pub async fn read_ahead(state: &AppState, id: &str) {
    if state.running.any() {
        return;
    }
    let Ok(row) = row(state, id).await else {
        return;
    };
    if row.shared_instance_id.is_some() && row.role() != Some(MemberRole::Owner) {
        return;
    }
    let index = match pack::base_for(state, &row).await {
        Some(base) => match pack::saved_index(&state.paths, &base) {
            Some(index) => Some(index),
            None => return,
        },
        None => None,
    };
    let started = std::time::Instant::now();
    let rules = match file_rules(state, id).await {
        Ok(rules) => rules,
        Err(e) => return tracing::debug!(error = %e, "couldn't read share rules"),
    };
    match read_files(state, &row, &rules, index.as_ref(), &|_, _| {}).await {
        Ok((_, hashed)) => tracing::debug!(
            instance = %row.name,
            files = hashed.len(),
            ms = started.elapsed().as_millis() as u64,
            "read shared files ahead"
        ),
        Err(e) => {
            tracing::debug!(instance = %row.name, error = %e, "couldn't read shared files ahead")
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
    // Not known to be on no mod site: uploading it could share a mod friends could
    // have downloaded, under a permission the owner never needed to give.
    if let Some(r) = uploads.iter().find(|r| not_checked(r)) {
        return Err(AppError::conflict(format!(
            "Couldn't check {} with Modrinth and CurseForge. Try again in a moment, or set it to Only me.",
            r.name
        )));
    }
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
            page_url: r
                .page_url
                .clone()
                .filter(|u| !upload && manifest::allowed_page(u)),
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
    if let Some(files) = &input.files {
        save_file_rules(state, id, files).await?;
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
    if let Some(files) = &input.files {
        save_file_rules(state, id, files).await?;
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
