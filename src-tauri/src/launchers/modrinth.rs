//! Modrinth App.
//!
//! <root>/app.db (SQLite, the app's own; opened read-only. The one exception is
//! `set_icon`: a picture for an instance Atlas had it make, written only while it's closed.)
//! <root>/profiles/<path>/ is the game dir.
//!
//! Current schema (checked against Modrinth App, migration 20260918120000):
//!   instances(id, path, name, icon_path, last_played [unix s],
//!             submitted_time_played, recent_time_played [s], applied_content_set_id, ...)
//!   instance_content_sets(id, game_version, loader, loader_version, ...)
//! Older versions had one `profiles` table instead (unverified, kept as a fallback).
//!
//! New instances: the app makes them itself from a .mrpack Atlas hands it (see `create`).

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection, SqliteRow};
use sqlx::{Connection, FromRow};

use super::util::{
    any_running, env_dir, file_stem, first_existing, from_unix_seconds, image_ext, loader_from,
};
use super::{FoundInstance, NewInstance, ReadError, ReadResult};
use crate::dto::Loader;

pub fn default_roots() -> Vec<PathBuf> {
    let Some(appdata) = env_dir("APPDATA") else {
        return vec![];
    };
    vec![
        appdata.join("ModrinthApp"),
        appdata.join("com.modrinth.theseus"),
    ]
}

pub fn is_root(path: &Path) -> bool {
    path.join("app.db").is_file() && path.join("profiles").is_dir()
}

pub fn program() -> Option<PathBuf> {
    let local = env_dir("LOCALAPPDATA")?;
    first_existing([
        local.join("Modrinth App").join("Modrinth App.exe"),
        local
            .join("Programs")
            .join("Modrinth App")
            .join("Modrinth App.exe"),
    ])
}

#[derive(FromRow)]
struct Row {
    path: String,
    name: String,
    icon_path: Option<String>,
    last_played: Option<i64>,
    played: Option<i64>,
    game_version: Option<String>,
    loader: Option<String>,
    loader_version: Option<String>,
}

const CURRENT: &str = "
    SELECT i.path, i.name, i.icon_path, i.last_played,
           i.submitted_time_played + i.recent_time_played AS played,
           s.game_version, s.loader, s.loader_version
    FROM instances i
    LEFT JOIN instance_content_sets s ON s.id = i.applied_content_set_id";

const LEGACY: &str = "
    SELECT path, name, icon_path, last_played,
           submitted_time_played + recent_time_played AS played,
           game_version, mod_loader AS loader, mod_loader_version AS loader_version
    FROM profiles";

pub async fn list(root: &Path) -> ReadResult<Vec<FoundInstance>> {
    let rows: Vec<Row> = rows(root, CURRENT, LEGACY).await?;
    Ok(rows
        .into_iter()
        .map(|r| FoundInstance {
            game_dir: root.join("profiles").join(&r.path),
            launcher_instance_id: r.path,
            name: r.name,
            icon: r.icon_path.map(|p| format!("file:{p}")),
            minecraft_version: r.game_version,
            loader: r.loader.as_deref().and_then(loader_from),
            loader_version: r.loader_version,
            last_played_at: r.last_played.and_then(from_unix_seconds),
            total_playtime_seconds: r.played.and_then(|s| u64::try_from(s).ok()),
        })
        .collect())
}

/// Rows from the app's database: `current` on today's schema, `legacy` on the old one.
async fn rows<T>(root: &Path, current: &'static str, legacy: &'static str) -> ReadResult<Vec<T>>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
{
    let db = root.join("app.db");
    if !db.is_file() {
        return Ok(vec![]);
    }
    match read(&db, true, current, legacy).await {
        Ok(rows) => Ok(rows),
        // Locked or mid-write: read a copy instead of waiting on the app.
        Err(e) => {
            tracing::debug!(error = %e, "reading a copy of Modrinth's app.db");
            read_copy(&db, current, legacy).await
        }
    }
}

