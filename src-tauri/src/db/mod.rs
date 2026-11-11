//! The local SQLite database (`atlas.db`). Schema: migrations/0001_init.sql.
//!
//! One file per table, each with plain async functions taking the pool. Row structs
//! are separate from the DTOs in crate::dto; services convert one into the other.

pub mod backups;
pub mod cloud_cache;
pub mod content;
pub mod file_hashes;
pub mod instances;
pub mod launchers;
pub mod settings;

use std::{path::Path, str::FromStr, time::Duration};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

use crate::error::AppResult;

/// Open (or create) the database file and bring its schema up to date.
pub async fn open(file: &Path) -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(file)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

/// A fresh database in memory, for tests. One connection, because every
/// in-memory connection would otherwise be its own empty database.
pub async fn open_in_memory() -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    migrate(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &SqlitePool) -> AppResult<()> {
    // Embeds migrations/*.sql into the binary at compile time.
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn migrations_run_and_tables_exist() {
        let pool = super::open_in_memory().await.unwrap();
        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();
        let names: Vec<_> = tables.into_iter().map(|(n,)| n).collect();
        for t in [
            "backup",
            "cloud_cache",
            "instance",
            "instance_content",
            "launcher",
            "setting",
        ] {
            assert!(names.iter().any(|n| n == t), "missing table {t}: {names:?}");
        }
    }

    #[tokio::test]
    async fn files_already_off_become_only_me_except_for_viewers() {
        let pool = super::open_in_memory().await.unwrap();
        for (id, role) in [
            ("mine", None),
            ("owned", Some("owner")),
            ("viewed", Some("viewer")),
        ] {
            sqlx::query(
                "INSERT INTO instance (id, launcher_kind, game_dir, name, shared_instance_id, shared_role, tracked_at)
                 VALUES (?, 'folder', ?, ?, ?, ?, '')",
            )
            .bind(id)
            .bind(format!("C:/{id}"))
            .bind(id)
            .bind(role.map(|_| format!("s-{id}")))
            .bind(role)
            .execute(&pool)
            .await
            .unwrap();
            for (path, enabled) in [("mods/on.jar", true), ("mods/off.jar", false)] {
                sqlx::query(
                    "INSERT INTO instance_content (id, instance_id, kind, path, enabled, size_bytes, sha1, sha256, source, name)
                     VALUES (?, ?, 'mod', ?, ?, 1, 'a', 'b', 'local', 'x')",
                )
                .bind(format!("{id}-{path}"))
                .bind(id)
                .bind(path)
                .bind(enabled)
                .execute(&pool)
                .await
                .unwrap();
            }
        }
        sqlx::raw_sql(include_str!("../../migrations/0011_off_means_only_me.sql"))
            .execute(&pool)
            .await
            .unwrap();
        let modes: Vec<(String, String)> =
            sqlx::query_as("SELECT id, share_mode FROM instance_content ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        let private: Vec<&str> = modes
            .iter()
            .filter(|(_, m)| m == "private")
            .map(|(id, _)| id.as_str())
            .collect();
        assert_eq!(private, ["mine-mods/off.jar", "owned-mods/off.jar"]);
    }

    #[tokio::test]
    async fn content_is_indexed_once_by_instance() {
        let pool = super::open_in_memory().await.unwrap();
        let indexes: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'instance_content'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let names: Vec<_> = indexes.into_iter().map(|(n,)| n).collect();
        assert!(
            !names
                .iter()
                .any(|n| n == "instance_content_instance_id_idx")
        );
        assert!(
            names
                .iter()
                .any(|n| n.starts_with("sqlite_autoindex_instance_content")),
            "the UNIQUE (instance_id, path) one covers it: {names:?}"
        );
    }
}
