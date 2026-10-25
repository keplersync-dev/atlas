//! `instance` table: tracked game folders.

use std::collections::HashMap;

use sqlx::{FromRow, SqlitePool};

use crate::dto::{InstanceStatus, LauncherKind, Loader, from_wire, to_wire};
use crate::util::{ids::new_id, time::now_iso};

#[derive(Debug, Clone, FromRow)]
pub struct InstanceRow {
    pub id: String,
    pub launcher_id: Option<String>,
    pub launcher_kind: String,
    pub launcher_instance_id: Option<String>,
    pub game_dir: String,
    pub name: String,
    pub icon: Option<String>,
    pub minecraft_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub last_played_at: Option<String>,
    pub total_playtime_seconds: Option<i64>,
    pub status: String,
    pub is_favorite: bool,
    pub hide_activity: bool,
    pub shared_instance_id: Option<String>,
    pub shared_role: Option<String>,
    pub synced_revision: Option<i64>,
    pub synced_manifest_hash: Option<String>,
    pub synced_manifest: Option<String>,
    pub sync_report_due: bool,
    pub tracked_at: String,
    pub last_scanned_at: Option<String>,
}

impl InstanceRow {
    pub fn kind(&self) -> LauncherKind {
        from_wire(&self.launcher_kind).unwrap_or(LauncherKind::Folder)
    }
    pub fn loader(&self) -> Option<Loader> {
        self.loader.as_deref().and_then(from_wire)
    }
    /// The manifest this folder was last made equal to, if it's shared.
    pub fn synced_manifest(&self) -> Option<crate::sync::manifest::Manifest> {
        serde_json::from_str(self.synced_manifest.as_deref()?).ok()
    }
    pub fn role(&self) -> Option<crate::dto::MemberRole> {
        self.shared_role.as_deref().and_then(from_wire)
    }
    /// Stored status: Ready or Missing. (Syncing only exists at runtime.)
    pub fn status(&self) -> InstanceStatus {
        from_wire(&self.status).unwrap_or(InstanceStatus::Ready)
    }
}

/// What a launcher says about an instance; written on track and on every rescan.
#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
    pub name: String,
    pub icon: Option<String>,
    pub minecraft_version: Option<String>,
    pub loader: Option<Loader>,
    pub loader_version: Option<String>,
    pub last_played_at: Option<String>,
    pub total_playtime_seconds: Option<i64>,
}

impl Metadata {
    pub fn of(row: &InstanceRow) -> Self {
        Self {
            name: row.name.clone(),
            icon: row.icon.clone(),
            minecraft_version: row.minecraft_version.clone(),
            loader: row.loader(),
            loader_version: row.loader_version.clone(),
            last_played_at: row.last_played_at.clone(),
            total_playtime_seconds: row.total_playtime_seconds,
        }
    }
}

pub struct NewInstance {
    pub launcher_id: Option<String>,
    pub kind: LauncherKind,
    pub launcher_instance_id: Option<String>,
    pub game_dir: String,
    pub meta: Metadata,
}

pub async fn list(db: &SqlitePool) -> sqlx::Result<Vec<InstanceRow>> {
    sqlx::query_as("SELECT * FROM instance ORDER BY tracked_at")
        .fetch_all(db)
        .await
}

pub async fn get(db: &SqlitePool, id: &str) -> sqlx::Result<Option<InstanceRow>> {
    sqlx::query_as("SELECT * FROM instance WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn find_by_game_dir(
    db: &SqlitePool,
    game_dir: &str,
) -> sqlx::Result<Option<InstanceRow>> {
    sqlx::query_as("SELECT * FROM instance WHERE game_dir = ? COLLATE NOCASE")
        .bind(game_dir)
        .fetch_optional(db)
        .await
}

pub async fn insert(db: &SqlitePool, new: NewInstance) -> sqlx::Result<InstanceRow> {
    let now = now_iso();
    let m = new.meta;
    sqlx::query_as(
        "INSERT INTO instance (id, launcher_id, launcher_kind, launcher_instance_id, game_dir, name, icon,
            minecraft_version, loader, loader_version, last_played_at, total_playtime_seconds,
            status, tracked_at, last_scanned_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'ready', ?, ?) RETURNING *",
    )
    .bind(new_id())
    .bind(new.launcher_id)
    .bind(to_wire(new.kind))
    .bind(new.launcher_instance_id)
    .bind(new.game_dir)
    .bind(m.name)
    .bind(m.icon)
    .bind(m.minecraft_version)
    .bind(m.loader.map(to_wire))
    .bind(m.loader_version)
    .bind(m.last_played_at)
    .bind(m.total_playtime_seconds)
    .bind(&now)
    .bind(&now)
    .fetch_one(db)
    .await
}

