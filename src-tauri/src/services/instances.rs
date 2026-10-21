//! Tracked instances: tracking, summaries for the UI, metadata refresh, opening.
//!
//! A rescan = launcher metadata (T1) + the file scan (T2). Sync info (T4/T7) is still
//! a placeholder here.

use std::path::Path;
use std::time::Duration;

use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;

use super::{clean_path, launchers as launcher_service};
use crate::db::{
    self,
    instances::{InstanceRow, Metadata, NewInstance},
};
use crate::dto::{
    InstanceDetail, InstanceStatus, InstanceSummary, LauncherKind, LauncherRef, MemberRole,
    ShareMode, TrackInstanceRef, UpdateInstanceInput, from_wire, to_wire,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::launchers::{self, FoundInstance, OpenError};
use crate::scan;
use crate::state::AppState;

fn not_found() -> AppError {
    AppError::not_found("Instance not found")
}

async fn row(state: &AppState, id: &str) -> AppResult<InstanceRow> {
    db::instances::get(&state.db, id).await?.ok_or_else(not_found)
}

fn metadata(found: &FoundInstance) -> Metadata {
    Metadata {
        name: found.name.clone(),
        icon: found.icon.clone(),
        minecraft_version: found.minecraft_version.clone(),
        loader: found.loader,
        loader_version: found.loader_version.clone(),
        last_played_at: found.last_played_at.clone(),
        total_playtime_seconds: found.total_playtime_seconds.map(|s| s as i64),
    }
}

/// A row as the UI's `InstanceSummary`.
pub fn summary(row: &InstanceRow, content_count: u32, is_running: bool) -> InstanceSummary {
    let kind = row.kind();
    InstanceSummary {
        id: row.id.clone(),
        name: row.name.clone(),
        icon: row.icon.clone(),
        launcher: LauncherRef { kind, name: launchers::display_name(kind).into() },
        game_dir: row.game_dir.clone(),
        minecraft_version: row.minecraft_version.clone(),
        loader: row.loader(),
        loader_version: row.loader_version.clone(),
        status: row.status(), // T4: "syncing" from the job manager
        is_favorite: row.is_favorite,
        hide_activity: row.hide_activity,
        content_count,
        last_played_at: row.last_played_at.clone(),
        total_playtime_seconds: row.total_playtime_seconds.map(|s| s.max(0) as u64),
        is_running,
        tracked_at: row.tracked_at.clone(),
        last_scanned_at: row.last_scanned_at.clone(),
        sync: None, // T4/T7: shared instances
        progress: None,
    }
}

async fn summary_of(state: &AppState, row: &InstanceRow) -> AppResult<InstanceSummary> {
    let counts = db::instances::content_counts(&state.db).await?;
    Ok(summary(row, counts.get(&row.id).copied().unwrap_or(0), state.is_running(&row.id)))
}

/// Every tracked instance. No file or network access, so it's always fast.
pub async fn list(state: &AppState) -> AppResult<Vec<InstanceSummary>> {
    let counts = db::instances::content_counts(&state.db).await?;
    Ok(db::instances::list(&state.db)
        .await?
        .iter()
        .map(|r| summary(r, counts.get(&r.id).copied().unwrap_or(0), state.is_running(&r.id)))
        .collect())
}

pub async fn detail(state: &AppState, id: &str) -> AppResult<InstanceDetail> {
    let row = row(state, id).await?;
    let content: Vec<_> = db::content::list(&state.db, id).await?.iter().map(|c| c.to_dto()).collect();
    let backups = db::backups::list(&state.db, id).await?.iter().map(|b| b.to_dto()).collect();
    let count = content.iter().filter(|c| c.installed).count() as u32;
    Ok(InstanceDetail { summary: summary(&row, count, state.is_running(id)), content, backups })
}

/// Track instances found in launchers. Ones already tracked, or no longer in their
/// launcher, are skipped. Returns the newly tracked ones right away; their files are
/// scanned in the background (a big modpack takes a while to hash), and each sends
/// `instance.changed` when its scan is done.
pub async fn track<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    refs: &[TrackInstanceRef],
) -> AppResult<Vec<InstanceSummary>> {
    let mut added = Vec::new();
    for r in refs {
        let Some((launcher, found)) =
            launcher_service::find_instance(state, r.launcher_kind, &r.launcher_instance_id).await?
        else {
            tracing::warn!(id = %r.launcher_instance_id, "instance to track not found in its launcher");
            continue;
        };
        let game_dir = clean_path(&found.game_dir);
        if db::instances::find_by_game_dir(&state.db, &game_dir).await?.is_some() {
            continue;
        }
        let row = db::instances::insert(
            &state.db,
            NewInstance {
                launcher_id: Some(launcher.id),
                kind: r.launcher_kind,
                launcher_instance_id: Some(found.launcher_instance_id.clone()),
                game_dir,
                meta: metadata(&found),
            },
        )
        .await?;
        tracing::info!(name = %row.name, "tracking instance");
        added.push(summary(&row, 0, false));
        scan_in_background(state, app, &row.id);
    }
    Ok(added)
}

