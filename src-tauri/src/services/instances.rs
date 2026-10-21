//! Tracked instances: tracking, summaries for the UI, metadata refresh, opening.
//!
//! T1 reads launcher metadata only. Content scanning, the running-game check (T2) and
//! sync info (T4/T7) fill in the fields that are still placeholders here.

use std::path::Path;

use tauri::{AppHandle, Runtime};
use tauri_plugin_opener::OpenerExt;

use super::{clean_path, launchers as launcher_service};
use crate::db::{
    self,
    instances::{InstanceRow, Metadata, NewInstance},
};
use crate::dto::{
    InstanceDetail, InstanceStatus, InstanceSummary, LauncherKind, LauncherRef, TrackInstanceRef,
    UpdateInstanceInput,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::launchers::{self, FoundInstance, OpenError};
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
pub fn summary(row: &InstanceRow, content_count: u32) -> InstanceSummary {
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
        is_running: false, // T2: game monitor
        tracked_at: row.tracked_at.clone(),
        last_scanned_at: row.last_scanned_at.clone(),
        sync: None, // T4/T7: shared instances
        progress: None,
    }
}

async fn summary_of(state: &AppState, row: &InstanceRow) -> AppResult<InstanceSummary> {
    let counts = db::instances::content_counts(&state.db).await?;
    Ok(summary(row, counts.get(&row.id).copied().unwrap_or(0)))
}

/// Every tracked instance. No file or network access, so it's always fast.
pub async fn list(state: &AppState) -> AppResult<Vec<InstanceSummary>> {
    let counts = db::instances::content_counts(&state.db).await?;
    Ok(db::instances::list(&state.db)
        .await?
        .iter()
        .map(|r| summary(r, counts.get(&r.id).copied().unwrap_or(0)))
        .collect())
}

pub async fn detail(state: &AppState, id: &str) -> AppResult<InstanceDetail> {
    let row = row(state, id).await?;
    let content: Vec<_> = db::content::list(&state.db, id).await?.iter().map(|c| c.to_dto()).collect();
    let backups = db::backups::list(&state.db, id).await?.iter().map(|b| b.to_dto()).collect();
    let count = content.iter().filter(|c| c.installed).count() as u32;
    Ok(InstanceDetail { summary: summary(&row, count), content, backups })
}

/// Track instances found in launchers. Ones already tracked, or no longer in their
/// launcher, are skipped. Returns the newly tracked ones.
pub async fn track(state: &AppState, refs: &[TrackInstanceRef]) -> AppResult<Vec<InstanceSummary>> {
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
        added.push(summary(&row, 0));
    }
    Ok(added)
}

/// Track a game folder that isn't in a known launcher.
pub async fn track_folder(state: &AppState, game_dir: &str) -> AppResult<InstanceSummary> {
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
    Ok(summary(&row, 0))
}

/// Stop tracking. Deletes Atlas's rows and backups only; the folder is never touched.
pub async fn untrack(state: &AppState, id: &str) -> AppResult<()> {
    // T4: refuse while a job is running ("Wait for the update to finish").
    if !db::instances::delete(&state.db, id).await? {
        return Err(not_found());
    }
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
/// Returns the new summary and whether anything changed.
pub async fn refresh(state: &AppState, id: &str) -> AppResult<(InstanceSummary, bool)> {
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
    Ok((summary_of(state, &row(state, id).await?).await?, changed))
}

/// Refresh every tracked instance (at startup), telling the UI about the ones that changed.
pub async fn refresh_all<R: Runtime>(state: &AppState, app: &AppHandle<R>) {
    let rows = match db::instances::list(&state.db).await {
        Ok(rows) => rows,
        Err(e) => return tracing::warn!(error = %e, "couldn't list instances to refresh"),
    };
    for r in rows {
        match refresh(state, &r.id).await {
            Ok((_, true)) => emit(app, AtlasEvent::InstanceChanged { instance_id: r.id }),
            Ok(_) => {}
            Err(e) => tracing::warn!(instance = %r.name, error = %e, "refresh failed"),
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