async fn read<T>(
    db: &Path,
    read_only: bool,
    current: &'static str,
    legacy: &'static str,
) -> ReadResult<Vec<T>>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
{
    let options = SqliteConnectOptions::new()
        .filename(db)
        .read_only(read_only)
        .busy_timeout(Duration::from_secs(2));
    let mut conn = SqliteConnection::connect_with(&options).await?;
    let has_instances: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'instances'",
    )
    .fetch_optional(&mut conn)
    .await?;
    let sql = if has_instances.is_some() {
        current
    } else {
        legacy
    };
    let rows = sqlx::query_as::<_, T>(sql).fetch_all(&mut conn).await?;
    conn.close().await?;
    Ok(rows)
}

async fn read_copy<T>(db: &Path, current: &'static str, legacy: &'static str) -> ReadResult<Vec<T>>
where
    T: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
{
    let dir = std::env::temp_dir().join(format!("atlas-modrinth-{}", crate::util::ids::new_id()));
    std::fs::create_dir_all(&dir)?;
    let result = async {
        for suffix in ["", "-wal", "-shm"] {
            let from = PathBuf::from(format!("{}{suffix}", db.display()));
            if from.is_file() {
                std::fs::copy(&from, dir.join(format!("app.db{suffix}")))?;
            }
        }
        read(&dir.join("app.db"), false, current, legacy).await
    }
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    result.map_err(|e| ReadError::Format(format!("Modrinth app.db: {e}")))
}

/// How long the user has to confirm the new instance in the app.
const CONFIRM_WAIT: Duration = Duration::from_secs(5 * 60);

/// Create an instance through the app, the way it supports from outside: hand it a pack
/// file (.mrpack) that names only the Minecraft and loader versions, like opening one from
/// Explorer. The app asks to confirm (packs it doesn't know get a warning first), makes the
/// instance and installs the loader. Returns the new instance's path once the app is done
/// setting up its folder.
///
/// Writing app.db instead isn't safe: an instance there is tied to install jobs, content
/// sets and the app's file store. The new instance has no picture: the app only takes
/// one from Modrinth or its own editor.
pub async fn create(root: &Path, new: &NewInstance) -> ReadResult<String> {
    let loader_version = new.loader_version()?;
    let before: HashSet<String> = rows::<Stage>(root, STAGES, LEGACY_STAGES)
        .await?
        .into_iter()
        .map(|s| s.path)
        .collect();
    let dir = std::env::temp_dir().join(format!("atlas-mrpack-{}", crate::util::ids::new_id()));
    std::fs::create_dir_all(&dir)?;
    let result = async {
        // The app names the instance after the file at first, then after the pack.
        let pack = dir.join(format!("{}.mrpack", file_stem(&new.name)));
        write_pack(&pack, new, loader_version)?;
        open_pack(&pack)?;
        let deadline = Instant::now() + CONFIRM_WAIT;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            match find_new(root, &before, &new.name).await {
                Ok(Some(path)) => return Ok(path),
                Ok(None) => {}
                Err(e) => {
                    tracing::debug!(error = %e, "couldn't look for the new Modrinth instance")
                }
            }
            if Instant::now() > deadline {
                return Err(ReadError::NotCreated);
            }
        }
    }
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// A loader's key in a pack's `dependencies` ("fabric-loader"). None for vanilla.
pub fn dependency_key(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("fabric-loader"),
        Loader::Quilt => Some("quilt-loader"),
        Loader::Forge => Some("forge"),
        Loader::Neoforge => Some("neoforge"),
    }
}

/// A Modrinth pack with no files: modrinth.index.json naming the versions.
fn write_pack(file: &Path, new: &NewInstance, loader_version: Option<&str>) -> ReadResult<()> {
    let mut dependencies = serde_json::json!({ "minecraft": new.minecraft });
    if let (Some(key), Some(version)) = (dependency_key(new.loader), loader_version) {
        dependencies[key] = version.into();
    }
    let index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "atlas",
        "name": new.name,
        "summary": "Shared with Atlas",
        "files": [],
        "dependencies": dependencies,
    });
    let zip_error = |e: zip::result::ZipError| ReadError::Format(format!("writing the pack: {e}"));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(file)?);
    zip.start_file(
        "modrinth.index.json",
        zip::write::SimpleFileOptions::default(),
    )
    .map_err(zip_error)?;
    zip.write_all(&serde_json::to_vec_pretty(&index)?)?;
    zip.finish().map_err(zip_error)?;
    Ok(())
}

