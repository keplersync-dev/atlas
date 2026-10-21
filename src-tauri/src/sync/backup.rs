//! Backups: copies of every file an update, revert or restore changes, taken just before
//! (tauri-plan §12.7). Restoring undoes exactly what the backup recorded.
//!
//!   backups/<instance id>/<backup id>/<same relative paths>
//!   backups/<instance id>/<backup id>/state.json   the database's view before the change

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::{self, content::ContentRow};
use crate::dto::{BackupReason, to_wire};
use crate::error::AppResult;
use crate::state::AppPaths;
use crate::util::{ids::new_id, time::now_iso};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileAction {
    /// A file that was there and got overwritten: restored from the copy.
    Replaced,
    /// A file that was there and got deleted: restored from the copy.
    Removed,
    /// A file that wasn't there: deleted on restore.
    Added,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileRecord {
    pub path: String,
    pub action: FileAction,
}

/// The database's view of the instance before the change, so a restore puts it back too.
#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub synced_revision: Option<i64>,
    pub synced_manifest: Option<String>,
    pub synced_manifest_hash: Option<String>,
    pub rows: Vec<ContentRow>,
}

pub fn dir(paths: &AppPaths, instance_id: &str, backup_id: &str) -> PathBuf {
    paths.backups.join(instance_id).join(backup_id)
}

/// Copy the files about to change and record the backup. Blocking file copies are small
/// in number (only what the update touches).
#[allow(clippy::too_many_arguments)]
pub async fn create(
    db: &SqlitePool,
    paths: &AppPaths,
    instance_id: &str,
    game_dir: &Path,
    reason: BackupReason,
    from_revision: Option<u32>,
    to_revision: Option<u32>,
    files: Vec<FileRecord>,
) -> AppResult<(String, PathBuf)> {
    let id = new_id();
    let folder = dir(paths, instance_id, &id);
    std::fs::create_dir_all(&folder)?;
    let mut size = 0u64;
    for f in files.iter().filter(|f| f.action != FileAction::Added) {
        let from = game_dir.join(&f.path);
        if from.is_file() {
            let to = folder.join(&f.path);
            std::fs::create_dir_all(to.parent().expect("relative path has a parent"))?;
            size += std::fs::copy(&from, &to)?;
        }
    }
    let row = db::instances::get(db, instance_id).await?;
    let snapshot = Snapshot {
        synced_revision: row.as_ref().and_then(|r| r.synced_revision),
        synced_manifest: row.as_ref().and_then(|r| r.synced_manifest.clone()),
        synced_manifest_hash: row.as_ref().and_then(|r| r.synced_manifest_hash.clone()),
        rows: db::content::list(db, instance_id).await?,
    };
    std::fs::write(folder.join("state.json"), serde_json::to_vec(&snapshot)?)?;
    sqlx::query(
        "INSERT INTO backup (id, instance_id, reason, from_revision, to_revision, files, size_bytes, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(instance_id)
    .bind(to_wire(reason))
    .bind(from_revision.map(i64::from))
    .bind(to_revision.map(i64::from))
    .bind(serde_json::to_string(&files)?)
    .bind(size as i64)
    .bind(now_iso())
    .execute(db)
    .await?;
    // Not pruned here: a restore reads from an older backup after creating this one.
    // Jobs call prune() when they finish.
    Ok((id, folder))
}

/// Keep the newest `backups_to_keep` (default 3) backups of an instance.
pub async fn prune(db: &SqlitePool, paths: &AppPaths, instance_id: &str) -> AppResult<()> {
    let keep: usize = db::settings::get_or(db, "backups_to_keep", 3usize).await?;
    for old in db::backups::list(db, instance_id)
        .await?
        .into_iter()
        .skip(keep)
    {
        let _ = std::fs::remove_dir_all(dir(paths, instance_id, &old.id));
        sqlx::query("DELETE FROM backup WHERE id = ?")
            .bind(&old.id)
            .execute(db)
            .await?;
    }
    Ok(())
}

pub fn read_snapshot(folder: &Path) -> AppResult<Snapshot> {
    Ok(serde_json::from_slice(&std::fs::read(
        folder.join("state.json"),
    )?)?)
}

/// Put the database back the way the snapshot recorded it. The caller rescans afterwards,
/// which brings the rows in line with the files while keeping their settings.
pub async fn restore_snapshot(db: &SqlitePool, instance_id: &str, s: &Snapshot) -> AppResult<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM instance_content WHERE instance_id = ?")
        .bind(instance_id)
        .execute(&mut *tx)
        .await?;
    for r in &s.rows {
        sqlx::query(
            "INSERT INTO instance_content (id, instance_id, kind, path, enabled, size_bytes, mtime_ms, sha1, sha256,
                murmur2, mod_id, source, project_id, version_id, name, version_label, author, icon_url, download_url,
                page_url, side, resolved_at, is_managed, share_mode, published_share_mode, installed)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&r.id).bind(instance_id).bind(&r.kind).bind(&r.path).bind(r.enabled).bind(r.size_bytes)
        .bind(r.mtime_ms).bind(&r.sha1).bind(&r.sha256).bind(r.murmur2).bind(&r.mod_id).bind(&r.source)
        .bind(&r.project_id).bind(&r.version_id).bind(&r.name).bind(&r.version_label).bind(&r.author)
        .bind(&r.icon_url).bind(&r.download_url).bind(&r.page_url).bind(&r.side).bind(&r.resolved_at)
        .bind(r.is_managed).bind(&r.share_mode).bind(&r.published_share_mode).bind(r.installed)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE instance SET synced_revision = ?, synced_manifest = ?, synced_manifest_hash = ? WHERE id = ?")
        .bind(s.synced_revision)
        .bind(&s.synced_manifest)
        .bind(&s.synced_manifest_hash)
        .bind(instance_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
