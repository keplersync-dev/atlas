//! Apply a version to an instance folder: the body of update, first install, revert and
//! "download this optional item" jobs. Stages:
//!
//!   waiting_for_game_exit → backup → download (→ waiting_for_manual_downloads) → apply
//!
//! Files are assembled in `<game dir>/.atlas-staging/<job>/` (same drive, so the final
//! step is a rename), with a journal of every change made, so a crash in the middle can
//! be undone at the next start (recovery.rs). Anything that fails before "apply" leaves
//! the folder untouched. Restoring a backup goes the same way (`restore_files`).

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Runtime;
use tauri_plugin_opener::OpenerExt;

use super::backup::{self, FileAction, FileRecord};
use super::fetch::{self, Route};
use super::manifest::{Entry, Keep, Manifest, Override, PackBase};
use super::pack;
use super::plan::{self, Action, Plan};
use super::source::Source;
use super::{adopt, prism_versions};
use crate::cloud::files::LinkBatches;
use crate::db::{self, instances::InstanceRow};
use crate::dto::{BackupReason, ConfigChoice, SyncStage};
use crate::error::{AppError, AppResult};
use crate::jobs::{self, Progress};
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

/// sha256 of every config path either version mentions, as it is on disk now. A file
/// set the same as the synced version (a mod only rewrote it) counts as that version, so
/// an update doesn't take it for the member's own change. `cached` reads a version's
/// file by sha256, when it's still in the download cache.
fn config_hashes(
    game_dir: &Path,
    synced: Option<&Manifest>,
    target: &Manifest,
    cached: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> HashMap<String, String> {
    let base: HashMap<String, &str> = synced
        .into_iter()
        .flat_map(|m| &m.overrides)
        .map(|o| (o.path.to_lowercase(), o.sha256.as_str()))
        .collect();
    let paths = synced
        .into_iter()
        .flat_map(|m| &m.overrides)
        .chain(&target.overrides)
        .map(|o| o.path.clone());
    paths
        .filter_map(|p| {
            let bytes = std::fs::read(game_dir.join(&p)).ok()?;
            let key = p.to_lowercase();
            let sha = hex::encode(Sha256::digest(&bytes));
            let rewritten = base.get(&key).filter(|b| **b != sha).is_some_and(|b| {
                let mine = super::settings::fingerprint(&p, &bytes);
                mine.is_some()
                    && cached(b)
                        .is_some_and(|theirs| super::settings::fingerprint(&p, &theirs) == mine)
            });
            Some((
                key.clone(),
                if rewritten {
                    base[&key].to_owned()
                } else {
                    sha
                },
            ))
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
    let paths = state.paths.clone();
    let configs = tokio::task::spawn_blocking(move || {
        let cached = |sha: &str| std::fs::read(fetch::blob_file(&paths, sha)).ok();
        config_hashes(&game_dir, s.as_ref(), &t, &cached)
    })
    .await
    .map_err(AppError::from)?;
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
    let rec = |path: &str, action| FileRecord {
        path: path.to_string(),
        action,
    };
    plan.actions
        .iter()
        .map(|a| match a {
            Action::Remove { path } => rec(path, FileAction::Removed),
            Action::Place {
                entry,
                replaces: true,
                ..
            } => rec(&entry.path, FileAction::Replaced),
            Action::Place { entry, .. } => rec(&entry.path, FileAction::Added),
            Action::WriteConfig {
                config,
                replaces: true,
            } => rec(&config.path, FileAction::Replaced),
            Action::WriteConfig { config, .. } => rec(&config.path, FileAction::Added),
            Action::RemoveConfig { path } => rec(path, FileAction::Removed),
        })
        .collect()
}

/// What has been done so far (see recovery.rs). On disk, `journal.jsonl` in the staging
/// folder: the backup folder on the first line, then one line per change, each added
/// before the change is made.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Journal {
    pub backup_dir: PathBuf,
    pub done: Vec<FileRecord>,
    #[serde(skip)]
    file: Option<std::fs::File>,
}

const JOURNAL: &str = "journal.jsonl";

impl Journal {
    fn start(staging: &Path, backup_dir: PathBuf) -> AppResult<Self> {
        let mut file = std::fs::File::create(staging.join(JOURNAL))?;
        file.write_all(format!("{}\n", serde_json::to_string(&backup_dir)?).as_bytes())?;
        Ok(Self {
            backup_dir,
            done: vec![],
            file: Some(file),
        })
    }

    fn record(&mut self, path: &str, action: FileAction) -> AppResult<()> {
        let step = FileRecord {
            path: path.to_string(),
            action,
        };
        if let Some(file) = &mut self.file {
            file.write_all(format!("{}\n", serde_json::to_string(&step)?).as_bytes())?;
        }
        self.done.push(step);
        Ok(())
    }

    /// The journal a job left in `staging`, if it got as far as starting one. A last line
    /// cut short by a crash is left out: that change hadn't been made. Older versions
    /// wrote the whole journal as one JSON file.
    pub fn read(staging: &Path) -> Option<Self> {
        let Ok(text) = std::fs::read_to_string(staging.join(JOURNAL)) else {
            let old = std::fs::read(staging.join("journal.json")).ok()?;
            return serde_json::from_slice(&old).ok();
        };
        let mut lines = text.lines();
        let backup_dir = serde_json::from_str(lines.next()?).ok()?;
        let done = lines.map_while(|l| serde_json::from_str(l).ok()).collect();
        Some(Self {
            backup_dir,
            done,
            file: None,
        })
    }
}

pub async fn run<R: Runtime>(
    state: &AppState,
    progress: &Progress<R>,
    source: &Source,
    req: Request,
) -> AppResult<()> {
    let id = progress.instance_id.clone();

    // 1. Never write under a running game.
    progress.wait_for_game_exit(state).await?;

    let row = db::instances::require(&state.db, &id).await?;
    let game_dir = PathBuf::from(&row.game_dir);
    if !game_dir.is_dir() {
        return Err(AppError::conflict(
            "The instance's folder is missing. If you moved it, add it to Atlas again from its new place.",
        ));
    }
    let (plan, _, _) = make_plan(
        state,
        &row,
        &req.manifest,
        &req.install_optional,
        &req.config_choices,
    )
    .await?;

    if !plan.actions.is_empty() {
        // 2. Back up everything that will change.
        progress.stage(SyncStage::Backup);
        let from = row.synced_revision.filter(|r| *r > 0).map(|r| r as u32);
        let (backup_id, backup_dir) = backup::create(
            &state.db,
            &state.paths,
            &id,
            &game_dir,
            req.reason,
            from,
            Some(req.revision),
            records(&plan),
        )
        .await?;

        // 3. Get every file into the cache, verified, then check the game wasn't started
        // meanwhile. Nothing has changed yet, so stopping here drops the backup too.
        let ready = async {
            tokio::select! {
                fetched = fetch_all(state, progress, source, &game_dir, &plan, req.manifest.base.as_ref()) => fetched,
                _ = progress.cancelled() => Err(jobs::cancelled()),
            }?;
            progress.wait_for_game_exit(state).await
        }
        .await;
        if let Err(e) = ready {
            backup::discard(&state.db, &state.paths, &id, &backup_id).await;
            return Err(e);
        }

        // 4. Stage, then move into place. From here on the job can't be cancelled.
        progress.stage(SyncStage::Apply);
        let (paths, actions, game, backup_dir2) = (
            state.paths.clone(),
            plan.actions.clone(),
            game_dir.clone(),
            backup_dir,
        );
        // Big packs have thousands of files: say how far along it is, not 0% for minutes.
        let total = apply_steps(&actions);
        let done = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&done);
        let mut applying = tokio::task::spawn_blocking(move || {
            apply_actions(&paths, &game, &actions, backup_dir2, &counter)
        });
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                applied = &mut applying => break applied.map_err(AppError::from)??,
                _ = tick.tick() => progress.bytes(done.load(Ordering::Relaxed), total),
            }
        }
    }

    // 5. The launcher's own settings, where Atlas can edit them (Prism).
    prism_versions::update_if_needed(&row, &req.manifest);

    // 6. The database follows the folder.
    let _scanning = state.scan_lock(&id).await;
    scan::files::scan(&state.db, &id, &game_dir, None).await?;
    adopt::adopt(&state.db, &id, req.revision, &req.manifest).await?;
    scan::icons::fill(&state.db, &state.paths.icons, &id, &game_dir).await?;
    Ok(())
}

