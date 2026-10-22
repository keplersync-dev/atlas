//! Shared-instance sync on this PC: state for summaries, preview, update, revert, restore,
//! optional items. Versions come from a `Source`: the Atlas server, or a dev folder.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Runtime};

use crate::db::{self, instances::InstanceRow};
use crate::dto::{
    BackupReason, ConfigConflict, InstanceSyncInfo, MemberRole, SyncOptions, SyncPreview, SyncStage,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::jobs::{self, Progress};
use crate::scan;
use crate::state::AppState;
use crate::sync::backup::{self, FileAction, FileRecord};
use crate::sync::fetch::{self, Route};
use crate::sync::source::{self, Source};
use crate::sync::{adopt, apply, prism_versions, state as sync_state};
use crate::util::ids::new_id;

async fn row(state: &AppState, id: &str) -> AppResult<InstanceRow> {
    db::instances::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))
}

/// The instance's source, or `conflict` if it isn't shared.
async fn linked(state: &AppState, row: &InstanceRow) -> AppResult<Source> {
    let shared = row
        .shared_instance_id
        .as_deref()
        .ok_or_else(|| AppError::conflict("Instance is not shared"))?;
    let synced = row.synced_revision.unwrap_or(0).max(0) as u32;
    source::for_shared(state, shared, synced).await
}

/// `InstanceSummary.sync`: None when the instance isn't shared.
pub async fn info(state: &AppState, row: &InstanceRow) -> AppResult<Option<InstanceSyncInfo>> {
    let (Some(shared), Some(role)) = (row.shared_instance_id.as_deref(), row.role()) else {
        return Ok(None);
    };
    let synced = row.synced_revision.unwrap_or(0).max(0) as u32;
    let src = source::for_shared(state, shared, synced).await?;
    let latest = src.latest_revision().max(synced);
    let frozen = src.is_frozen();
    let owner = match src.server_id() {
        Some(id) => super::shared::cached(state, id)
            .await?
            .and_then(|s| s.owner),
        None => None,
    };
    let rows = db::content::list(&state.db, &row.id).await?;
    let manifest = row.synced_manifest();
    let changes = sync_state::local_changes(role, &rows, manifest.as_ref()).count();
    Ok(Some(InstanceSyncInfo {
        shared_instance_id: shared.to_string(),
        role,
        owner,
        synced_revision: synced,
        latest_revision: latest,
        state: sync_state::sync_state(frozen, synced, latest, changes),
        local_change_count: changes,
    }))
}

/// First differing line of two texts, trimmed, for the conflict list.
fn first_difference(mine: &str, theirs: &str) -> (String, String) {
    let trim = |s: &str| s.trim().chars().take(200).collect::<String>();
    let (a, b): (Vec<_>, Vec<_>) = (mine.lines().collect(), theirs.lines().collect());
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(""),
            b.get(i).copied().unwrap_or(""),
        );
        if x != y {
            return (trim(x), trim(y));
        }
    }
    (String::new(), String::new())
}

pub async fn preview(state: &AppState, id: &str) -> AppResult<SyncPreview> {
    let row = row(state, id).await?;
    let src = linked(state, &row).await?;
    let latest = src.latest_revision();
    let target = src.manifest(state, latest).await?;
    let none = HashSet::new();
    let (plan, rows, _) =
        apply::make_plan(state, &row, &target, &none, &Default::default()).await?;
    let game_dir = PathBuf::from(&row.game_dir);

    let mut download_bytes = 0;
    let mut manual = Vec::new();
    for (e, local) in plan.places() {
        match fetch::route(&state.paths, &src, &game_dir, e, local) {
            Route::Cached | Route::Local(_) => {}
            Route::Manual(m) => manual.push(m),
            _ => download_bytes += e.size_bytes,
        }
    }
    let mut config_conflicts = Vec::new();
    for c in &plan.conflicts {
        let mine = std::fs::read_to_string(game_dir.join(&c.path)).unwrap_or_default();
        let theirs_file = match (
            src.server_id(),
            target.overrides.iter().find(|o| o.path == c.path),
        ) {
            (Some(shared), Some(o)) => {
                let url = state.cloud.file_url(shared, &o.sha256).await?;
                Some(fetch::config_from(&state.paths, state.cloud.file_http(), o, &url).await?)
            }
            _ => src.local_copy(&c.path),
        };
        let theirs = theirs_file
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let (mine, theirs) = first_difference(&mine, &theirs);
        config_conflicts.push(ConfigConflict {
            path: c.path.clone(),
            mine,
            theirs,
        });
    }
    let role = row.role().unwrap_or(MemberRole::Viewer);
    let synced = row.synced_manifest();
    Ok(SyncPreview {
        from_revision: row.synced_revision.unwrap_or(0).max(0) as u32,
        to_revision: latest,
        changes: plan.changes,
        download_bytes,
        manual_downloads: manual,
        overwritten_local_changes: sync_state::local_changes(role, &rows, synced.as_ref()).removed,
        config_conflicts,
        launcher_change: prism_versions::change(&row, &target),
        game_running: state.is_running(id),
        new_optional: plan.new_optional,
    })
}

