//! Apply a version to an instance folder: the body of update, first install, revert and
//! "download this optional item" jobs (tauri-plan §12.3). Stages:
//!
//!   waiting_for_game_exit → backup → download (→ waiting_for_manual_downloads) → apply
//!
//! Files are assembled in `<game dir>/.atlas-staging/<job>/` (same drive, so the final
//! step is a rename), with a journal of every change made, so a crash in the middle can
//! be undone at the next start (recovery.rs). Anything that fails before "apply" leaves
//! the folder untouched.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Runtime;
use tauri_plugin_opener::OpenerExt;

use super::backup::{self, FileAction, FileRecord};
use super::fetch::{self, Route};
use super::manifest::Manifest;
use super::plan::{self, Action, Plan};
use super::source::Source;
use super::{adopt, prism_versions};
use crate::db::{self, instances::InstanceRow};
use crate::dto::{BackupReason, ConfigChoice, SyncStage};
use crate::error::{AppError, AppResult};
use crate::jobs::Progress;
use crate::scan;
use crate::state::AppState;
use crate::util::ids::new_id;

pub const STAGING: &str = ".atlas-staging";

pub struct Request {
    pub revision: u32,
    pub manifest: Manifest,
    pub reason: BackupReason,
    pub install_optional: HashSet<String>,
    pub config_choices: HashMap<String, ConfigChoice>,
}

/// sha256 of every config path either version mentions, as it is on disk now.
fn config_hashes(game_dir: &Path, synced: Option<&Manifest>, target: &Manifest) -> HashMap<String, String> {
    let paths = synced.into_iter().flat_map(|m| &m.overrides).chain(&target.overrides).map(|o| o.path.clone());
    paths
        .filter_map(|p| {
            let bytes = std::fs::read(game_dir.join(&p)).ok()?;
            Some((p.to_lowercase(), hex::encode(Sha256::digest(bytes))))
        })
        .collect()
}

/// Work out the plan for this instance and target (used by preview too).
pub async fn make_plan(
    state: &AppState,
    row: &InstanceRow,
    target: &Manifest,
    install_optional: &HashSet<String>,
    config_choices: &HashMap<String, ConfigChoice>,
) -> AppResult<(Plan, Vec<db::content::ContentRow>, HashMap<String, String>)> {
    let rows = db::content::list(&state.db, &row.id).await?;
    let synced = row.synced_manifest();
    let game_dir = PathBuf::from(&row.game_dir);
    let (s, t) = (synced.clone(), target.clone());
    let configs = tokio::task::spawn_blocking(move || config_hashes(&game_dir, s.as_ref(), &t))
        .await
        .map_err(|e| AppError::unknown(e.to_string()))?;
    let lookup = |p: &str| configs.get(&p.to_lowercase()).cloned();
    let plan = plan::plan(&plan::Input {
        synced: synced.as_ref(),
        target,
        rows: &rows,
        install_optional,
        config_choices,
        config_on_disk: &lookup,
    });
    Ok((plan, rows, configs))
}

fn records(plan: &Plan) -> Vec<FileRecord> {
    let rec = |path: &str, action| FileRecord { path: path.to_string(), action };
    plan.actions
        .iter()
        .map(|a| match a {
            Action::Remove { path } => rec(path, FileAction::Removed),
            Action::Place { entry, replaces: true, .. } => rec(&entry.path, FileAction::Replaced),
            Action::Place { entry, .. } => rec(&entry.path, FileAction::Added),
            Action::WriteConfig { config, replaces: true } => rec(&config.path, FileAction::Replaced),
            Action::WriteConfig { config, .. } => rec(&config.path, FileAction::Added),
            Action::RemoveConfig { path } => rec(path, FileAction::Removed),
        })
        .collect()
}

/// What has been done so far, written after every change (see recovery.rs).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Journal {
    pub backup_dir: PathBuf,
    pub done: Vec<FileRecord>,
}

