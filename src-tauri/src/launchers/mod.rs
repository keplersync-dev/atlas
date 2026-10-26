//! Launcher adapters: read the instances other launchers created.
//!
//! One small module per launcher, each tested against sample files in
//! tests/fixtures/<launcher>/. Launchers change their formats now and then; when one
//! breaks, only its module and fixtures change.
//!
//! Adapters read files and open the launcher. The one thing they write is a new, empty
//! instance when someone accepts a shared instance into it (see `creatable_loaders`).

mod curseforge;
mod modrinth;
mod official;
mod prism;
mod util;

use std::path::{Path, PathBuf};

pub use util::file_stem;

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
    /// Something the player can act on, shown to them as it is.
    #[error("{0}")]
    Shown(String),
    /// The launcher was asked to make an instance and didn't (the user said no, or never
    /// answered).
    #[error("the launcher didn't create the instance")]
    NotCreated,
}

pub type ReadResult<T> = Result<T, ReadError>;

impl From<ReadError> for AppError {
    fn from(e: ReadError) -> Self {
        tracing::warn!(error = %e, "couldn't read launcher files");
        match e {
            ReadError::Shown(m) => AppError::unknown(m),
            _ => AppError::unknown("Couldn't read the launcher's files"),
        }
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

/// The loaders Atlas can set up in a new instance there (for accepting invites).
/// Empty: Atlas can't create instances in that launcher.
///
/// - Prism/MultiMC: Atlas writes the instance folder; Prism installs the game and loader.
/// - Modrinth App: the app makes the instance itself from a pack file Atlas hands it
///   (modrinth.rs). Writing its app.db directly isn't safe: the instance rows are tied to
///   install jobs, content sets and its file store.
/// - Minecraft Launcher: a new profile. Fabric and Quilt only need their version file,
///   which Atlas fetches like their installers do. Forge and NeoForge have to run their
///   installer under Java to patch the game, so Atlas leaves those to the user.
/// - CurseForge: Atlas writes the instance folder. Its instance file carries the loader's
///   install data, which only CurseForge's API hands out: the Atlas server asks it (the
///   key stays there) and passes it on, see `create_instance`.
pub fn creatable_loaders(kind: LauncherKind) -> &'static [Loader] {
    const ALL: &[Loader] = &[
        Loader::Vanilla,
        Loader::Fabric,
        Loader::Quilt,
        Loader::Forge,
        Loader::Neoforge,
    ];
    match kind {
        LauncherKind::Prism
        | LauncherKind::Multimc
        | LauncherKind::Modrinth
        | LauncherKind::Curseforge => ALL,
        // Forge and NeoForge through their installers, which need a Java (official.rs).
        LauncherKind::Official => ALL,
        LauncherKind::Folder => &[],
    }
}

pub fn can_create_instance(kind: LauncherKind, loader: Loader) -> bool {
    creatable_loaders(kind).contains(&loader)
}

/// What a new instance for a shared one needs.
#[derive(Debug, Clone)]
pub struct NewInstance {
    pub name: String,
    pub minecraft: String,
    pub loader: Loader,
    /// Required for every loader but vanilla.
    pub loader_version: Option<String>,
    /// The shared instance's picture (png, jpg, gif or webp).
    pub icon: Option<Vec<u8>>,
}

impl NewInstance {
    fn loader_version(&self) -> ReadResult<Option<&str>> {
        match (self.loader, self.loader_version.as_deref()) {
            (Loader::Vanilla, _) => Ok(None),
            (_, Some(v)) if !v.is_empty() => Ok(Some(v)),
            _ => Err(ReadError::Format(
                "the shared instance doesn't say which loader version".into(),
            )),
        }
    }
}

/// Launchers that can't take a picture with a new instance, so Atlas sets it afterwards
/// while they're closed (see `set_missing_icon`).
pub fn sets_icon_later(kind: LauncherKind) -> bool {
    kind == LauncherKind::Modrinth
}

pub fn is_running(kind: LauncherKind) -> bool {
    match kind {
        LauncherKind::Modrinth => modrinth::running(),
        LauncherKind::Official => official::running(),
        LauncherKind::Curseforge => curseforge::running(),
        _ => false,
    }
}

/// Give an instance the launcher has no picture for this one. Only for `sets_icon_later`
/// launchers, only while they're closed. Ok(true) when it did.
pub async fn set_missing_icon(
    kind: LauncherKind,
    root: &Path,
    instance_id: &str,
    icon: &[u8],
) -> ReadResult<bool> {
    match kind {
        LauncherKind::Modrinth => modrinth::set_icon(root, instance_id, icon).await,
        _ => Ok(false),
    }
}

/// Why Atlas can't create an instance there right now, if it can't.
pub fn create_blocker(kind: LauncherKind) -> Option<&'static str> {
    match kind {
        // It keeps its profiles in memory and saves them over the file.
        LauncherKind::Official if official::running() => {
            Some("Close the Minecraft Launcher first, then try again")
        }
        // It may not notice an instance folder added while it's open.
        LauncherKind::Curseforge if curseforge::running() => {
            Some("Close CurseForge first, then try again")
        }
        _ => None,
    }
}

