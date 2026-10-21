//! Launcher adapters: read the instances other launchers created (docs/tauri-plan.md §8).
//!
//! One small module per launcher, each tested against sample files in
//! tests/fixtures/<launcher>/. Launchers change their formats now and then; when one
//! breaks, only its module and fixtures change.
//!
//! Adapters only read files (and open the launcher). They never write anything in T1.

mod curseforge;
mod modrinth;
mod official;
mod prism;
mod util;

use std::path::{Path, PathBuf};

use crate::dto::{LauncherKind, Loader};
use crate::error::AppError;

/// An instance as a launcher describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundInstance {
    /// The launcher's own key: Prism folder name, Modrinth profile path,
    /// CurseForge folder name, official launcher game dir.
    pub launcher_instance_id: String,
    pub name: String,
    /// Absolute; the folder holding mods/, config/, saves/.
    pub game_dir: PathBuf,
    /// An image Atlas can show: "file:<absolute path>", an https URL, or a
    /// "data:image/...;base64," URI (official launcher). None = no picture (Atlas draws
    /// a generated block). services::instances copies local ones into Atlas's cache.
    pub icon: Option<String>,
    pub minecraft_version: Option<String>,
    pub loader: Option<Loader>,
    pub loader_version: Option<String>,
    pub last_played_at: Option<String>,
    pub total_playtime_seconds: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Format(String),
}

pub type ReadResult<T> = Result<T, ReadError>;

impl From<ReadError> for AppError {
    fn from(e: ReadError) -> Self {
        tracing::warn!(error = %e, "couldn't read launcher files");
        AppError::unknown("Couldn't read the launcher's files")
    }
}

/// Launchers that live in an install folder (everything except hand-added folders).
pub const KINDS: [LauncherKind; 5] = [
    LauncherKind::Prism,
    LauncherKind::Multimc,
    LauncherKind::Modrinth,
    LauncherKind::Curseforge,
    LauncherKind::Official,
];

/// "Prism Launcher", "Modrinth App", ...
pub fn display_name(kind: LauncherKind) -> &'static str {
    match kind {
        LauncherKind::Prism => "Prism Launcher",
        LauncherKind::Multimc => "MultiMC",
        LauncherKind::Modrinth => "Modrinth App",
        LauncherKind::Curseforge => "CurseForge",
        LauncherKind::Official => "Minecraft Launcher",
        LauncherKind::Folder => "Folder",
    }
}

/// Atlas can create instances there (for accepting invites, T7).
/// Modrinth App stays off until its import flow is verified (tauri-plan §12.5).
pub fn can_create_instances(kind: LauncherKind) -> bool {
    matches!(kind, LauncherKind::Prism | LauncherKind::Multimc)
}

/// Default install folders of this launcher that exist on this PC.
pub fn detect(kind: LauncherKind) -> Vec<PathBuf> {
    let candidates = match kind {
        LauncherKind::Prism => prism::default_roots(prism::Flavor::Prism),
        LauncherKind::Multimc => prism::default_roots(prism::Flavor::MultiMc),
        LauncherKind::Modrinth => modrinth::default_roots(),
        LauncherKind::Curseforge => curseforge::default_roots(),
        LauncherKind::Official => official::default_roots(),
        LauncherKind::Folder => vec![],
    };
    candidates
        .into_iter()
        .filter(|p| is_root(kind, p))
        .collect()
}

/// Is this folder an install of that launcher? (For "Add launcher" with a custom path.)
pub fn is_root(kind: LauncherKind, path: &Path) -> bool {
    match kind {
        LauncherKind::Prism => prism::is_root(prism::Flavor::Prism, path),
        LauncherKind::Multimc => prism::is_root(prism::Flavor::MultiMc, path),
        LauncherKind::Modrinth => modrinth::is_root(path),
        LauncherKind::Curseforge => curseforge::is_root(path),
        LauncherKind::Official => official::is_root(path),
        LauncherKind::Folder => false,
    }
}

/// Every instance in one launcher install. File reading runs on a blocking thread.
pub async fn list_instances(kind: LauncherKind, root: &Path) -> ReadResult<Vec<FoundInstance>> {
    match kind {
        LauncherKind::Prism => blocking(root, |r| prism::list(prism::Flavor::Prism, r)).await,
        LauncherKind::Multimc => blocking(root, |r| prism::list(prism::Flavor::MultiMc, r)).await,
        LauncherKind::Curseforge => blocking(root, curseforge::list).await,
        LauncherKind::Official => blocking(root, official::list).await,
        // Reads the app's SQLite database, which is async with sqlx.
        LauncherKind::Modrinth => modrinth::list(root).await,
        LauncherKind::Folder => Ok(vec![]),
    }
}

/// Run a file-reading function on a blocking thread, so it doesn't stall async work.
async fn blocking(
    root: &Path,
    read: fn(&Path) -> ReadResult<Vec<FoundInstance>>,
) -> ReadResult<Vec<FoundInstance>> {
    let root = root.to_path_buf();
    tokio::task::spawn_blocking(move || read(&root))
        .await
        .map_err(|e| ReadError::Format(format!("reader thread failed: {e}")))?
}

/// Open the launcher, launching the instance directly where the launcher allows it.
/// Falls back to opening the install folder when the program can't be found.
pub fn open(kind: LauncherKind, root: &Path, launcher_instance_id: &str) -> Result<(), OpenError> {
    let program = match kind {
        LauncherKind::Prism => prism::program(prism::Flavor::Prism, root),
        LauncherKind::Multimc => prism::program(prism::Flavor::MultiMc, root),
        LauncherKind::Modrinth => modrinth::program(),
        LauncherKind::Curseforge => curseforge::program(),
        LauncherKind::Official => official::program(),
        LauncherKind::Folder => None,
    };
    let Some(exe) = program else {
        return Err(OpenError::NoProgram);
    };
    let mut command = std::process::Command::new(&exe);
    if matches!(kind, LauncherKind::Prism | LauncherKind::Multimc) {
        command.arg("--launch").arg(launcher_instance_id);
    }
    command.spawn().map(|_| ()).map_err(OpenError::Spawn)
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("launcher program not found")]
    NoProgram,
    #[error("couldn't start the launcher: {0}")]
    Spawn(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lists what the adapters find in this PC's real launchers. Opt-in, prints only:
    /// `cargo test --lib real_launchers -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn real_launchers() {
        for kind in KINDS {
            for root in detect(kind) {
                let found = list_instances(kind, &root).await;
                println!("{} at {}:", display_name(kind), root.display());
                match found {
                    Ok(list) => {
                        for i in list {
                            println!(
                                "  {:40} {:>8} {:?} {:<10} played {:?}",
                                i.name,
                                i.minecraft_version.unwrap_or_default(),
                                i.loader,
                                i.loader_version.unwrap_or_default(),
                                i.last_played_at
                            );
                        }
                    }
                    Err(e) => println!("  error: {e}"),
                }
            }
        }
    }
}
