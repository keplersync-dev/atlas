//! Modrinth App.
//!
//! <root>/app.db (SQLite, the app's own; opened read-only, never written)
//! <root>/profiles/<path>/ is the game dir.
//!
//! Current schema (checked against Modrinth App, migration 20260918120000):
//!   instances(id, path, name, icon_path, last_played [unix s],
//!             submitted_time_played, recent_time_played [s], applied_content_set_id, ...)
//!   instance_content_sets(id, game_version, loader, loader_version, ...)
//! Older versions had one `profiles` table instead (unverified, kept as a fallback).

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{Connection, FromRow};

use super::util::{env_dir, first_existing, from_unix_seconds, loader_from};
use super::{FoundInstance, ReadError, ReadResult};

pub fn default_roots() -> Vec<PathBuf> {
    let Some(appdata) = env_dir("APPDATA") else { return vec![] };
    vec![appdata.join("ModrinthApp"), appdata.join("com.modrinth.theseus")]
}

pub fn is_root(path: &Path) -> bool {
    path.join("app.db").is_file() && path.join("profiles").is_dir()
}

pub fn program() -> Option<PathBuf> {
    let local = env_dir("LOCALAPPDATA")?;
    first_existing([
        local.join("Modrinth App").join("Modrinth App.exe"),
        local.join("Programs").join("Modrinth App").join("Modrinth App.exe"),
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
    let db = root.join("app.db");
    if !db.is_file() {
        return Ok(vec![]);
    }
    let rows = match read(&db, true).await {
        Ok(rows) => rows,
        // Locked or mid-write: read a copy instead of waiting on the app.
        Err(e) => {
            tracing::debug!(error = %e, "reading a copy of Modrinth's app.db");
            read_copy(&db).await?
        }
    };
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

async fn read(db: &Path, read_only: bool) -> ReadResult<Vec<Row>> {
    let options = SqliteConnectOptions::new()
        .filename(db)
        .read_only(read_only)
        .busy_timeout(Duration::from_secs(2));
    let mut conn = SqliteConnection::connect_with(&options).await?;
    let has_instances: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'instances'")
            .fetch_optional(&mut conn)
            .await?;
    let sql = if has_instances.is_some() { CURRENT } else { LEGACY };
    let rows = sqlx::query_as::<_, Row>(sql).fetch_all(&mut conn).await?;
    conn.close().await?;
    Ok(rows)
}

async fn read_copy(db: &Path) -> ReadResult<Vec<Row>> {
    let dir = std::env::temp_dir().join(format!("atlas-modrinth-{}", crate::util::ids::new_id()));
    std::fs::create_dir_all(&dir)?;
    let result = async {
        for suffix in ["", "-wal", "-shm"] {
            let from = PathBuf::from(format!("{}{suffix}", db.display()));
            if from.is_file() {
                std::fs::copy(&from, dir.join(format!("app.db{suffix}")))?;
            }
        }
        read(&dir.join("app.db"), false).await
    }
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    result.map_err(|e| ReadError::Format(format!("Modrinth app.db: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::Loader;

    /// A Modrinth root with the real table definitions (only the columns Atlas reads
    /// plus the NOT NULL ones) and two instances.
    async fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("atlas-mr-test-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(root.join("profiles/Better MC [FABRIC] - BMC2")).unwrap();
        let options = SqliteConnectOptions::new().filename(root.join("app.db")).create_if_missing(true);
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
    async fn reads_instances_with_versions_from_content_sets() {
        let root = fixture().await;
        assert!(is_root(&root));
        let mut list = list(&root).await.unwrap();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        std::fs::remove_dir_all(&root).ok();

        assert_eq!(list.len(), 2);
        let bmc = &list[0];
        assert_eq!(bmc.launcher_instance_id, "Better MC [FABRIC] - BMC2");
        assert_eq!(bmc.game_dir, root.join("profiles/Better MC [FABRIC] - BMC2"));
        assert_eq!(bmc.minecraft_version.as_deref(), Some("1.20.1"));
        assert_eq!(bmc.loader, Some(Loader::Fabric));
        assert_eq!(bmc.loader_version.as_deref(), Some("0.16.9"));
        assert_eq!(bmc.last_played_at.as_deref(), Some("2026-06-04T07:54:51.000Z"));
        assert_eq!(bmc.total_playtime_seconds, Some(3653));
        assert_eq!(bmc.icon.as_deref(), Some("file:C:\\icons\\bmc.png"));

        // no content set (half-installed) and never played
        let new = &list[1];
        assert_eq!(new.name, "Never played");
        assert_eq!(new.minecraft_version, None);
        assert_eq!(new.loader, None);
        assert_eq!(new.last_played_at, None);
    }
}
