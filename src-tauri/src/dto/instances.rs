use serde::{Deserialize, Serialize};

use super::{
    BackupReason, ContentKind, ContentSource, Id, InstanceStatus, IsoDate, LauncherKind, Loader,
    MemberRole, PublicUser, ShareMode, Side, SyncStage, SyncState,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Launcher {
    pub id: Id,
    /// Never `Folder` here.
    pub kind: LauncherKind,
    pub name: String,
    pub root_path: String,
    pub instance_count: u32,
    /// The loaders Atlas can set up in a new instance there; empty when it can't create any.
    pub creatable_loaders: Vec<Loader>,
    pub added_by_user: bool,
}

/// An instance found in a launcher, before or after tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedInstance {
    pub launcher_kind: LauncherKind,
    pub launcher_instance_id: String,
    pub name: String,
    pub game_dir: String,
    pub minecraft_version: Option<String>,
    pub loader: Option<Loader>,
    pub loader_version: Option<String>,
    /// Set when it's already tracked.
    pub instance_id: Option<Id>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDownload {
    pub content_name: String,
    pub file_name: String,
    pub page_url: String,
}

/// Progress of an update/restore Atlas is applying to a folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub stage: SyncStage,
    /// 0–100
    pub percent: u8,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub manual_downloads: Vec<ManualDownload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSyncInfo {
    pub shared_instance_id: Id,
    pub role: MemberRole,
    pub owner: Option<PublicUser>,
    pub synced_revision: u32,
    pub latest_revision: u32,
    /// Never `LocalOnly` here (that's `sync: null`).
    pub state: SyncState,
    pub local_change_count: u32,
}

/// `{ kind: LauncherKind; name: string }` in InstanceSummary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherRef {
    pub kind: LauncherKind,
    pub name: String,
}

/// What the sidebar, tiles and lists need. Cheap to build for every instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSummary {
    pub id: Id,
    pub name: String,
    pub icon: Option<String>,
    pub launcher: LauncherRef,
    pub game_dir: String,
    pub minecraft_version: Option<String>,
    pub loader: Option<Loader>,
    pub loader_version: Option<String>,
    pub status: InstanceStatus,
    pub is_favorite: bool,
    pub hide_activity: bool,
    pub content_count: u32,
    pub last_played_at: Option<IsoDate>,
    pub total_playtime_seconds: Option<u64>,
    pub is_running: bool,
    pub tracked_at: IsoDate,
    pub last_scanned_at: Option<IsoDate>,
    pub sync: Option<InstanceSyncInfo>,
    pub progress: Option<SyncProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceContent {
    pub id: Id,
    pub kind: ContentKind,
    pub source: ContentSource,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub name: String,
    pub version_label: Option<String>,
    pub author: Option<String>,
    pub icon_url: Option<String>,
    /// Relative to the game folder, without ".disabled".
    pub path: String,
    pub size_bytes: u64,
    pub side: Side,
    pub enabled: bool,
    pub is_managed: bool,
    pub share_mode: ShareMode,
    pub published_share_mode: Option<ShareMode>,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub id: Id,
    pub reason: BackupReason,
    pub from_revision: Option<u32>,
    pub to_revision: Option<u32>,
    pub file_count: u32,
    pub size_bytes: u64,
    pub created_at: IsoDate,
}

/// `InstanceDetail extends InstanceSummary` in types.ts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDetail {
    #[serde(flatten)]
    pub summary: InstanceSummary,
    pub content: Vec<InstanceContent>,
    /// Newest first.
    pub backups: Vec<Backup>,
}
