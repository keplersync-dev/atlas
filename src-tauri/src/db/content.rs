//! `instance_content` table: the file index of each tracked instance.
//! T1 only reads it; the scanner (T2) writes it.

use sqlx::{FromRow, SqliteConnection, SqlitePool};

use crate::dto::{ContentKind, ContentSource, InstanceContent, ShareMode, Side, from_wire};

#[derive(Debug, Clone, FromRow)]
pub struct ContentRow {
    pub id: String,
    pub instance_id: String,
    pub kind: String,
    pub path: String,
    pub enabled: bool,
    pub size_bytes: i64,
    pub mtime_ms: Option<i64>,
    pub sha1: String,
    pub sha256: String,
    pub murmur2: Option<i64>,
    pub source: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub name: String,
    pub version_label: Option<String>,
    pub author: Option<String>,
    pub icon_url: Option<String>,
    pub download_url: Option<String>,
    pub page_url: Option<String>,
    pub side: String,
    pub resolved_at: Option<String>,
    pub is_managed: bool,
    pub share_mode: String,
    pub published_share_mode: Option<String>,
    pub installed: bool,
}

impl ContentRow {
    pub fn to_dto(&self) -> InstanceContent {
        InstanceContent {
            id: self.id.clone(),
            kind: from_wire(&self.kind).unwrap_or(ContentKind::Mod),
            source: from_wire(&self.source).unwrap_or(ContentSource::Local),
            project_id: self.project_id.clone(),
            version_id: self.version_id.clone(),
            name: self.name.clone(),
            version_label: self.version_label.clone(),
            author: self.author.clone(),
            icon_url: self.icon_url.clone(),
            path: self.path.clone(),
            size_bytes: self.size_bytes.max(0) as u64,
            side: from_wire(&self.side).unwrap_or(Side::Both),
            enabled: self.enabled,
            is_managed: self.is_managed,
            share_mode: from_wire(&self.share_mode).unwrap_or(ShareMode::Required),
            published_share_mode: self.published_share_mode.as_deref().and_then(from_wire),
            installed: self.installed,
        }
    }
}

/// Sorted by kind (mods first, the order of the CHECK constraint), then name.
pub async fn list(db: &SqlitePool, instance_id: &str) -> sqlx::Result<Vec<ContentRow>> {
    sqlx::query_as(
        "SELECT * FROM instance_content WHERE instance_id = ?
         ORDER BY CASE kind WHEN 'mod' THEN 0 WHEN 'resourcepack' THEN 1 WHEN 'shaderpack' THEN 2 ELSE 3 END,
                  name COLLATE NOCASE",
    )
    .bind(instance_id)
    .fetch_all(db)
    .await
}

// ── Writes (the scanner and file commands). They take a connection so the scanner
// can apply a whole scan in one transaction: pass `&mut *tx`.


/// A file the scanner found that has no row yet. New files are shared as Required.
pub struct NewFile<'a> {
    pub instance_id: &'a str,
    pub kind: &'a str,
    pub path: &'a str,
    pub enabled: bool,
    pub size_bytes: i64,
    pub mtime_ms: i64,
    pub sha1: &'a str,
    pub sha256: &'a str,
    pub murmur2: i64,
    pub name: &'a str,
}

pub async fn insert_file(c: &mut SqliteConnection, f: NewFile<'_>) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO instance_content (id, instance_id, kind, path, enabled, size_bytes, mtime_ms,
            sha1, sha256, murmur2, source, name, side, is_managed, share_mode, installed)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'local', ?, 'both', 0, 'required', 1)",
    )
    .bind(crate::util::ids::new_id())
    .bind(f.instance_id)
    .bind(f.kind)
    .bind(f.path)
    .bind(f.enabled)
    .bind(f.size_bytes)
    .bind(f.mtime_ms)
    .bind(f.sha1)
    .bind(f.sha256)
    .bind(f.murmur2)
    .bind(f.name)
    .execute(c)
    .await?;
    Ok(())
}

/// The file at an existing row's path is a different file now (new hash): keep the
/// row (and its share setting) but forget where it came from until it's resolved again.
#[allow(clippy::too_many_arguments)]
pub async fn replace_file(
    c: &mut SqliteConnection,
    id: &str,
    enabled: bool,
    size_bytes: i64,
    mtime_ms: i64,
    sha1: &str,
    sha256: &str,
    murmur2: i64,
    name: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE instance_content SET enabled = ?, size_bytes = ?, mtime_ms = ?, sha1 = ?, sha256 = ?,
            murmur2 = ?, installed = 1, source = 'local', project_id = NULL, version_id = NULL, name = ?,
            version_label = NULL, author = NULL, icon_url = NULL, download_url = NULL, page_url = NULL,
            side = 'both', resolved_at = NULL
         WHERE id = ?",
    )
    .bind(enabled)
    .bind(size_bytes)
    .bind(mtime_ms)
    .bind(sha1)
    .bind(sha256)
    .bind(murmur2)
    .bind(name)
    .bind(id)
    .execute(c)
    .await?;
    Ok(())
}

/// Same file, only its size/time or enabled state changed (e.g. renamed to .disabled).
pub async fn touch_file(
    c: &mut SqliteConnection,
    id: &str,
    enabled: bool,
    size_bytes: i64,
    mtime_ms: i64,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE instance_content SET enabled = ?, size_bytes = ?, mtime_ms = ?, installed = 1 WHERE id = ?")
        .bind(enabled)
        .bind(size_bytes)
        .bind(mtime_ms)
        .bind(id)
        .execute(c)
        .await?;
    Ok(())
}

/// An optional item from the shared version whose file is gone: the user chose not to have it.
pub async fn mark_not_installed(c: &mut SqliteConnection, id: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE instance_content SET installed = 0, mtime_ms = NULL WHERE id = ?")
        .bind(id)
        .execute(c)
        .await?;
    Ok(())
}

pub async fn delete(c: &mut SqliteConnection, id: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM instance_content WHERE id = ?").bind(id).execute(c).await?;
    Ok(())
}

pub async fn get(db: &SqlitePool, instance_id: &str, id: &str) -> sqlx::Result<Option<ContentRow>> {
    sqlx::query_as("SELECT * FROM instance_content WHERE id = ? AND instance_id = ?")
        .bind(id)
        .bind(instance_id)
        .fetch_optional(db)
        .await
}

pub async fn set_enabled(db: &SqlitePool, id: &str, enabled: bool) -> sqlx::Result<()> {
    sqlx::query("UPDATE instance_content SET enabled = ? WHERE id = ?")
        .bind(enabled)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn set_share_mode(db: &SqlitePool, id: &str, mode: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE instance_content SET share_mode = ? WHERE id = ?")
        .bind(mode)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}
