//! `setting` table: app settings as JSON values.
//! Keys so far: `backups_to_keep` (3), `dev_source:<shared id>` (debug builds, sync/source.rs).

use serde::{Serialize, de::DeserializeOwned};
use sqlx::SqlitePool;

pub async fn get<T: DeserializeOwned>(db: &SqlitePool, key: &str) -> sqlx::Result<Option<T>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM setting WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await?;
    Ok(row.and_then(|(v,)| serde_json::from_str(&v).ok()))
}

pub async fn get_or<T: DeserializeOwned>(
    db: &SqlitePool,
    key: &str,
    default: T,
) -> sqlx::Result<T> {
    Ok(get(db, key).await?.unwrap_or(default))
}

pub async fn set<T: Serialize>(db: &SqlitePool, key: &str, value: &T) -> sqlx::Result<()> {
    let json = serde_json::to_string(value).expect("setting serializes");
    sqlx::query("INSERT INTO setting (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value")
        .bind(key)
        .bind(json)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete(db: &SqlitePool, key: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM setting WHERE key = ?")
        .bind(key)
        .execute(db)
        .await?;
    Ok(())
}