impl Journal {
    fn save(&self, staging: &Path) -> AppResult<()> {
        let tmp = staging.join("journal.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(self)?)?;
        std::fs::rename(tmp, staging.join("journal.json"))?;
        Ok(())
    }
}

pub async fn run<R: Runtime>(state: &AppState, progress: &Progress<R>, source: &Source, req: Request) -> AppResult<()> {
    let id = progress.instance_id.clone();

    // 1. Never write under a running game.
    if state.is_running(&id) {
        progress.stage(SyncStage::WaitingForGameExit);
        while state.is_running(&id) {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    let row = db::instances::get(&state.db, &id).await?.ok_or_else(|| AppError::not_found("Instance not found"))?;
    let game_dir = PathBuf::from(&row.game_dir);
    if !game_dir.is_dir() {
        return Err(AppError::conflict("The folder is missing"));
    }
    let (plan, _, _) = make_plan(state, &row, &req.manifest, &req.install_optional, &req.config_choices).await?;

    if !plan.actions.is_empty() {
        // 2. Back up everything that will change.
        progress.stage(SyncStage::Backup);
        let from = row.synced_revision.filter(|r| *r > 0).map(|r| r as u32);
        let (_, backup_dir) = backup::create(
            &state.db, &state.paths, &id, &game_dir, req.reason, from, Some(req.revision), records(&plan),
        )
        .await?;

        // 3. Get every file into the cache, verified.
        fetch_all(state, progress, source, &game_dir, &plan).await?;

        // 4. Stage, then move into place.
        progress.stage(SyncStage::Apply);
        let (paths, actions, game, backup_dir2) = (state.paths.clone(), plan.actions.clone(), game_dir.clone(), backup_dir);
        tokio::task::spawn_blocking(move || apply_actions(&paths, &game, &actions, backup_dir2))
            .await
            .map_err(|e| AppError::unknown(e.to_string()))??;
    }

    // 5. The launcher's own settings, where Atlas can edit them (Prism).
    prism_versions::update_if_needed(&row, &req.manifest);

    // 6. The database follows the folder.
    let _scanning = state.scan_lock(&id).await;
    scan::files::scan(&state.db, &id, &game_dir).await?;
    adopt::adopt(&state.db, &id, req.revision, &req.manifest).await?;
    Ok(())
}

async fn fetch_all<R: Runtime>(
    state: &AppState,
    progress: &Progress<R>,
    source: &Source,
    game_dir: &Path,
    plan: &Plan,
) -> AppResult<()> {
    let routes: Vec<_> = plan
        .places()
        .map(|(e, local)| (e, fetch::route(&state.paths, source, game_dir, e, local)))
        .collect();
    let total: u64 = routes.iter().filter(|(_, r)| *r != Route::Cached).map(|(e, _)| e.size_bytes).sum();
    progress.stage(SyncStage::Download);
    let mut done = 0u64;
    let mut manual = Vec::new();
    for (entry, route) in &routes {
        if let Route::Manual(m) = route {
            manual.push((*entry, m.clone()));
            continue;
        }
        let mut on_bytes = |n: u64| {
            done += n;
            progress.bytes(done.min(total), total);
        };
        fetch::content(&state.paths, &state.http, entry, route, &mut on_bytes).await?;
    }
    for a in &plan.actions {
        if let Action::WriteConfig { config, .. } = a {
            fetch::config(&state.paths, source, config).await?;
        }
    }
    if !manual.is_empty() {
        wait_for_manual_downloads(state, progress, &manual).await?;
    }
    progress.bytes(total, total);
    Ok(())
}

/// CurseForge files that can't be downloaded automatically: open their pages and pick the
/// files up from the Downloads folder as they arrive.
async fn wait_for_manual_downloads<R: Runtime>(
    state: &AppState,
    progress: &Progress<R>,
    manual: &[(&super::manifest::Entry, crate::dto::ManualDownload)],
) -> AppResult<()> {
    let since = SystemTime::now();
    for (_, m) in manual {
        if let Err(e) = progress.app.opener().open_url(&m.page_url, None::<&str>) {
            tracing::warn!(error = %e, "couldn't open download page");
        }
    }
    loop {
        let waiting: Vec<_> = manual
            .iter()
            .filter(|(e, _)| !fetch::cache_file(&state.paths, &e.sha1).is_file())
            .collect();
        if waiting.is_empty() {
            return Ok(());
        }
        progress.manual(waiting.iter().map(|(_, m)| m.clone()).collect());
        let entries: Vec<_> = waiting.iter().map(|(e, _)| (*e).clone()).collect();
        let paths = state.paths.clone();
        tokio::task::spawn_blocking(move || {
            let refs: Vec<_> = entries.iter().collect();
            fetch::pick_up_manual_downloads(&paths, &refs, since)
        })
        .await
        .map_err(|e| AppError::unknown(e.to_string()))?;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// Stage every file, then make the changes, journaling each one. Blocking.
fn apply_actions(paths: &crate::state::AppPaths, game_dir: &Path, actions: &[Action], backup_dir: PathBuf) -> AppResult<()> {
    let staging = game_dir.join(STAGING).join(new_id());
    std::fs::create_dir_all(&staging)?;
    hide(&game_dir.join(STAGING));

    // Copy from the cache into staging first: if that fails, nothing has changed yet.
    for a in actions {
        let (path, from) = match a {
            Action::Place { entry, .. } => (&entry.path, fetch::cache_file(paths, &entry.sha1)),
            Action::WriteConfig { config, .. } => (&config.path, fetch::blob_file(paths, &config.sha256)),
            _ => continue,
        };
        let to = staging.join("files").join(path);
        std::fs::create_dir_all(to.parent().expect("relative path has a parent"))?;
        std::fs::copy(&from, &to)?;
    }

    let mut journal = Journal { backup_dir, done: vec![] };
    journal.save(&staging)?;
    let result = (|| -> AppResult<()> {
        for a in actions {
            match a {
                Action::Remove { path } | Action::RemoveConfig { path } => {
                    remove(&game_dir.join(path))?;
                    journal.done.push(FileRecord { path: path.clone(), action: FileAction::Removed });
                }
                Action::Place { entry: _, replaces, .. } | Action::WriteConfig { config: _, replaces } => {
                    let path = match a {
                        Action::Place { entry, .. } => &entry.path,
                        Action::WriteConfig { config, .. } => &config.path,
                        _ => unreachable!(),
                    };
                    let target = game_dir.join(path);
                    if *replaces && target.exists() {
                        remove(&target)?;
                        journal.done.push(FileRecord { path: path.clone(), action: FileAction::Removed });
                        journal.save(&staging)?;
                    }
                    std::fs::create_dir_all(target.parent().expect("relative path has a parent"))?;
                    std::fs::rename(staging.join("files").join(path), &target)?;
                    journal.done.push(FileRecord { path: path.clone(), action: FileAction::Added });
                }
            }
            journal.save(&staging)?;
        }
        Ok(())
    })();

    if let Err(e) = result {
        // Undo what was done so far, then report the failure.
        super::recovery::undo(game_dir, &journal);
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_dir(game_dir.join(STAGING)); // only if empty
    Ok(())
}

/// Delete a file, retrying briefly if something (antivirus, the launcher) has it open.
fn remove(path: &Path) -> AppResult<()> {
    for attempt in 0..6 {
        match std::fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) && attempt < 5 => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(), "couldn't remove file");
                return Err(AppError::conflict("A file is in use. Close the launcher and try again."));
            }
        }
    }
    unreachable!()
}

/// Mark the staging folder hidden, so it doesn't confuse anyone browsing the game folder.
fn hide(path: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // FILE_ATTRIBUTE_HIDDEN = 0x2
        unsafe extern "system" {
            fn SetFileAttributesW(name: *const u16, attrs: u32) -> i32;
        }
        unsafe {
            SetFileAttributesW(wide.as_ptr(), 0x2);
        }
    }
    #[cfg(not(windows))]
    let _ = path;
}
