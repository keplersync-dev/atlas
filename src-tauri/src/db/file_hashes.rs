//! `file_hash` table: what Share last read of an instance's files besides mods and packs.

use std::collections::HashMap;

use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq)]
pub struct FileHash {
    pub path: String,
    pub size_bytes: u64,
    pub modified_ms: i64,
    pub sha256: String,
    pub settings: Option<String>,
}

/// The instance's saved hashes, by path.
pub async fn list(db: &SqlitePool, instance_id: &str) -> sqlx::Result<HashMap<String, FileHash>> {
    let rows: Vec<(String, i64, i64, String, Option<String>)> = sqlx::query_as(
        "SELECT path, size_bytes, modified_ms, sha256, settings FROM file_hash WHERE instance_id = ?",
    )
    .bind(instance_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(path, size, modified_ms, sha256, settings)| {
            let h = FileHash {
                path: path.clone(),
                size_bytes: size.max(0) as u64,
                modified_ms,
                sha256,
                settings,
            };
            (path, h)
        })
        .collect())
}

/// Replace the instance's saved hashes: files no longer read are forgotten.
pub async fn replace(db: &SqlitePool, instance_id: &str, hashes: &[FileHash]) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM file_hash WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&mut *tx)
        .await?;
    for h in hashes {
        sqlx::query(
            "INSERT INTO file_hash (instance_id, path, size_bytes, modified_ms, sha256, settings)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(instance_id)
        .bind(&h.path)
        .bind(h.size_bytes as i64)
        .bind(h.modified_ms)
        .bind(&h.sha256)
        .bind(&h.settings)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}