/// How many files download at the same time.
const PARALLEL_DOWNLOADS: usize = 6;

async fn fetch_all<R: Runtime>(
    state: &AppState,
    progress: &Progress<R>,
    source: &Source,
    game_dir: &Path,
    plan: &Plan,
    base: Option<&PackBase>,
) -> AppResult<()> {
    let routes: Vec<_> = plan
        .places()
        .map(|(e, local)| (e, fetch::route(&state.paths, source, game_dir, e, local)))
        .collect();
    // The modpack's own files come out of its download, and so do its mods.
    let pack_mod = |e: &Entry, r: &Route| e.in_pack && *r == Route::Blob;
    let from_pack: Vec<Override> = plan
        .actions
        .iter()
        .filter_map(|a| match a {
            Action::WriteConfig { config, .. }
                if config.in_pack && !fetch::blob_file(&state.paths, &config.sha256).is_file() =>
            {
                Some(config.clone())
            }
            _ => None,
        })
        .chain(
            routes
                .iter()
                .filter(|(e, r)| pack_mod(e, r))
                .map(|(e, _)| Override {
                    path: e.path.clone(),
                    sha256: e.sha256.clone(),
                    size_bytes: e.size_bytes,
                    in_pack: true,
                    keep: None,
                }),
        )
        .collect();
    let pack_bytes = match base {
        Some(b) if !from_pack.is_empty() && pack::needs_download(&state.paths, b) => b.size_bytes,
        _ => 0,
    };
    let total = routes
        .iter()
        .filter(|(e, r)| *r != Route::Cached && !pack_mod(e, r))
        .fold(pack_bytes, |sum, (e, _)| sum.saturating_add(e.size_bytes));
    progress.stage(SyncStage::Download);
    // Uploaded files and config files come from the server: their links in batches.
    let links = source.server_id().map(|shared| {
        let blobs = routes
            .iter()
            .filter(|(e, r)| {
                *r == Route::Blob && !e.in_pack && !fetch::content_file(&state.paths, e).is_file()
            })
            .map(|(e, _)| e.sha256.clone());
        let configs = plan.actions.iter().filter_map(|a| match a {
            Action::WriteConfig { config, .. }
                if !config.in_pack && !fetch::blob_file(&state.paths, &config.sha256).is_file() =>
            {
                Some(config.sha256.clone())
            }
            _ => None,
        });
        LinkBatches::new(&state.cloud, shared, blobs.chain(configs).collect())
    });
    let links = links.as_ref();
    // Several files at a time; the first failure stops the rest (nothing is placed until
    // every file is in the cache).
    let done = AtomicU64::new(0);
    let manual: Vec<_> = routes
        .iter()
        .filter_map(|(e, r)| match r {
            Route::Manual(m) => Some((*e, m.clone())),
            _ => None,
        })
        .collect();
    let downloads = routes
        .iter()
        .filter(|(e, r)| !matches!(r, Route::Manual(_)) && !pack_mod(e, r))
        .map(Ok::<_, AppError>);
    futures_util::stream::iter(downloads)
        .try_for_each_concurrent(PARALLEL_DOWNLOADS, |(entry, route)| {
            let done = &done;
            async move {
                let mut on_bytes = |n: u64| {
                    let now = done.fetch_add(n, Ordering::Relaxed) + n;
                    progress.bytes(now.min(total), total);
                };
                match (route, links) {
                    (Route::Blob, Some(links))
                        if !fetch::content_file(&state.paths, entry).is_file() =>
                    {
                        let url = links
                            .get(&entry.sha256)
                            .await
                            .map_err(|e| taken_down(e, &entry.name))?;
                        let target = fetch::content_file(&state.paths, entry);
                        fetch::download(
                            state.cloud.file_http(),
                            &url,
                            &target,
                            Some(&entry.sha1),
                            Some(&entry.sha256),
                            entry.size_bytes,
                            &mut on_bytes,
                        )
                        .await
                    }
                    (Route::Blob, _) => {
                        on_bytes(entry.size_bytes);
                        Ok(())
                    }
                    _ => {
                        fetch::content(&state.paths, &state.downloads, entry, route, &mut on_bytes)
                            .await
                    }
                }
            }
        })
        .await?;
    if !from_pack.is_empty() {
        let base = base.ok_or_else(|| {
            AppError::conflict("This version's files come from a modpack it doesn't name.")
        })?;
        let mut on_bytes = |n: u64| {
            let now = done.fetch_add(n, Ordering::Relaxed) + n;
            progress.bytes(now.min(total), total);
        };
        pack::extract(state, base, from_pack, &mut on_bytes).await?;
    }
    for a in &plan.actions {
        if let Action::WriteConfig { config, .. } = a {
            if config.in_pack {
                continue;
            }
            match links {
                Some(links) if !fetch::blob_file(&state.paths, &config.sha256).is_file() => {
                    let url = links
                        .get(&config.sha256)
                        .await
                        .map_err(|e| taken_down(e, &config.path))?;
                    fetch::config_from(&state.paths, state.cloud.file_http(), config, &url).await?;
                }
                Some(_) => {}
                None => {
                    fetch::config(&state.paths, source, config).await?;
                }
            }
        }
    }
    if !manual.is_empty() {
        wait_for_manual_downloads(state, progress, &manual).await?;
    }
    progress.bytes(total, total);
    Ok(())
}

