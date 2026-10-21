//! Crash safety: if Atlas stopped in the middle of applying an update,
//! the folder is put back exactly as it was, using the journal and the backup.
//!
//! Runs at startup, before anything else touches instance folders.

use std::path::Path;

use super::apply::{Journal, STAGING};
use super::backup::FileAction;
use crate::db;
use crate::state::AppState;

/// Undo the changes a journal records, newest first: added files are deleted, removed
/// files are copied back from the backup. Best effort: logs what it can't undo.
pub fn undo(game_dir: &Path, journal: &Journal) {
    for step in journal.done.iter().rev() {
        let target = game_dir.join(&step.path);
        let result = match step.action {
            FileAction::Added => std::fs::remove_file(&target).or_else(ignore_missing),
            FileAction::Removed | FileAction::Replaced => {
                let copy = journal.backup_dir.join(&step.path);
                if copy.is_file() {
                    std::fs::create_dir_all(target.parent().expect("relative path has a parent"))
                        .and_then(|_| std::fs::copy(&copy, &target).map(|_| ()))
                } else {
                    Ok(())
                }
            }
        };
        if let Err(e) = result {
            tracing::error!(path = %step.path, error = %e, "couldn't undo a change");
        }
    }
}

fn ignore_missing(e: std::io::Error) -> std::io::Result<()> {
    if e.kind() == std::io::ErrorKind::NotFound {
        Ok(())
    } else {
        Err(e)
    }
}

/// Recover every tracked instance that has leftover staging folders. Returns the ids of
/// instances that were put back.
pub async fn run(state: &AppState) -> Vec<String> {
    let Ok(instances) = db::instances::list(&state.db).await else {
        return vec![];
    };
    let mut recovered = Vec::new();
    for i in instances {
        let game_dir = Path::new(&i.game_dir);
        let staging_root = game_dir.join(STAGING);
        let Ok(jobs) = std::fs::read_dir(&staging_root) else {
            continue;
        };
        for job in jobs.flatten() {
            let journal = std::fs::read(job.path().join("journal.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<Journal>(&b).ok());
            if let Some(j) = journal {
                tracing::warn!(instance = %i.name, steps = j.done.len(), "undoing an update that was interrupted");
                undo(game_dir, &j);
                recovered.push(i.id.clone());
            }
            // No journal: nothing had been moved yet. Either way the staging folder goes.
            let _ = std::fs::remove_dir_all(job.path());
        }
        let _ = std::fs::remove_dir(&staging_root);
    }
    recovered
}
