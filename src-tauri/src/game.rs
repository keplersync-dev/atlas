//! Is the game running from a tracked folder? (docs/tauri-plan.md §9.4)
//!
//! Every 3 seconds, look at Java processes. One belongs to an instance when its
//! `--gameDir` argument or its working directory is that instance's game dir.
//! Commands refuse to write files while it runs; sync jobs (T4) wait instead.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, Runtime};

use crate::db;
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;

const POLL: Duration = Duration::from_secs(3);

/// Instance ids whose game is running right now. Cheap to clone and read.
#[derive(Clone, Default)]
pub struct Running(Arc<RwLock<HashSet<String>>>);

impl Running {
    pub fn contains(&self, instance_id: &str) -> bool {
        self.0.read().unwrap().contains(instance_id)
    }
    fn replace(&self, now: HashSet<String>) -> HashSet<String> {
        std::mem::replace(&mut *self.0.write().unwrap(), now)
    }
    pub fn remove(&self, instance_id: &str) {
        self.0.write().unwrap().remove(instance_id);
    }
    /// For tests: pretend a game is running.
    pub fn insert(&self, instance_id: &str) {
        self.0.write().unwrap().insert(instance_id.to_string());
    }
}

/// Start polling. Emits `instance.running` for every change.
pub fn spawn<R: Runtime>(state: AppState, app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut system = System::new();
        loop {
            let dirs = tokio::task::spawn_blocking(move || {
                let dirs = java_game_dirs(&mut system);
                (system, dirs)
            })
            .await;
            let Ok((sys, dirs)) = dirs else { return };
            system = sys;
            update(&state, &app, &dirs).await;
            tokio::time::sleep(POLL).await;
        }
    });
}

async fn update<R: Runtime>(state: &AppState, app: &AppHandle<R>, running_dirs: &HashSet<String>) {
    let Ok(instances) = db::instances::list(&state.db).await else { return };
    let now: HashSet<String> = instances
        .into_iter()
        .filter(|i| running_dirs.contains(&key(Path::new(&i.game_dir))))
        .map(|i| i.id)
        .collect();
    let before = state.running.replace(now.clone());
    for id in now.symmetric_difference(&before) {
        let is_running = now.contains(id);
        tracing::info!(instance = %id, is_running, "game running changed");
        emit(app, AtlasEvent::InstanceRunning { instance_id: id.clone(), is_running });
        // T6: send presence ("playing" + instance name unless hidden).
    }
}

/// Game dirs of running Java processes, as comparison keys. Blocking.
fn java_game_dirs(system: &mut System) -> HashSet<String> {
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_cwd(UpdateKind::OnlyIfNotSet),
    );
    let mut dirs = HashSet::new();
    for process in system.processes().values() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name != "java.exe" && name != "javaw.exe" {
            continue;
        }
        let args: Vec<String> = process.cmd().iter().map(|a| a.to_string_lossy().into_owned()).collect();
        if let Some(dir) = game_dir_arg(&args) {
            dirs.insert(key(Path::new(&dir)));
        }
        if let Some(cwd) = process.cwd() {
            dirs.insert(key(cwd));
        }
    }
    dirs
}

/// `--gameDir <path>` or `--gameDir=<path>` from Minecraft's command line.
fn game_dir_arg(args: &[String]) -> Option<String> {
    args.iter().enumerate().find_map(|(i, a)| {
        if a == "--gameDir" {
            args.get(i + 1).cloned()
        } else {
            a.strip_prefix("--gameDir=").map(str::to_string)
        }
    })
}

/// Paths compared the way Windows does: canonical when possible, ignoring case.
fn key(path: &Path) -> String {
    let p = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_game_dir_in_either_form() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            game_dir_arg(&args(&["javaw", "-Xmx4G", "net.minecraft.client.main.Main", "--gameDir", r"C:\pack", "--version", "1.21.1"])),
            Some(r"C:\pack".into())
        );
        assert_eq!(game_dir_arg(&args(&["java", r"--gameDir=C:\pack 2"])), Some(r"C:\pack 2".into()));
        assert_eq!(game_dir_arg(&args(&["java", "-jar", "server.jar"])), None);
    }

    #[test]
    fn keys_ignore_case_and_trailing_slashes() {
        assert_eq!(key(Path::new(r"C:\Does\Not\Exist\")), key(Path::new(r"c:\does\not\exist")));
    }
}
