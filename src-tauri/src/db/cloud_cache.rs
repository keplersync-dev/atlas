//! `cloud_cache`: the last answer to each server list (friends, requests, activity), so
//! the app still shows them offline. Values are the DTO JSON the UI receives.

use serde::{Serialize, de::DeserializeOwned};
use sqlx::SqlitePool;

pub async fn get<T: DeserializeOwned>(db: &SqlitePool, key: &str) -> sqlx::Result<Option<T>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM cloud_cache WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await?;
    Ok(row.and_then(|(v,)| serde_json::from_str(&v).ok()))
}

pub async fn set<T: Serialize>(db: &SqlitePool, key: &str, value: &T) -> sqlx::Result<()> {
    let json = serde_json::to_string(value).expect("cached value serializes");
    sqlx::query(
        "INSERT INTO cloud_cache (key, value, fetched_at) VALUES (?, ?, ?)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value, fetched_at = excluded.fetched_at",
    )
    .bind(key)
    .bind(json)
    .bind(crate::util::time::now_iso())
    .execute(db)
    .await?;
    Ok(())
}

/// Forget everything (signing out: the next account mustn't see this one's data).
pub async fn clear(db: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM cloud_cache").execute(db).await?;
    Ok(())
}