/// An uploaded file the server won't hand out any more: it was taken down after a report.
/// The version can't be installed until someone publishes one without it.
fn taken_down(e: AppError, name: &str) -> AppError {
    if e.code == crate::error::ErrorCode::NotFound {
        AppError::conflict(format!(
            "{name} was taken down from Atlas, so this version can't be installed. Ask the \
             owner to publish one without it."
        ))
    } else {
        e
    }
}

/// CurseForge files that can't be downloaded automatically: open their pages and pick the
/// files up from the Downloads folder as they arrive.
async fn wait_for_manual_downloads<R: Runtime>(
    state: &AppState,
    progress: &Progress<R>,
    manual: &[(&Entry, crate::dto::ManualDownload)],
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
            .filter(|(e, _)| !fetch::content_file(&state.paths, e).is_file())
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
        .map_err(AppError::from)?;
        progress.sleep(Duration::from_secs(2)).await?;
    }
}

/// How many steps `apply_actions` counts: a copy per file it places, then one per action.
fn apply_steps(actions: &[Action]) -> u64 {
    let copies = actions
        .iter()
        .filter(|a| matches!(a, Action::Place { .. } | Action::WriteConfig { .. }))
        .count();
    (copies + actions.len()) as u64
}

/// Stage every file, then make the changes, journaling each one. Blocking.
fn apply_actions(
    paths: &crate::state::AppPaths,
    game_dir: &Path,
    actions: &[Action],
    backup_dir: PathBuf,
    done: &AtomicU64,
) -> AppResult<()> {
    let staging = new_staging(game_dir)?;

    // Copy from the cache into staging first: if that fails, nothing has changed yet.
    for a in actions {
        let (path, from) = match a {
            Action::Place { entry, .. } => (&entry.path, fetch::content_file(paths, entry)),
            Action::WriteConfig { config, .. } => {
                (&config.path, fetch::blob_file(paths, &config.sha256))
            }
            _ => continue,
        };
        let to = staging.join("files").join(path);
        std::fs::create_dir_all(to.parent().expect("relative path has a parent"))?;
        let mine = std::fs::read(game_dir.join(path)).ok();
        match (a, mine) {
            (
                Action::WriteConfig {
                    config:
                        Override {
                            keep: Some(Keep::MergeServers),
                            ..
                        },
                    ..
                },
                Some(mine),
            ) => {
                // An unreadable list stays as the member has it.
                let merged = super::servers::merge(&mine, &std::fs::read(&from)?).unwrap_or(mine);
                std::fs::write(&to, merged)?;
            }
            _ => {
                std::fs::copy(&from, &to)?;
            }
        }
        done.fetch_add(1, Ordering::Relaxed);
    }

    let steps = actions.iter().map(|a| match a {
        Action::Remove { path } | Action::RemoveConfig { path } => (path.as_str(), false),
        Action::Place { entry, .. } => (entry.path.as_str(), true),
        Action::WriteConfig { config, .. } => (config.path.as_str(), true),
    });
    commit(game_dir, &staging, steps, backup_dir, done)
}

