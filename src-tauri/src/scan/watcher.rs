//! Watch tracked game folders and ask for a rescan when their content changes.
//!
//! Each game dir is watched recursively, so content folders created later are seen too,
//! and events are filtered down to the content folders: saves/, logs/ and the like
//! change all the time while playing and must not cause rescans. Events are debounced
//! (1.5 s), and each burst becomes one instance id on the channel. Whoever owns the
//! receiving end (the app's setup) runs the rescans.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use tokio::sync::mpsc;

use super::files::is_content_folder;

/// game dir (lowercase) → instance id
type Dirs = Arc<Mutex<HashMap<String, String>>>;

pub struct Watcher {
    debouncer: Mutex<Option<Debouncer<RecommendedWatcher, RecommendedCache>>>,
    dirs: Dirs,
    watched: Mutex<HashMap<String, PathBuf>>, // instance id → path passed to watch()
    receiver: Mutex<Option<mpsc::UnboundedReceiver<String>>>,
}

impl Watcher {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let dirs: Dirs = Arc::default();
        let lookup = dirs.clone();
        let debouncer = new_debouncer(
            Duration::from_millis(1500),
            None,
            move |result: DebounceEventResult| {
                let Ok(events) = result else { return };
                let ids: HashSet<String> = {
                    let dirs = lookup.lock().unwrap();
                    events
                        .iter()
                        .flat_map(|e| e.paths.iter())
                        .filter_map(|p| instance_for(&dirs, p))
                        .collect()
                };
                for id in ids {
                    let _ = tx.send(id); // nobody listening (tests): fine
                }
            },
        );
        if let Err(e) = &debouncer {
            tracing::warn!(error = %e, "folder watching unavailable");
        }
        Self {
            debouncer: Mutex::new(debouncer.ok()),
            dirs,
            watched: Mutex::default(),
            receiver: Mutex::new(Some(rx)),
        }
    }

    /// The stream of instance ids to rescan. Can be taken once.
    pub fn take_receiver(&self) -> Option<mpsc::UnboundedReceiver<String>> {
        self.receiver.lock().unwrap().take()
    }

    pub fn watch(&self, instance_id: &str, game_dir: &Path) {
        self.unwatch(instance_id);
        let mut debouncer = self.debouncer.lock().unwrap();
        let Some(debouncer) = debouncer.as_mut() else {
            return;
        };
        if !game_dir.is_dir() {
            return; // missing: watched again when it comes back (rescan calls watch)
        }
        if let Err(e) = debouncer.watch(game_dir, RecursiveMode::Recursive) {
            tracing::warn!(dir = %game_dir.display(), error = %e, "couldn't watch folder");
            return;
        }
        self.dirs.lock().unwrap().insert(
            game_dir.to_string_lossy().to_lowercase(),
            instance_id.to_string(),
        );
        self.watched
            .lock()
            .unwrap()
            .insert(instance_id.to_string(), game_dir.to_path_buf());
    }

    pub fn unwatch(&self, instance_id: &str) {
        let Some(dir) = self.watched.lock().unwrap().remove(instance_id) else {
            return;
        };
        self.dirs
            .lock()
            .unwrap()
            .remove(&dir.to_string_lossy().to_lowercase());
        if let Some(d) = self.debouncer.lock().unwrap().as_mut() {
            let _ = d.unwatch(&dir);
        }
    }

    pub fn is_watching(&self, instance_id: &str) -> bool {
        self.watched.lock().unwrap().contains_key(instance_id)
    }
}

impl Default for Watcher {
    fn default() -> Self {
        Self::new()
    }
}

/// The instance a changed path belongs to, if it's inside one of its content folders.
fn instance_for(dirs: &HashMap<String, String>, path: &Path) -> Option<String> {
    let lower = path.to_string_lossy().to_lowercase();
    dirs.iter().find_map(|(dir, id)| {
        let rest = lower
            .strip_prefix(dir.as_str())?
            .trim_start_matches(['\\', '/']);
        let top = rest.split(['\\', '/']).next()?;
        is_content_folder(top).then(|| id.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_content_folders_count() {
        let dirs = HashMap::from([(r"c:\games\smp\minecraft".to_string(), "i1".to_string())]);
        let id = |p: &str| instance_for(&dirs, Path::new(p));
        assert_eq!(
            id(r"C:\Games\SMP\minecraft\mods\sodium.jar").as_deref(),
            Some("i1")
        );
        assert_eq!(
            id(r"C:\Games\SMP\minecraft\mods").as_deref(),
            Some("i1"),
            "the folder itself"
        );
        assert_eq!(
            id(r"C:\Games\SMP\minecraft\resourcepacks\x.zip").as_deref(),
            Some("i1")
        );
        assert_eq!(id(r"C:\Games\SMP\minecraft\saves\world\level.dat"), None);
        assert_eq!(id(r"C:\Games\SMP\minecraft\logs\latest.log"), None);
        assert_eq!(id(r"C:\Games\SMP\minecraft\options.txt"), None);
        assert_eq!(id(r"C:\Games\Other\mods\a.jar"), None);
    }

    #[tokio::test]
    async fn a_new_mod_is_reported_once() {
        let dir = std::env::temp_dir().join(format!("atlas-watch-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(dir.join("saves")).unwrap();
        let w = Watcher::new();
        let mut rx = w.take_receiver().unwrap();
        w.watch("i1", &dunce::canonicalize(&dir).unwrap());

        std::fs::write(dir.join("saves/level.dat"), b"x").unwrap(); // ignored
        std::fs::create_dir_all(dir.join("mods")).unwrap(); // created after watching
        std::fs::write(dir.join("mods/a.jar"), b"a").unwrap();
        std::fs::write(dir.join("mods/b.jar"), b"b").unwrap();

        let got = tokio::time::timeout(Duration::from_secs(10), rx.recv()).await;
        assert_eq!(got.ok().flatten().as_deref(), Some("i1"));
        // The burst was one message.
        let more = tokio::time::timeout(Duration::from_millis(2500), rx.recv()).await;
        assert!(
            more.is_err(),
            "expected one message per burst, got {more:?}"
        );

        w.unwatch("i1");
        assert!(!w.is_watching("i1"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
