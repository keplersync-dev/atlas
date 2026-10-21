//! `backup` table: snapshots taken before an update, revert or restore.
//! T1 only reads it; the sync engine (T4) writes it.

use sqlx::{FromRow, SqlitePool};

use crate::dto::{Backup, BackupReason, from_wire};

#[derive(Debug, Clone, FromRow)]
pub struct BackupRow {
    pub id: String,
    pub instance_id: String,
    pub reason: String,
    pub from_revision: Option<i64>,
    pub to_revision: Option<i64>,
    /// JSON [{ path, action }]
    pub files: String,
    pub size_bytes: i64,
    pub created_at: String,
}

impl BackupRow {
    pub fn to_dto(&self) -> Backup {
        let file_count = serde_json::from_str::<Vec<serde_json::Value>>(&self.files)
            .map(|f| f.len() as u32)
            .unwrap_or(0);
        Backup {
            id: self.id.clone(),
            reason: from_wire(&self.reason).unwrap_or(BackupReason::BeforeUpdate),
            from_revision: self.from_revision.map(|r| r as u32),
            to_revision: self.to_revision.map(|r| r as u32),
            file_count,
            size_bytes: self.size_bytes.max(0) as u64,
            created_at: self.created_at.clone(),
        }
    }
}

/// Newest first.
pub async fn list(db: &SqlitePool, instance_id: &str) -> sqlx::Result<Vec<BackupRow>> {
    sqlx::query_as("SELECT * FROM backup WHERE instance_id = ? ORDER BY created_at DESC")
        .bind(instance_id)
        .fetch_all(db)
        .await
}

pub async fn get(db: &SqlitePool, instance_id: &str, id: &str) -> sqlx::Result<Option<BackupRow>> {
    sqlx::query_as("SELECT * FROM backup WHERE id = ? AND instance_id = ?")
        .bind(id)
        .bind(instance_id)
        .fetch_optional(db)
        .await
}