/// Put back the files a backup recorded, staged and journaled like an update, so a crash
/// in the middle is undone at the next start. `backup_dir` holds the files as they are
/// now (the backup taken before restoring). Blocking.
pub fn restore_files(
    game_dir: &Path,
    from: &Path,
    records: &[FileRecord],
    backup_dir: PathBuf,
) -> AppResult<()> {
    let staging = new_staging(game_dir)?;
    let mut steps = Vec::new();
    for r in records {
        let copy = from.join(&r.path);
        match r.action {
            FileAction::Added => steps.push((r.path.as_str(), false)),
            // Not in the backup: left as it is.
            FileAction::Replaced | FileAction::Removed if !copy.is_file() => {}
            FileAction::Replaced | FileAction::Removed => {
                let to = staging.join("files").join(&r.path);
                std::fs::create_dir_all(to.parent().expect("relative path has a parent"))?;
                std::fs::copy(&copy, &to)?;
                steps.push((r.path.as_str(), true));
            }
        }
    }
    commit(game_dir, &staging, steps, backup_dir, &AtomicU64::new(0))
}

/// A staging folder for one job, inside the game folder (same drive, so moving a file
/// into place is a rename).
fn new_staging(game_dir: &Path) -> AppResult<PathBuf> {
    let staging = game_dir.join(STAGING).join(new_id());
    std::fs::create_dir_all(&staging)?;
    hide(&game_dir.join(STAGING));
    Ok(staging)
}