/// Start an update to the latest version. Returns once the job has started.
pub async fn sync<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    options: SyncOptions,
) -> AppResult<()> {
    let row = row(state, id).await?;
    let src = linked(state, &row).await?;
    if src.is_frozen() {
        return Err(AppError::conflict("This instance is frozen"));
    }
    if !Path::new(&row.game_dir).is_dir() {
        return Err(AppError::conflict("The folder is missing"));
    }
    let revision = src.latest_revision();
    let manifest = src.manifest(state, revision).await?;
    let state2 = state.clone();
    jobs::spawn(state, app, id, move |progress| async move {
        let req = apply::Request {
            revision,
            manifest,
            reason: BackupReason::BeforeUpdate,
            install_optional: options.install_optional.into_iter().collect(),
            config_choices: options.config_choices,
        };
        apply::run(&state2, &progress, &src, req).await?;
        report_if_shared(&state2, &progress.instance_id, &src).await;
        backup::prune(&state2.db, &state2.paths, &progress.instance_id).await
    })
}

/// After a server instance's folder changed revision, tell the server.
pub async fn report_if_shared(state: &AppState, instance_id: &str, src: &Source) {
    if src.server_id().is_none() {
        return;
    }
    if let Ok(Some(row)) = db::instances::get(&state.db, instance_id).await {
        super::shared::report_synced(state, &row).await;
    }
}

/// Viewers: put shared files back the way the synced version has them. Waits for it.
pub async fn revert<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) -> AppResult<()> {
    let row = row(state, id).await?;
    let src = linked(state, &row).await?;
    if state.is_running(id) {
        return Err(AppError::conflict("Close the game first"));
    }
    let revision = row.synced_revision.unwrap_or(0).max(0) as u32;
    let manifest = row
        .synced_manifest()
        .ok_or_else(|| AppError::conflict("Nothing to revert to yet"))?;
    jobs::run(state, app, id, |progress| async move {
        let req = apply::Request {
            revision,
            manifest,
            reason: BackupReason::BeforeRevert,
            install_optional: HashSet::new(),
            config_choices: Default::default(),
        };
        apply::run(state, &progress, &src, req).await?;
        backup::prune(&state.db, &state.paths, id).await
    })
    .await
}

/// Download (true) or remove (false) an optional item from the synced version. Waits for it.
pub async fn set_optional_installed<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    content_id: &str,
    installed: bool,
) -> AppResult<()> {
    let row = row(state, id).await?;
    if state.is_running(id) {
        return Err(AppError::conflict("Close the game first"));
    }
    let item = db::content::get(&state.db, id, content_id)
        .await?
        .ok_or_else(|| AppError::not_found("Content not found"))?;
    if !item.is_managed || item.share_mode != "optional" {
        return Err(AppError::invalid_input(
            "Only optional items can be added or removed this way",
        ));
    }
    if installed == item.installed {
        return Ok(());
    }
    if !installed {
        // Their choice: delete the file, keep the row so it can be downloaded again.
        let _scanning = state.scan_lock(id).await;
        let path = Path::new(&row.game_dir).join(crate::sync::plan::disk_path(&item));
        std::fs::remove_file(&path).or_else(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(e)
            }
        })?;
        let mut c = state.db.acquire().await?;
        db::content::mark_not_installed(&mut c, content_id).await?;
        emit(
            app,
            AtlasEvent::InstanceChanged {
                instance_id: id.to_string(),
            },
        );
        return Ok(());
    }
    let src = linked(state, &row).await?;
    let revision = row.synced_revision.unwrap_or(0).max(0) as u32;
    let manifest = row
        .synced_manifest()
        .ok_or_else(|| AppError::conflict("Instance is not shared"))?;
    let key = manifest
        .content
        .iter()
        .find(|e| e.path.eq_ignore_ascii_case(&item.path))
        .map(|e| e.key().to_string())
        .ok_or_else(|| AppError::not_found("That item isn't in the shared version anymore"))?;
    jobs::run(state, app, id, |progress| async move {
        let req = apply::Request {
            revision,
            manifest,
            reason: BackupReason::BeforeUpdate,
            install_optional: HashSet::from([key]),
            config_choices: Default::default(),
        };
        apply::run(state, &progress, &src, req).await?;
        backup::prune(&state.db, &state.paths, id).await
    })
    .await
}