/// Create an empty instance for a shared one. Returns its launcher instance id.
/// `curseforge_loader`: CurseForge's install data for the loader, from the Atlas server
/// (CurseForge instances other than vanilla need it).
pub async fn create_instance(
    kind: LauncherKind,
    root: &Path,
    new: NewInstance,
    http: &reqwest::Client,
    curseforge_loader: Option<serde_json::Value>,
) -> ReadResult<String> {
    if !can_create_instance(kind, new.loader) {
        return Err(ReadError::Format(format!(
            "can't create {:?} instances in {}",
            new.loader,
            display_name(kind)
        )));
    }
    let root = root.to_path_buf();
    match kind {
        LauncherKind::Prism | LauncherKind::Multimc => {
            let flavor = if kind == LauncherKind::Prism {
                prism::Flavor::Prism
            } else {
                prism::Flavor::MultiMc
            };
            unblock(move || prism::create(flavor, &root, &new)).await
        }
        LauncherKind::Official => {
            let version = match new.loader {
                Loader::Forge | Loader::Neoforge => {
                    Some(official::install_forge(http, &root, &new).await?)
                }
                _ => official::fetch_version(http, &new).await?,
            };
            unblock(move || official::create(&root, &new, version)).await
        }
        LauncherKind::Modrinth => modrinth::create(&root, &new).await,
        LauncherKind::Curseforge => {
            unblock(move || curseforge::create(&root, &new, curseforge_loader)).await
        }
        LauncherKind::Folder => unreachable!("no creatable loaders"),
    }
}

async fn unblock<T: Send + 'static>(
    work: impl FnOnce() -> ReadResult<T> + Send + 'static,
) -> ReadResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| ReadError::Format(format!("creating the instance failed: {e}")))?
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
/// The modpack an instance was installed from, as its launcher recorded it.
#[derive(Debug, Clone, PartialEq)]
pub enum InstalledPack {
    /// CurseForge keeps the pack's download details with the instance.
    Known(atlas_protocol::manifest::PackBase),
    /// The Modrinth App and Prism keep only Modrinth's ids.
    Modrinth {
        project_id: String,
        version_id: String,
    },
}

/// The modpack the instance came from, if its launcher installed it from one. None for
/// instances made by hand, by Atlas, or in launchers that don't say.
pub async fn installed_pack(
    kind: LauncherKind,
    root: &Path,
    launcher_instance_id: &str,
    game_dir: &Path,
) -> Option<InstalledPack> {
    match kind {
        LauncherKind::Curseforge => curseforge::installed_pack(game_dir),
        LauncherKind::Modrinth => modrinth::installed_pack(root, launcher_instance_id).await,
        LauncherKind::Prism | LauncherKind::Multimc => prism::installed_pack(game_dir),
        LauncherKind::Official | LauncherKind::Folder => None,
    }
}

/// A loader's key in a Modrinth pack's `dependencies`.
pub fn pack_dependency_key(loader: Loader) -> Option<&'static str> {
    modrinth::dependency_key(loader)
}

/// Have a launcher install a Modrinth pack as a new instance: Prism or the Modrinth App.
pub fn import_pack(kind: LauncherKind, root: &Path, pack: &Path) -> Result<(), ReadError> {
    match kind {
        LauncherKind::Prism => Ok(prism::import(root, pack)?),
        LauncherKind::Modrinth => modrinth::open_pack(pack),
        _ => Err(ReadError::Format(
            "this launcher can't install Modrinth packs".into(),
        )),
    }
}

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
    std::process::Command::new(&exe)
        .args(open_args(kind, launcher_instance_id))
        .spawn()
        .map(|_| ())
        .map_err(OpenError::Spawn)
}

/// What to pass the launcher so it opens on the instance, where it can. Prism's `--show`
/// opens the instance's own window (only when Prism isn't already running: a running
/// Prism just comes to the front). The others can only launch the game for a specific
/// instance (MultiMC's `--launch`, the Modrinth App's `modrinth://launch/…`) or have no
/// way at all, so they open as they are.
fn open_args(kind: LauncherKind, launcher_instance_id: &str) -> Vec<String> {
    match kind {
        LauncherKind::Prism => vec!["--show".into(), launcher_instance_id.into()],
        _ => vec![],
    }
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

    #[test]
    fn opening_never_starts_the_game() {
        assert_eq!(
            open_args(LauncherKind::Prism, "Fabric 1.21.1"),
            ["--show", "Fabric 1.21.1"]
        );
        for kind in [
            LauncherKind::Multimc,
            LauncherKind::Modrinth,
            LauncherKind::Curseforge,
            LauncherKind::Official,
        ] {
            assert!(open_args(kind, "x").is_empty(), "{kind:?}");
        }
    }

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