/// Make the changes: `(path, true)` moves the staged file into place, `(path, false)`
/// removes what's there. Each is journaled before it's made, so a crash in between is
/// still undone (recovery.rs); a failure undoes what was done so far. Blocking.
fn commit<'a>(
    game_dir: &Path,
    staging: &Path,
    steps: impl IntoIterator<Item = (&'a str, bool)>,
    backup_dir: PathBuf,
    done: &AtomicU64,
) -> AppResult<()> {
    let mut journal = Journal::start(staging, backup_dir)?;
    let result = (|| -> AppResult<()> {
        for (path, put) in steps {
            done.fetch_add(1, Ordering::Relaxed);
            let target = game_dir.join(path);
            if !put {
                journal.record(path, FileAction::Removed)?;
                remove(&target)?;
                continue;
            }
            // Anything already there was backed up, tracked or not.
            if target.exists() {
                journal.record(path, FileAction::Removed)?;
                remove(&target)?;
            }
            std::fs::create_dir_all(target.parent().expect("relative path has a parent"))?;
            journal.record(path, FileAction::Added)?;
            std::fs::rename(staging.join("files").join(path), &target)?;
        }
        Ok(())
    })();

    if result.is_err() {
        // Undo what was done so far, then report the failure.
        super::recovery::undo(game_dir, &journal);
    }
    // Closed first: Windows won't remove a folder with a file open in it, and a journal
    // left behind would be undone at the next start.
    drop(journal);
    let _ = std::fs::remove_dir_all(staging);
    let _ = std::fs::remove_dir(game_dir.join(STAGING)); // only if empty
    result
}