fn scan_in_background<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) {
    let (state, app, id) = (state.clone(), app.clone(), id.to_string());
    tauri::async_runtime::spawn(async move {
        if let Err(e) = rescan(&state, &app, &id).await {
            tracing::warn!(instance = %id, error = %e, "scan failed");
        }
    });
}

/// Track a game folder that isn't in a known launcher.
pub async fn track_folder<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    game_dir: &str,
) -> AppResult<InstanceSummary> {
    let path = Path::new(game_dir.trim());
    let looks_like_game_dir = ["mods", "config", "resourcepacks"].iter().any(|d| path.join(d).is_dir());
    if !path.is_dir() || !looks_like_game_dir {
        return Err(AppError::invalid_input("Choose a folder that contains mods or config"));
    }
    let game_dir = clean_path(path);
    if db::instances::find_by_game_dir(&state.db, &game_dir).await?.is_some() {
        return Err(AppError::conflict("That folder is already tracked"));
    }
    let name = Path::new(&game_dir)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Folder".into());
    let row = db::instances::insert(
        &state.db,
        NewInstance {
            launcher_id: None,
            kind: LauncherKind::Folder,
            launcher_instance_id: None,
            game_dir,
            meta: Metadata {
                name,
                icon: None,
                // No launcher metadata to read. T3 can guess from the mods' own metadata.
                minecraft_version: None,
                loader: None,
                loader_version: None,
                last_played_at: None,
                total_playtime_seconds: None,
            },
        },
    )
    .await?;
    scan_in_background(state, app, &row.id);
    Ok(summary(&row, 0, false))
}

/// Stop tracking. Deletes Atlas's rows and backups only; the folder is never touched.
pub async fn untrack(state: &AppState, id: &str) -> AppResult<()> {
    // T4: refuse while a job is running ("Wait for the update to finish").
    if !db::instances::delete(&state.db, id).await? {
        return Err(not_found());
    }
    state.watcher.unwatch(id);
    state.running.remove(id);
    let backups = state.paths.backups.join(id);
    if backups.exists()
        && let Err(e) = std::fs::remove_dir_all(&backups)
    {
        tracing::warn!(error = %e, "couldn't delete backups of untracked instance");
    }
    Ok(())
}

pub async fn update(state: &AppState, id: &str, patch: &UpdateInstanceInput) -> AppResult<InstanceSummary> {
    row(state, id).await?;
    db::instances::update_prefs(&state.db, id, patch.is_favorite, patch.hide_activity).await?;
    // T6: if hideActivity changed while this instance is running, resend presence.
    summary_of(state, &row(state, id).await?).await
}

/// Re-read the launcher's metadata and whether the folder still exists.
/// Returns the updated row and whether anything changed.
async fn refresh_metadata(state: &AppState, id: &str) -> AppResult<(InstanceRow, bool)> {
    let before = row(state, id).await?;
    let found = match (&before.launcher_id, &before.launcher_instance_id) {
        (Some(launcher_id), Some(lid)) => match db::launchers::get(&state.db, launcher_id).await? {
            Some(launcher) => launcher_service::instances_in(&launcher).await.into_iter().find(|f| {
                f.launcher_instance_id == *lid || clean_path(&f.game_dir).eq_ignore_ascii_case(&before.game_dir)
            }),
            None => None,
        },
        _ => None,
    };
    let status = if Path::new(&before.game_dir).is_dir() {
        InstanceStatus::Ready
    } else {
        InstanceStatus::Missing
    };
    let meta = found.as_ref().map(metadata);
    let changed = status != before.status() || meta.as_ref().is_some_and(|m| *m != Metadata::of(&before));
    db::instances::update_scan(&state.db, id, meta.as_ref(), status).await?;
    Ok((row(state, id).await?, changed))
}

/// Re-read an instance: launcher metadata, then its files. Sends `instance.changed`
/// when anything differs, and keeps the folder watched while it exists.
pub async fn rescan<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) -> AppResult<InstanceSummary> {
    let _scanning = state.scan_lock(id).await;
    let (row, meta_changed) = refresh_metadata(state, id).await?;
    let game_dir = Path::new(&row.game_dir);
    let files_changed = if row.status() == InstanceStatus::Ready {
        if !state.watcher.is_watching(id) {
            state.watcher.watch(id, game_dir);
        }
        let result = scan::files::scan(&state.db, id, game_dir).await?;
        if result.changed() {
            tracing::debug!(instance = %row.name, ?result, "content changed");
        }
        result.changed()
    } else {
        state.watcher.unwatch(id);
        false
    };
    if meta_changed || files_changed {
        emit(app, AtlasEvent::InstanceChanged { instance_id: id.to_string() });
    }
    summary_of(state, &row).await
}

