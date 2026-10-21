//! Shared app state, available to every command as `State<'_, AppState>`.
//!
//! Later phases add the job manager, game monitor, watcher and cloud client here
//! (docs/tauri-plan.md §4.2). Everything inside is cheap to clone, so a background
//! task can take its own copy.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub paths: AppPaths,
    /// Look for launchers in their default folders. Off in tests, so results don't
    /// depend on what's installed on the machine running them.
    pub auto_detect: bool,
    /// Set once launcher detection has run this session (see services::launchers).
    pub detected: Arc<OnceCell<()>>,
}

impl AppState {
    pub fn new(db: SqlitePool, paths: AppPaths) -> Self {
        Self { db, paths, auto_detect: true, detected: Arc::new(OnceCell::new()) }
    }
}

/// Atlas's own folders under `%APPDATA%\dev.keplerhaloxx.atlas` (data-plan §2).
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    /// Downloaded files, reused across instances: cache/<sha1[0..2]>/<sha1>
    pub cache: PathBuf,
    /// backups/<instance id>/<backup id>/
    pub backups: PathBuf,
    /// Shared-instance manifests, never change once published: manifests/<shared id>/<revision>.json
    pub manifests: PathBuf,
    pub logs: PathBuf,
}

impl AppPaths {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            cache: root.join("cache"),
            backups: root.join("backups"),
            manifests: root.join("manifests"),
            logs: root.join("logs"),
        }
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("atlas.db")
    }

    pub fn create_all(&self) -> std::io::Result<()> {
        for dir in [&self.root, &self.cache, &self.backups, &self.manifests, &self.logs] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