/// Delete a file, retrying briefly if something (antivirus, the launcher) has it open.
fn remove(path: &Path) -> AppResult<()> {
    match crate::util::files::retry_in_use(|| std::fs::remove_file(path)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "couldn't remove file");
            Err(AppError::conflict(
                "A file is in use. Close the launcher and try again.",
            ))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_that_fails_partway_is_undone() {
        let dir = std::env::temp_dir().join(format!("atlas-apply-{}", new_id()));
        let paths = crate::state::AppPaths::new(&dir.join("appdata"));
        let (game, backup) = (dir.join("game"), dir.join("backup"));
        std::fs::create_dir_all(game.join("mods/stuck/x")).unwrap();
        std::fs::create_dir_all(backup.join("mods")).unwrap();
        std::fs::write(game.join("mods/a.jar"), "a v1").unwrap();
        std::fs::write(backup.join("mods/a.jar"), "a v1").unwrap();
        let entry: Entry = serde_json::from_value(serde_json::json!({
            "kind": "mod", "source": "modrinth", "name": "A", "path": "mods/a.jar",
            "sha1": "1".repeat(40), "sha256": "2".repeat(64), "sizeBytes": 4, "required": true
        }))
        .unwrap();
        let cached = fetch::content_file(&paths, &entry);
        std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
        std::fs::write(&cached, "a v2").unwrap();

        // a.jar is replaced, then removing "mods/stuck" (a folder) fails.
        let actions = [
            Action::Place {
                entry: Box::new(entry),
                replaces: true,
                local: None,
            },
            Action::Remove {
                path: "mods/stuck".into(),
            },
        ];
        let done = AtomicU64::new(0);
        assert!(apply_actions(&paths, &game, &actions, backup, &done).is_err());
        assert_eq!(std::fs::read(game.join("mods/a.jar")).unwrap(), b"a v1");
        assert!(!game.join(STAGING).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_restore_goes_through_the_journal_and_is_undone_when_it_fails() {
        let dir = std::env::temp_dir().join(format!("atlas-restore-{}", new_id()));
        let (game, old, now) = (dir.join("game"), dir.join("old"), dir.join("now"));
        for d in [&game, &old, &now] {
            std::fs::create_dir_all(d.join("mods")).unwrap();
        }
        std::fs::write(game.join("mods/a.jar"), b"a, as it is now").unwrap();
        std::fs::write(now.join("mods/a.jar"), b"a, as it is now").unwrap();
        std::fs::write(old.join("mods/a.jar"), b"a, as it was").unwrap();
        std::fs::write(game.join("mods/b.jar"), b"added since").unwrap();
        let rec = |path: &str, action| FileRecord {
            path: path.into(),
            action,
        };

        // "mods/stuck" is a folder, so removing it fails after a.jar was put back.
        std::fs::create_dir_all(game.join("mods/stuck/x")).unwrap();
        let failing = [
            rec("mods/a.jar", FileAction::Replaced),
            rec("mods/stuck", FileAction::Added),
        ];
        assert!(restore_files(&game, &old, &failing, now.clone()).is_err());
        assert_eq!(
            std::fs::read(game.join("mods/a.jar")).unwrap(),
            b"a, as it is now",
            "undone"
        );
        assert!(!game.join(STAGING).exists());

        let records = [
            rec("mods/a.jar", FileAction::Replaced),
            rec("mods/b.jar", FileAction::Added),
            rec("mods/gone.jar", FileAction::Removed),
        ];
        restore_files(&game, &old, &records, now).unwrap();
        assert_eq!(
            std::fs::read(game.join("mods/a.jar")).unwrap(),
            b"a, as it was"
        );
        assert!(!game.join("mods/b.jar").exists());
        assert!(
            !game.join("mods/gone.jar").exists(),
            "not in the backup: left alone"
        );
        assert!(!game.join(STAGING).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_config_a_mod_only_rewrote_counts_as_the_synced_one() {
        let dir = std::env::temp_dir().join(format!("atlas-apply-{}", new_id()));
        std::fs::create_dir_all(dir.join("config")).unwrap();
        let synced_bytes = b"[general]
speed = 2
";
        let synced_sha = hex::encode(Sha256::digest(synced_bytes));
        let config = |path: &str, sha: &str| Override {
            path: path.into(),
            sha256: sha.into(),
            size_bytes: 1,
            in_pack: false,
            keep: None,
        };
        let mut synced: Manifest = serde_json::from_value(serde_json::json!({
            "schemaVersion": 2, "name": "m",
            "minecraft": { "version": "1.21.1", "loader": "fabric" },
            "content": [], "overrides": []
        }))
        .unwrap();
        synced.overrides = vec![
            config("config/a.toml", &synced_sha),
            config("config/b.toml", &synced_sha),
        ];
        // a: the game rewrote it with comments; b: the member changed a setting.
        std::fs::write(
            dir.join("config/a.toml"),
            "# Speed
[general]
  speed = 2
",
        )
        .unwrap();
        std::fs::write(
            dir.join("config/b.toml"),
            "[general]
speed = 5
",
        )
        .unwrap();
        let cached = |sha: &str| (sha == synced_sha).then(|| synced_bytes.to_vec());
        let hashes = config_hashes(&dir, Some(&synced), &synced, &cached);
        assert_eq!(hashes["config/a.toml"], synced_sha);
        assert_ne!(hashes["config/b.toml"], synced_sha);
        let not_cached = |_: &str| None;
        let hashes = config_hashes(&dir, Some(&synced), &synced, &not_cached);
        assert_ne!(
            hashes["config/a.toml"], synced_sha,
            "nothing to compare with"
        );
    }
}