/// Undo exactly what a backup recorded, and put the database back too. Returns once started.
pub async fn restore<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    backup_id: &str,
) -> AppResult<()> {
    let row = row(state, id).await?;
    if !Path::new(&row.game_dir).is_dir() {
        return Err(AppError::conflict(format!("Instance is {}", "missing")));
    }
    let b = db::backups::get(&state.db, id, backup_id)
        .await?
        .ok_or_else(|| AppError::not_found("Backup not found"))?;
    let records: Vec<FileRecord> = serde_json::from_str(&b.files)?;
    let from_dir = backup::dir(&state.paths, id, backup_id);
    let state2 = state.clone();
    jobs::spawn(state, app, id, move |progress| async move {
        restore_job(&state2, &progress, row, records, from_dir).await
    })
}

async fn restore_job<R: Runtime>(
    state: &AppState,
    progress: &Progress<R>,
    row: InstanceRow,
    records: Vec<FileRecord>,
    from_dir: PathBuf,
) -> AppResult<()> {
    let id = row.id.clone();
    if state.is_running(&id) {
        progress.stage(SyncStage::WaitingForGameExit);
        while state.is_running(&id) {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
    let game_dir = PathBuf::from(&row.game_dir);
    let snapshot = backup::read_snapshot(&from_dir)?;

    // A backup of the current state first, so the restore itself can be undone.
    progress.stage(SyncStage::Backup);
    let now: Vec<FileRecord> = records
        .iter()
        .map(|r| {
            let exists = game_dir.join(&r.path).is_file();
            let action = match (r.action, exists) {
                (FileAction::Added, _) => FileAction::Removed, // restore deletes it
                (_, true) => FileAction::Replaced,
                (_, false) => FileAction::Added,
            };
            FileRecord {
                path: r.path.clone(),
                action,
            }
        })
        .collect();
    let to = snapshot.synced_revision.map(|r| r.max(0) as u32);
    let from = row.synced_revision.map(|r| r.max(0) as u32);
    backup::create(
        &state.db,
        &state.paths,
        &id,
        &game_dir,
        BackupReason::BeforeRestore,
        from,
        to,
        now,
    )
    .await?;

    progress.stage(SyncStage::Apply);
    let (game, dir, recs) = (game_dir.clone(), from_dir.clone(), records.clone());
    tokio::task::spawn_blocking(move || -> AppResult<()> {
        for r in &recs {
            let target = game.join(&r.path);
            match r.action {
                FileAction::Added => match std::fs::remove_file(&target) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                    _ => {}
                },
                FileAction::Replaced | FileAction::Removed => {
                    let copy = dir.join(&r.path);
                    if copy.is_file() {
                        std::fs::create_dir_all(
                            target.parent().expect("relative path has a parent"),
                        )?;
                        std::fs::copy(&copy, &target)?;
                    }
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| AppError::unknown(e.to_string()))??;

    let _scanning = state.scan_lock(&id).await;
    backup::restore_snapshot(&state.db, &id, &snapshot).await?;
    scan::files::scan(&state.db, &id, &game_dir).await?;
    drop(_scanning);
    backup::prune(&state.db, &state.paths, &id).await
}

// ── Development: link an instance to a folder of manifests ─────────────

/// Debug builds only: link an instance to a dev source folder (sync/source.rs) with a
/// role. `adopt_revision` = treat the folder as already on that revision (marks its files
/// as shared); None = nothing installed yet, so an update installs everything.
pub async fn dev_link(
    state: &AppState,
    id: &str,
    folder: &str,
    role: MemberRole,
    adopt_revision: Option<u32>,
) -> AppResult<()> {
    if !cfg!(debug_assertions) {
        return Err(AppError::forbidden("Only available in development builds"));
    }
    row(state, id).await?;
    let folder = PathBuf::from(folder);
    let src = Source::Dev(folder.clone());
    if src.latest_revision() == 0 {
        return Err(AppError::invalid_input(
            "That folder has no revisions/<n>.json",
        ));
    }
    let shared_id = format!("dev:{}", new_id());
    source::save_dev(&state.db, &shared_id, &folder).await?;
    db::instances::set_link(&state.db, id, Some((&shared_id, role))).await?;
    if let Some(n) = adopt_revision {
        adopt::adopt(&state.db, id, n, &src.manifest(state, n).await?).await?;
    }
    Ok(())
}

/// Stop being linked: the folder stays, now only on this PC.
pub async fn unlink(state: &AppState, id: &str) -> AppResult<()> {
    let row = row(state, id).await?;
    if let Some(shared) = &row.shared_instance_id {
        source::forget(&state.db, shared).await?;
    }
    db::instances::set_link(&state.db, id, None).await?;
    sqlx::query("DELETE FROM instance_content WHERE instance_id = ? AND installed = 0")
        .bind(id)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE instance_content SET is_managed = 0, published_share_mode = NULL WHERE instance_id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(())
}
