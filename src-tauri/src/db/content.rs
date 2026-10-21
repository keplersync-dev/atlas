//! `instance_content` table: the file index of each tracked instance.
//! T1 only reads it; the scanner (T2) writes it.

use sqlx::{FromRow, SqlitePool};

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