/// Give the pack to the app: its program takes it as an argument (and passes it on to a
/// running copy). Without the program, Explorer opens it with whatever owns .mrpack.
pub fn open_pack(pack: &Path) -> ReadResult<()> {
    let program = program().unwrap_or_else(|| PathBuf::from("explorer.exe"));
    std::process::Command::new(program).arg(pack).spawn()?;
    Ok(())
}

pub fn running() -> bool {
    any_running(&["modrinth app.exe"])
}

/// Ask the app to close, the way its own close button does (not a forced kill). True once
/// it has; false if it's still open after 15 s (a dialog of its own may be in the way).
pub async fn close() -> bool {
    let ask = tokio::task::spawn_blocking(|| {
        let mut taskkill = std::process::Command::new("taskkill");
        taskkill.args(["/IM", "Modrinth App.exe"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            taskkill.creation_flags(CREATE_NO_WINDOW);
        }
        taskkill.output()
    });
    if let Ok(Err(e)) = ask.await {
        tracing::warn!(error = %e, "couldn't ask the Modrinth App to close");
    }
    for _ in 0..30 {
        if !running() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

/// Give an instance a picture when it has none. Packs can't carry one, so instances Atlas
/// has the app make start without. Only while the app is closed: it keeps instances in
/// memory and would write over the change. Copies the picture into the app's icon cache
/// (named by its sha1, as the app names them) and sets `icon_path`, nothing else.
/// Ok(false) when the app is open, the instance has a picture, or the database isn't the
/// layout Atlas knows.
pub async fn set_icon(root: &Path, instance_path: &str, icon: &[u8]) -> ReadResult<bool> {
    if running() {
        return Ok(false);
    }
    set_icon_while_closed(root, instance_path, icon).await
}

async fn set_icon_while_closed(root: &Path, instance_path: &str, icon: &[u8]) -> ReadResult<bool> {
    use sha1::{Digest, Sha1};
    let db = root.join("app.db");
    let Some(ext) = image_ext(icon) else {
        return Ok(false);
    };
    if !db.is_file() {
        return Ok(false);
    }
    let options = SqliteConnectOptions::new()
        .filename(&db)
        .busy_timeout(Duration::from_secs(2));
    let mut conn = SqliteConnection::connect_with(&options).await?;
    let known: Option<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('instances') WHERE name = 'icon_path'")
            .fetch_optional(&mut conn)
            .await?;
    let current: Option<(Option<String>,)> = match known {
        Some(_) => {
            sqlx::query_as("SELECT icon_path FROM instances WHERE path = ?")
                .bind(instance_path)
                .fetch_optional(&mut conn)
                .await?
        }
        None => None,
    };
    if !matches!(current, Some((None,))) {
        conn.close().await?;
        return Ok(false);
    }
    let dir = root.join("caches").join("icons");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("{}.{ext}", hex::encode(Sha1::digest(icon))));
    if !file.is_file() {
        std::fs::write(&file, icon)?;
    }
    let done =
        sqlx::query("UPDATE instances SET icon_path = ? WHERE path = ? AND icon_path IS NULL")
            .bind(file.to_string_lossy().as_ref())
            .bind(instance_path)
            .execute(&mut conn)
            .await?;
    conn.close().await?;
    Ok(done.rows_affected() == 1)
}

#[derive(FromRow)]
struct Stage {
    path: String,
    name: String,
    install_stage: String,
}

const STAGES: &str = "SELECT path, name, install_stage FROM instances";
const LEGACY_STAGES: &str = "SELECT path, name, install_stage FROM profiles";

/// The instance the app made from the pack, once it's past installing the pack itself and
/// its folder exists (the app can report the stage before making the folder, while it
/// downloads a big loader). From then on it only installs the game and loader, outside
/// the instance folder.
async fn find_new(root: &Path, before: &HashSet<String>, name: &str) -> ReadResult<Option<String>> {
    let stem = file_stem(name);
    let rows: Vec<Stage> = rows(root, STAGES, LEGACY_STAGES).await?;
    Ok(rows
        .into_iter()
        .find(|s| !before.contains(&s.path) && (s.name == name || s.name == stem))
        .filter(|s| {
            matches!(
                s.install_stage.as_str(),
                "pack_installed" | "minecraft_installing" | "installing" | "installed"
            ) && root.join("profiles").join(&s.path).is_dir()
        })
        .map(|s| s.path))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Modrinth root with the real table definitions (only the columns Atlas reads
    /// plus the NOT NULL ones) and two instances.
    async fn fixture() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("atlas-mr-test-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(root.join("profiles/Better MC [FABRIC] - BMC2")).unwrap();
        let options = SqliteConnectOptions::new()
            .filename(root.join("app.db"))
            .create_if_missing(true);
        let mut c = SqliteConnection::connect_with(&options).await.unwrap();
        for sql in [
            "CREATE TABLE instances (id TEXT NOT NULL, path TEXT NOT NULL, applied_content_set_id TEXT NULL,
                install_stage TEXT NOT NULL, launcher_feature_version TEXT NOT NULL, update_channel TEXT NOT NULL DEFAULT 'release',
                name TEXT NOT NULL, icon_path TEXT NULL, created INTEGER NOT NULL, modified INTEGER NOT NULL, last_played INTEGER NULL,
                submitted_time_played INTEGER NOT NULL DEFAULT 0, recent_time_played INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (id), UNIQUE (path))",
            "CREATE TABLE instance_content_sets (id TEXT NOT NULL, instance_id TEXT NOT NULL, name TEXT NOT NULL,
                source_kind TEXT NOT NULL, status TEXT NOT NULL, game_version TEXT NOT NULL, protocol_version INTEGER NULL,
                loader TEXT NOT NULL, loader_version TEXT NULL, created INTEGER NOT NULL, modified INTEGER NOT NULL, PRIMARY KEY (id))",
            "INSERT INTO instances VALUES ('legacy:BMC2', 'Better MC [FABRIC] - BMC2', 'legacy:BMC2:default', 'installed', 'x', 'release',
                'Better MC [FABRIC] - BMC2', 'C:\\icons\\bmc.png', 1776231124, 1789529806, 1780559691, 100, 3553)",
            "INSERT INTO instances VALUES ('legacy:New', 'New', NULL, 'installed', 'x', 'release', 'Never played', NULL, 1, 1, NULL, 0, 0)",
            "INSERT INTO instance_content_sets VALUES ('legacy:BMC2:default', 'legacy:BMC2', 'Default', 'modrinth_modpack', 'available',
                '1.20.1', 763, 'fabric', '0.16.9', 1, 1)",
        ] {
            sqlx::query(sql).execute(&mut c).await.unwrap();
        }
        c.close().await.unwrap();
        root
    }

    #[tokio::test]
    async fn pictures_go_only_to_instances_without_one() {
        let root = fixture().await;
        let png = b"\x89PNG\r\n\x1a\nnot really a picture".to_vec();
        assert!(set_icon_while_closed(&root, "New", &png).await.unwrap());
        assert!(
            !set_icon_while_closed(&root, "New", &png).await.unwrap(),
            "it has one now"
        );
        assert!(
            !set_icon_while_closed(&root, "Better MC [FABRIC] - BMC2", &png)
                .await
                .unwrap(),
            "never replaces the user's own"
        );
        assert!(!set_icon_while_closed(&root, "Gone", &png).await.unwrap());
        assert!(
            !set_icon_while_closed(&root, "New", b"not an image")
                .await
                .unwrap()
        );

        let new = list(&root)
            .await
            .unwrap()
            .into_iter()
            .find(|i| i.launcher_instance_id == "New")
            .unwrap();
        let icon = new.icon.unwrap();
        let file = PathBuf::from(icon.strip_prefix("file:").unwrap());
        assert!(file.starts_with(root.join("caches").join("icons")));
        assert_eq!(std::fs::read(&file).unwrap(), png);
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn reads_instances_with_versions_from_content_sets() {
        let root = fixture().await;
        assert!(is_root(&root));
        let mut list = list(&root).await.unwrap();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        std::fs::remove_dir_all(&root).ok();

        assert_eq!(list.len(), 2);
        let bmc = &list[0];
        assert_eq!(bmc.launcher_instance_id, "Better MC [FABRIC] - BMC2");
        assert_eq!(
            bmc.game_dir,
            root.join("profiles/Better MC [FABRIC] - BMC2")
        );
        assert_eq!(bmc.minecraft_version.as_deref(), Some("1.20.1"));
        assert_eq!(bmc.loader, Some(Loader::Fabric));
        assert_eq!(bmc.loader_version.as_deref(), Some("0.16.9"));
        assert_eq!(
            bmc.last_played_at.as_deref(),
            Some("2026-06-04T07:54:51.000Z")
        );
        assert_eq!(bmc.total_playtime_seconds, Some(3653));
        assert_eq!(bmc.icon.as_deref(), Some("file:C:\\icons\\bmc.png"));

        // no content set (half-installed) and never played
        let new = &list[1];
        assert_eq!(new.name, "Never played");
        assert_eq!(new.minecraft_version, None);
        assert_eq!(new.loader, None);
        assert_eq!(new.last_played_at, None);
    }

    fn neoforge() -> NewInstance {
        NewInstance {
            name: "Friday: SMP".into(),
            minecraft: "1.21.1".into(),
            loader: Loader::Neoforge,
            loader_version: Some("21.1.77".into()),
            icon: None,
        }
    }

    #[test]
    fn packs_name_only_the_versions() {
        let file = std::env::temp_dir().join(format!(
            "atlas-mrpack-test-{}.mrpack",
            crate::util::ids::new_id()
        ));
        write_pack(&file, &neoforge(), Some("21.1.77")).unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&file).unwrap()).unwrap();
        assert_eq!(zip.len(), 1);
        let index: serde_json::Value =
            serde_json::from_reader(zip.by_name("modrinth.index.json").unwrap()).unwrap();
        drop(zip);
        std::fs::remove_file(&file).ok();
        assert_eq!(index["formatVersion"], 1);
        assert_eq!(index["game"], "minecraft");
        assert_eq!(index["name"], "Friday: SMP");
        assert_eq!(index["files"], serde_json::json!([]));
        assert_eq!(
            index["dependencies"],
            serde_json::json!({ "minecraft": "1.21.1", "neoforge": "21.1.77" })
        );
    }

    #[tokio::test]
    async fn finds_the_instance_once_the_pack_is_installed() {
        let root = fixture().await;
        let before: HashSet<String> = rows::<Stage>(&root, STAGES, LEGACY_STAGES)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.path)
            .collect();
        assert_eq!(before.len(), 2);
        assert_eq!(find_new(&root, &before, "Friday: SMP").await.unwrap(), None);

        let options = SqliteConnectOptions::new().filename(root.join("app.db"));
        let mut c = SqliteConnection::connect_with(&options).await.unwrap();
        // the app names it after the file first, while it installs the pack
        sqlx::query(
            "INSERT INTO instances (id, path, install_stage, launcher_feature_version, name, created, modified)
             VALUES ('x', 'Friday_ SMP', 'pack_installing', 'x', 'Friday_ SMP', 1, 1)",
        )
        .execute(&mut c)
        .await
        .unwrap();
        assert_eq!(find_new(&root, &before, "Friday: SMP").await.unwrap(), None);
        sqlx::query("UPDATE instances SET name = 'Friday: SMP', install_stage = 'minecraft_installing' WHERE id = 'x'")
            .execute(&mut c)
            .await
            .unwrap();
        c.close().await.unwrap();
        assert_eq!(
            find_new(&root, &before, "Friday: SMP").await.unwrap(),
            None,
            "its folder isn't there yet"
        );
        std::fs::create_dir_all(root.join("profiles/Friday_ SMP")).unwrap();
        assert_eq!(
            find_new(&root, &before, "Friday: SMP")
                .await
                .unwrap()
                .as_deref(),
            Some("Friday_ SMP")
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
