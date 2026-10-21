//! `launcher` table: launcher installs found on this PC or added by hand.

use sqlx::{FromRow, SqlitePool};

use crate::dto::{LauncherKind, from_wire, to_wire};
use crate::util::{ids::new_id, time::now_iso};

#[derive(Debug, Clone, FromRow)]
pub struct LauncherRow {
    pub id: String,
    pub kind: String,
    pub root_path: String,
    pub added_by_user: bool,
    pub last_scanned_at: Option<String>,
    pub created_at: String,
}

impl LauncherRow {
    pub fn kind(&self) -> Option<LauncherKind> {
        from_wire(&self.kind)
    }
}

pub async fn list(db: &SqlitePool) -> sqlx::Result<Vec<LauncherRow>> {
    sqlx::query_as("SELECT * FROM launcher ORDER BY created_at")
        .fetch_all(db)
        .await
}

pub async fn get(db: &SqlitePool, id: &str) -> sqlx::Result<Option<LauncherRow>> {
    sqlx::query_as("SELECT * FROM launcher WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
}

/// Windows paths are case-insensitive, so compare them that way.
pub async fn find_by_root(db: &SqlitePool, root: &str) -> sqlx::Result<Option<LauncherRow>> {
    sqlx::query_as("SELECT * FROM launcher WHERE root_path = ? COLLATE NOCASE")
        .bind(root)
        .fetch_optional(db)
        .await
}

pub async fn insert(
    db: &SqlitePool,
    kind: LauncherKind,
    root: &str,
    added_by_user: bool,
) -> sqlx::Result<LauncherRow> {
    sqlx::query_as(
        "INSERT INTO launcher (id, kind, root_path, added_by_user, created_at)
         VALUES (?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(new_id())
    .bind(to_wire(kind))
    .bind(root)
    .bind(added_by_user)
    .bind(now_iso())
    .fetch_one(db)
    .await
}

pub async fn delete(db: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM launcher WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(done.rows_affected() > 0)
}
