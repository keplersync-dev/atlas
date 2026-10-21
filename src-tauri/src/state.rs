//! Shared app state, available to every command as `State<'_, AppState>`.
//!
//! Later phases add the job manager, game monitor, watcher and cloud client here
//! (docs/tauri-plan.md §4.2). Everything inside is cheap to clone, so a background
//! task can take its own copy.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tokio::sync::OnceCell;

use crate::game::Running;
use crate::scan::watcher::Watcher;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub paths: AppPaths,
    /// Look for launchers in their default folders. Off in tests, so results don't
    /// depend on what's installed on the machine running them.
    pub auto_detect: bool,
    /// Set once launcher detection has run this session (see services::launchers).
    pub detected: Arc<OnceCell<()>>,
    /// Instances whose game is running (kept up to date by crate::game).
    pub running: Running,
    /// Watches tracked folders; rescans come out of `watcher.take_receiver()`.
    pub watcher: Arc<Watcher>,
    /// One scan at a time per instance (the watcher, a rescan and tracking can overlap).
    pub scan_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    /// For calls to Modrinth (and later the Atlas server). Identifies Atlas, as Modrinth asks.
    pub http: reqwest::Client,
    /// Modrinth API base URL. None = don't look files up (tests).
    pub modrinth_api: Option<String>,
    /// Instances with a lookup in flight, so a burst of scans doesn't ask twice.
    pub resolving: Arc<Mutex<HashSet<String>>>,
}

impl AppState {
    pub fn new(db: SqlitePool, paths: AppPaths) -> Self {
        Self {
            db,
            paths,
            auto_detect: true,
            detected: Arc::new(OnceCell::new()),
            running: Running::default(),
            watcher: Arc::new(Watcher::new()),
            scan_locks: Arc::default(),
            http: reqwest::Client::builder()
                .user_agent(concat!("keplerhaloxx/atlas/", env!("CARGO_PKG_VERSION")))
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("http client"),
            modrinth_api: Some(crate::resolve::modrinth::API.to_string()),
            resolving: Arc::default(),
        }
    }

    pub fn is_running(&self, instance_id: &str) -> bool {
        self.running.contains(instance_id)
    }

    /// Hold this while scanning an instance.
    pub async fn scan_lock(&self, instance_id: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = self.scan_locks.lock().unwrap().entry(instance_id.to_string()).or_default().clone();
        lock.lock_owned().await
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
