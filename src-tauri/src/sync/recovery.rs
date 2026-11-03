//! Crash safety: if Atlas stopped in the middle of applying an update or restoring a
//! backup, the folder is put back exactly as it was, using the journal and the backup.
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
            if let Some(j) = Journal::read(&job.path()) {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn folder() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atlas-recover-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(dir.join("game/mods")).unwrap();
        std::fs::create_dir_all(dir.join("backup/mods")).unwrap();
        dir
    }

    fn files(dir: &Path) -> Vec<(String, String)> {
        let mut out: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|f| f.path().is_file())
            .map(|f| {
                let text = std::fs::read_to_string(f.path()).unwrap();
                (f.file_name().to_string_lossy().into_owned(), text)
            })
            .collect();
        out.sort();
        out
    }

    /// The journal as `commit` writes it, then `tail` as a crash might leave it.
    fn journal(staging: &Path, backup: &Path, steps: &[(&str, &str)], tail: &str) {
        std::fs::create_dir_all(staging).unwrap();
        let mut text = format!("{}\n", serde_json::to_string(backup).unwrap());
        for (path, action) in steps {
            text += &format!("{{\"path\":\"{path}\",\"action\":\"{action}\"}}\n");
        }
        std::fs::write(staging.join("journal.jsonl"), text + tail).unwrap();
    }

    #[test]
    fn a_crash_mid_apply_puts_the_folder_back_exactly() {
        let dir = folder();
        let (game, backup) = (dir.join("game"), dir.join("backup"));
        let before = [("a.jar", "a v1"), ("c.jar", "c"), ("d.jar", "mine")];
        for (name, text) in before {
            std::fs::write(game.join("mods").join(name), text).unwrap();
        }
        std::fs::write(backup.join("mods/a.jar"), "a v1").unwrap();
        std::fs::write(backup.join("mods/c.jar"), "c").unwrap();
        // The update replaced a.jar, removed c.jar and added b.jar, then stopped while
        // writing down its next step (d.jar), before making it.
        std::fs::write(game.join("mods/a.jar"), "a v2").unwrap();
        std::fs::remove_file(game.join("mods/c.jar")).unwrap();
        std::fs::write(game.join("mods/b.jar"), "b").unwrap();
        let staging = game.join(STAGING).join("job");
        journal(
            &staging,
            &backup,
            &[
                ("mods/a.jar", "removed"),
                ("mods/a.jar", "added"),
                ("mods/c.jar", "removed"),
                ("mods/b.jar", "added"),
            ],
            r#"{"path":"mods/d.ja"#,
        );

        let j = Journal::read(&staging).unwrap();
        assert_eq!(j.done.len(), 4, "the cut-off line is left out");
        undo(&game, &j);
        let expected: Vec<(String, String)> = before
            .iter()
            .map(|(n, t)| (n.to_string(), t.to_string()))
            .collect();
        assert_eq!(files(&game.join("mods")), expected);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_backup_copy_leaves_the_rest_undone() {
        let dir = folder();
        let (game, backup) = (dir.join("game"), dir.join("backup"));
        std::fs::write(game.join("mods/new.jar"), "new").unwrap();
        let staging = game.join(STAGING).join("job");
        // x.jar was removed, but its copy is gone from the backup.
        journal(
            &staging,
            &backup,
            &[("mods/x.jar", "removed"), ("mods/new.jar", "added")],
            "",
        );
        undo(&game, &Journal::read(&staging).unwrap());
        assert!(files(&game.join("mods")).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn half_written_and_old_journals() {
        let dir = folder();
        let staging = dir.join("job");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("journal.jsonl"), r#""C:\Users\a\bac"#).unwrap();
        assert!(Journal::read(&staging).is_none(), "nothing was done yet");

        // Left by a version that wrote the whole journal at once.
        let old = dir.join("old");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(
            old.join("journal.json"),
            r#"{"backup_dir":"b","done":[{"path":"mods/a.jar","action":"added"}]}"#,
        )
        .unwrap();
        let j = Journal::read(&old).unwrap();
        assert_eq!(j.backup_dir, PathBuf::from("b"));
        assert_eq!(j.done.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