/// Rescan every tracked instance whose last scan is older than `older_than`
/// (Duration::ZERO = all of them). Used at startup and when the window gets focus.
pub async fn rescan_all<R: Runtime>(state: &AppState, app: &AppHandle<R>, older_than: Duration) {
    let rows = match db::instances::list(&state.db).await {
        Ok(rows) => rows,
        Err(e) => return tracing::warn!(error = %e, "couldn't list instances to rescan"),
    };
    let cutoff = crate::util::time::iso(chrono::Utc::now() - older_than);
    for r in rows {
        let stale = older_than.is_zero() || r.last_scanned_at.as_deref().is_none_or(|t| t < cutoff.as_str());
        if stale && let Err(e) = rescan(state, app, &r.id).await {
            tracing::warn!(instance = %r.name, error = %e, "rescan failed");
        }
    }
}

pub async fn open_in_launcher(state: &AppState, id: &str) -> AppResult<()> {
    let row = row(state, id).await?;
    if row.status() == InstanceStatus::Missing {
        return Err(AppError::conflict("The folder is missing"));
    }
    let kind = row.kind();
    if kind == LauncherKind::Folder {
        return Err(AppError::conflict("This folder isn't from a launcher Atlas knows"));
    }
    let name = launchers::display_name(kind);
    let launcher = match &row.launcher_id {
        Some(lid) => db::launchers::get(&state.db, lid).await?,
        None => None,
    }
    .ok_or_else(|| AppError::conflict(format!("Atlas can't find {name} on this PC anymore")))?;
    let instance = row.launcher_instance_id.as_deref().unwrap_or_default();
    launchers::open(kind, Path::new(&launcher.root_path), instance).map_err(|e| match e {
        OpenError::NoProgram => AppError::conflict(format!("Couldn't find {name}. Is it installed?")),
        OpenError::Spawn(e) => {
            tracing::warn!(error = %e, "couldn't start launcher");
            AppError::unknown(format!("Couldn't start {name}"))
        }
    })
}

pub async fn open_folder<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) -> AppResult<()> {
    let row = row(state, id).await?;
    if row.status() == InstanceStatus::Missing || !Path::new(&row.game_dir).is_dir() {
        return Err(AppError::conflict("The folder is missing"));
    }
    app.opener().open_path(&row.game_dir, None::<&str>).map_err(|e| {
        tracing::warn!(error = %e, "couldn't open folder");
        AppError::unknown("Couldn't open the folder")
    })
}

/// Turn a file on or off by renaming it to/from ".disabled", like every launcher does.
pub async fn set_content_enabled<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    instance_id: &str,
    content_id: &str,
    enabled: bool,
) -> AppResult<()> {
    let instance = row(state, instance_id).await?;
    if state.is_running(instance_id) {
        return Err(AppError::conflict("Close the game first"));
    }
    let item = db::content::get(&state.db, instance_id, content_id)
        .await?
        .ok_or_else(|| AppError::not_found("Content not found"))?;
    if !item.installed {
        return Err(AppError::invalid_input("That item isn't downloaded"));
    }
    if item.enabled == enabled {
        return Ok(());
    }
    let _scanning = state.scan_lock(instance_id).await; // don't race the watcher's rescan
    let on = Path::new(&instance.game_dir).join(&item.path);
    let off = Path::new(&instance.game_dir).join(format!("{}.disabled", item.path));
    let (from, to) = if enabled { (off, on) } else { (on, off) };
    if to.exists() {
        return Err(AppError::conflict("A file with that name is already there"));
    }
    rename_with_retry(&from, &to).await?;
    db::content::set_enabled(&state.db, content_id, enabled).await?;
    emit(app, AtlasEvent::InstanceChanged { instance_id: instance_id.to_string() });
    Ok(())
}

/// Antivirus and launchers hold new files open for a moment; retry briefly before giving up.
async fn rename_with_retry(from: &Path, to: &Path) -> AppResult<()> {
    let mut attempt = 0;
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            // 32 = sharing violation, 33 = lock violation (Windows)
            Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) && attempt < 5 => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(AppError::conflict("The file isn't there anymore. Atlas will rescan the folder."));
            }
            Err(e) => {
                tracing::warn!(error = %e, from = %from.display(), "rename failed");
                return Err(AppError::conflict("A file is in use. Close the launcher and try again."));
            }
        }
    }
}

/// Owners/editors: how a file is shared. Saved locally; it reaches friends with the next publish.
pub async fn set_share_mode<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    instance_id: &str,
    content_id: &str,
    mode: ShareMode,
) -> AppResult<()> {
    let instance = row(state, instance_id).await?;
    if instance.shared_role.as_deref().and_then(from_wire) == Some(MemberRole::Viewer) {
        return Err(AppError::forbidden("Only the owner and editors choose how files are shared"));
    }
    db::content::get(&state.db, instance_id, content_id)
        .await?
        .ok_or_else(|| AppError::not_found("Content not found"))?;
    db::content::set_share_mode(&state.db, content_id, &to_wire(mode)).await?;
    emit(app, AtlasEvent::InstanceChanged { instance_id: instance_id.to_string() });
    Ok(())
}