/// After a rescan: new launcher metadata (if the launcher still lists it) and status.
pub async fn update_scan(
    db: &SqlitePool,
    id: &str,
    meta: Option<&Metadata>,
    status: InstanceStatus,
) -> sqlx::Result<()> {
    let now = now_iso();
    match meta {
        Some(m) => {
            sqlx::query(
                "UPDATE instance SET name = ?, icon = ?, minecraft_version = ?, loader = ?, loader_version = ?,
                    last_played_at = ?, total_playtime_seconds = ?, status = ?, last_scanned_at = ?
                 WHERE id = ?",
            )
            .bind(&m.name)
            .bind(&m.icon)
            .bind(&m.minecraft_version)
            .bind(m.loader.map(to_wire))
            .bind(&m.loader_version)
            .bind(&m.last_played_at)
            .bind(m.total_playtime_seconds)
            .bind(to_wire(status))
            .bind(&now)
            .bind(id)
            .execute(db)
            .await?
        }
        None => {
            sqlx::query("UPDATE instance SET status = ?, last_scanned_at = ? WHERE id = ?")
                .bind(to_wire(status))
                .bind(&now)
                .bind(id)
                .execute(db)
                .await?
        }
    };
    Ok(())
}

pub async fn update_prefs(
    db: &SqlitePool,
    id: &str,
    is_favorite: Option<bool>,
    hide_activity: Option<bool>,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE instance SET is_favorite = COALESCE(?, is_favorite), hide_activity = COALESCE(?, hide_activity)
         WHERE id = ?",
    )
    .bind(is_favorite)
    .bind(hide_activity)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM instance WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(done.rows_affected() > 0)
}

/// Files on disk per instance (`installed = 1`), for `contentCount`.
pub async fn content_counts(db: &SqlitePool) -> sqlx::Result<HashMap<String, u32>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT instance_id, COUNT(*) FROM instance_content WHERE installed = 1 GROUP BY instance_id",
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(id, n)| (id, n as u32)).collect())
}

/// Keep up with role changes: each linked copy takes its role from the server's list of
/// shared instances (the owner can change a member's role at any time).
pub async fn sync_roles(
    db: &SqlitePool,
    shared: &[crate::dto::SharedInstance],
) -> sqlx::Result<()> {
    for s in shared {
        sqlx::query(
            "UPDATE instance SET shared_role = ? WHERE shared_instance_id = ? AND shared_role IS NOT ?",
        )
        .bind(to_wire(s.my_role))
        .bind(&s.id)
        .bind(to_wire(s.my_role))
        .execute(db)
        .await?;
    }
    Ok(())
}

/// Link to a shared instance (or unlink with `None`): role and sync columns.
pub async fn set_link(
    db: &SqlitePool,
    id: &str,
    shared: Option<(&str, crate::dto::MemberRole)>,
) -> sqlx::Result<()> {
    match shared {
        Some((shared_id, role)) => {
            sqlx::query(
                "UPDATE instance SET shared_instance_id = ?, shared_role = ?, synced_revision = 0,
                    synced_manifest = NULL, synced_manifest_hash = NULL WHERE id = ?",
            )
            .bind(shared_id)
            .bind(to_wire(role))
            .bind(id)
            .execute(db)
            .await?
        }
        None => {
            sqlx::query(
                "UPDATE instance SET shared_instance_id = NULL, shared_role = NULL, synced_revision = NULL,
                    synced_manifest = NULL, synced_manifest_hash = NULL, sync_report_due = 0 WHERE id = ?",
            )
            .bind(id)
            .execute(db)
            .await?
        }
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{MemberRole, SharedInstance};

    #[tokio::test]
    async fn roles_follow_the_server() {
        let db = crate::db::open_in_memory().await.unwrap();
        let row = insert(
            &db,
            NewInstance {
                launcher_id: None,
                kind: LauncherKind::Folder,
                launcher_instance_id: None,
                game_dir: "C:/Games/Pack".into(),
                meta: Metadata {
                    name: "Pack".into(),
                    icon: None,
                    minecraft_version: None,
                    loader: None,
                    loader_version: None,
                    last_played_at: None,
                    total_playtime_seconds: None,
                },
            },
        )
        .await
        .unwrap();
        set_link(&db, &row.id, Some(("s1", MemberRole::Viewer)))
            .await
            .unwrap();

        let shared: SharedInstance = serde_json::from_value(serde_json::json!({
            "id": "s1", "name": "Pack", "description": null, "icon": null, "owner": null,
            "minecraftVersion": "1.21.1", "loader": "fabric", "loaderVersion": null,
            "latestRevision": 1, "contentCount": 0, "sizeBytes": 0,
            "updatedAt": "2026-10-25T00:00:00.000Z", "frozen": false, "myRole": "editor",
            "myStatus": "active", "myViaGroup": null, "invitedBy": null, "invitedAt": null,
            "members": [], "groups": [], "optionalItems": []
        }))
        .unwrap();
        sync_roles(&db, &[shared]).await.unwrap();
        let row = get(&db, &row.id).await.unwrap().unwrap();
        assert_eq!(row.role(), Some(MemberRole::Editor), "made an editor");
    }
}
