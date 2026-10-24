//! Shared app state, available to every command as `State<'_, AppState>`.
//!
//! Everything inside is cheap to clone, so a background task can take its own copy.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tokio::sync::OnceCell;

use crate::cloud::Cloud;
use crate::cloud::tokens::CredentialManager;
use crate::dto::OpenedLink;
use crate::game::Running;
use crate::jobs::Jobs;
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
    /// Update/restore jobs in progress (crate::jobs).
    pub jobs: Jobs,
    /// Watches tracked folders; rescans come out of `watcher.take_receiver()`.
    pub watcher: Arc<Watcher>,
    /// One scan at a time per instance (the watcher, a rescan and tracking can overlap).
    pub scan_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    /// For calls to Modrinth (and later the Atlas server). Identifies Atlas, as Modrinth asks.
    pub http: reqwest::Client,
    /// For downloading mods and packs: no overall time limit, since big files on slow
    /// connections take a while (sync/fetch.rs gives up when a download stalls instead).
    pub downloads: reqwest::Client,
    /// Modrinth API base URL. None = don't look files up (tests).
    pub modrinth_api: Option<String>,
    /// Instances with a lookup in flight, so a burst of scans doesn't ask twice.
    pub resolving: Arc<Mutex<HashSet<String>>>,
    /// New instances whose download shouldn't wait for the launcher to close to get their
    /// picture: the player chose to skip it (see accept.rs).
    pub skip_picture: Arc<Mutex<HashSet<String>>>,
    /// The tray and notification settings, kept here for the close button and incoming
    /// activity, which can't wait on the database (see desktop.rs).
    pub prefs: Arc<crate::desktop::Prefs>,
    /// The Atlas server and the signed-in session.
    pub cloud: Arc<Cloud>,
    /// A friend or group link waiting for the UI (crate::links).
    pub opened_link: Arc<Mutex<Option<OpenedLink>>>,
}

impl AppState {
    pub fn new(db: SqlitePool, paths: AppPaths) -> Self {
        Self {
            paths,
            auto_detect: true,
            detected: Arc::new(OnceCell::new()),
            running: Running::default(),
            jobs: Jobs::default(),
            watcher: Arc::new(Watcher::new()),
            scan_locks: Arc::default(),
            http: reqwest::Client::builder()
                .user_agent(concat!("keplerhaloxx/atlas/", env!("CARGO_PKG_VERSION")))
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("http client"),
            downloads: reqwest::Client::builder()
                .user_agent(concat!("keplerhaloxx/atlas/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("http client"),
            modrinth_api: Some(crate::resolve::modrinth::API.to_string()),
            resolving: Arc::default(),
            skip_picture: Arc::default(),
            prefs: Arc::default(),
            cloud: Arc::new(Cloud::new(
                db.clone(),
                Cloud::server_url_from_env(),
                Arc::new(CredentialManager::new(dev_profile().as_deref())),
            )),
            opened_link: Arc::default(),
            db,
        }
    }

    pub fn is_running(&self, instance_id: &str) -> bool {
        self.running.contains(instance_id)
    }

    /// Hold this while scanning an instance.
    pub async fn scan_lock(&self, instance_id: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = self
            .scan_locks
            .lock()
            .unwrap()
            .entry(instance_id.to_string())
            .or_default()
            .clone();
        lock.lock_owned().await
    }
}

/// Development builds only: `ATLAS_PROFILE=2` gives this copy of Atlas its own data
/// folder and saved sign-in, so two copies can run side by side as different accounts.
/// 1–32 letters, digits, `-` or `_`; anything else is ignored.
pub fn dev_profile() -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let profile = std::env::var("ATLAS_PROFILE").ok()?;
    let valid = (1..=32).contains(&profile.len())
        && profile
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !valid {
        tracing::warn!(%profile, "ignoring ATLAS_PROFILE: use letters, digits, - or _");
    }
    valid.then_some(profile)
}

/// Atlas's own folders under `%APPDATA%\dev.keplerhaloxx.atlas` (or, with a development
/// profile, `…\profiles\<profile>`).
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    /// Downloaded files, reused across instances: cache/<sha1[0..2]>/<sha1>
    pub cache: PathBuf,
    /// Mod and pack icons taken out of their files: cache/icons/<sha1>.png (scan/icons.rs)
    pub icons: PathBuf,
    /// backups/<instance id>/<backup id>/
    pub backups: PathBuf,
    /// Shared-instance manifests, never change once published: manifests/<shared id>/<revision>.json
    pub manifests: PathBuf,
    pub logs: PathBuf,
    /// The user's Downloads folder, watched for manual CurseForge downloads.
    pub downloads: PathBuf,
}

impl AppPaths {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            cache: root.join("cache"),
            icons: root.join("cache").join("icons"),
            backups: root.join("backups"),
            manifests: root.join("manifests"),
            logs: root.join("logs"),
            downloads: std::env::var_os("USERPROFILE")
                .map(|h| PathBuf::from(h).join("Downloads"))
                .unwrap_or_else(|| root.join("downloads")),
        }
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("atlas.db")
    }

    pub fn create_all(&self) -> std::io::Result<()> {
        for dir in [
            &self.root,
            &self.cache,
            &self.backups,
            &self.manifests,
            &self.logs,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
