//! Launchers: detection, the `launcher` table, and the instances inside each install.

use std::collections::HashMap;
use std::path::Path;

use tauri::{AppHandle, Runtime};

use super::clean_path;
use crate::db::{self, launchers::LauncherRow};
use crate::dto::{DetectedInstance, Launcher, LauncherKind};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::launchers::{self, FoundInstance};
use crate::state::AppState;

/// Run detection once per session, the first time anything needs launchers.
/// Concurrent callers (the Track dialog asks for two lists at once) wait for the same run.
pub async fn ensure_detected(state: &AppState) -> AppResult<()> {
    if !state.auto_detect {
        return Ok(());
    }
    state
        .detected
        .get_or_try_init(|| async { detect_and_save(state).await.map(|_| ()) })
        .await?;
    Ok(())
}

/// Look in every launcher's default folders and save new installs. Auto-detected rows
/// whose folder no longer holds that launcher are removed; rows the user added stay.
/// Returns whether anything changed.
async fn detect_and_save(state: &AppState) -> AppResult<bool> {
    let mut changed = false;
    for kind in launchers::KINDS {
        for root in launchers::detect(kind) {
            let root = clean_path(&root);
            if db::launchers::find_by_root(&state.db, &root)
                .await?
                .is_none()
            {
                db::launchers::insert(&state.db, kind, &root, false).await?;
                tracing::info!(?kind, %root, "found launcher");
                changed = true;
            }
        }
    }
    for row in db::launchers::list(&state.db).await? {
        let gone = match row.kind() {
            Some(kind) => !launchers::is_root(kind, Path::new(&row.root_path)),
            None => true,
        };
        if gone && !row.added_by_user {
            db::launchers::delete(&state.db, &row.id).await?;
            tracing::info!(root = %row.root_path, "launcher no longer installed");
            changed = true;
        }
    }
    Ok(changed)
}

/// Every instance in one launcher install. A launcher that can't be read gives an
/// empty list (and a log line) rather than failing the whole request.
pub async fn instances_in(row: &LauncherRow) -> Vec<FoundInstance> {
    let Some(kind) = row.kind() else {
        return vec![];
    };
    match launchers::list_instances(kind, Path::new(&row.root_path)).await {
        Ok(list) => list,
        Err(e) => {
            tracing::warn!(root = %row.root_path, error = %e, "couldn't list instances");
            vec![]
        }
    }
}

async fn to_dto(row: &LauncherRow) -> Option<Launcher> {
    let kind = row.kind()?;
    Some(Launcher {
        id: row.id.clone(),
        kind,
        name: launchers::display_name(kind).into(),
        root_path: row.root_path.clone(),
        instance_count: instances_in(row).await.len() as u32,
        creatable_loaders: launchers::creatable_loaders(kind).to_vec(),
        added_by_user: row.added_by_user,
    })
}

pub async fn list(state: &AppState) -> AppResult<Vec<Launcher>> {
    ensure_detected(state).await?;
    let mut out = Vec::new();
    for row in db::launchers::list(&state.db).await? {
        out.extend(to_dto(&row).await);
    }
    Ok(out)
}

/// "Look again": re-run detection now.
pub async fn detect<R: Runtime>(state: &AppState, app: &AppHandle<R>) -> AppResult<Vec<Launcher>> {
    if state.auto_detect && detect_and_save(state).await? {
        emit(app, AtlasEvent::LaunchersChanged {});
    }
    list(state).await
}

pub async fn add<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    kind: LauncherKind,
    root: &str,
) -> AppResult<Launcher> {
    let name = launchers::display_name(kind);
    if kind == LauncherKind::Folder || !launchers::is_root(kind, Path::new(root)) {
        return Err(AppError::invalid_input(format!(
            "That folder isn't a {name} install"
        )));
    }
    let root = clean_path(Path::new(root));
    if db::launchers::find_by_root(&state.db, &root)
        .await?
        .is_some()
    {
        return Err(AppError::conflict(format!(
            "{name} in that folder is already added"
        )));
    }
    let row = db::launchers::insert(&state.db, kind, &root, true).await?;
    emit(app, AtlasEvent::LaunchersChanged {});
    to_dto(&row)
        .await
        .ok_or_else(|| AppError::unknown("Couldn't add the launcher"))
}

pub async fn remove<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) -> AppResult<()> {
    if !db::launchers::delete(&state.db, id).await? {
        return Err(AppError::not_found("Launcher not found"));
    }
    emit(app, AtlasEvent::LaunchersChanged {});
    Ok(())
}

/// Every instance in every known launcher, with `instanceId` set when it's tracked.
pub async fn detected_instances(state: &AppState) -> AppResult<Vec<DetectedInstance>> {
    ensure_detected(state).await?;
    let tracked = db::instances::list(&state.db).await?;
    let by_ref: HashMap<(String, String), String> = tracked
        .iter()
        .filter_map(|i| {
            Some((
                (i.launcher_kind.clone(), i.launcher_instance_id.clone()?),
                i.id.clone(),
            ))
        })
        .collect();
    let by_dir: HashMap<String, String> = tracked
        .iter()
        .map(|i| (i.game_dir.to_lowercase(), i.id.clone()))
        .collect();

    let mut out = Vec::new();
    for row in db::launchers::list(&state.db).await? {
        let Some(kind) = row.kind() else { continue };
        for found in instances_in(&row).await {
            let game_dir = found.game_dir.to_string_lossy().into_owned();
            let instance_id = by_ref
                .get(&(row.kind.clone(), found.launcher_instance_id.clone()))
                .or_else(|| by_dir.get(&game_dir.to_lowercase()))
                .cloned();
            out.push(DetectedInstance {
                launcher_kind: kind,
                launcher_instance_id: found.launcher_instance_id,
                name: found.name,
                game_dir,
                minecraft_version: found.minecraft_version,
                loader: found.loader,
                loader_version: found.loader_version,
                instance_id,
            });
        }
    }
    Ok(out)
}

/// The launcher install and instance a tracking request refers to.
pub async fn find_instance(
    state: &AppState,
    kind: LauncherKind,
    launcher_instance_id: &str,
) -> AppResult<Option<(LauncherRow, FoundInstance)>> {
    ensure_detected(state).await?;
    for row in db::launchers::list(&state.db).await? {
        if row.kind() != Some(kind) {
            continue;
        }
        if let Some(found) = instances_in(&row)
            .await
            .into_iter()
            .find(|f| f.launcher_instance_id == launcher_instance_id)
        {
            return Ok(Some((row, found)));
        }
    }
    Ok(None)
}
