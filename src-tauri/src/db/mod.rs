//! The local SQLite database (`atlas.db`). Schema: migrations/0001_init.sql.
//!
//! One file per table, each with plain async functions taking the pool. Row structs
//! are separate from the DTOs in crate::dto; services convert one into the other.

pub mod backups;
pub mod cloud_cache;
pub mod content;
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
}
