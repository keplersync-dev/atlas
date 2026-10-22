use serde::{Deserialize, Serialize};

pub use atlas_protocol::api::{
    AddMembers, CreateRevision, CreateSharedInstance, GroupLinkGroup, GroupRef, OptionalItem,
    Revision, RevisionChanges, RevisionSummary, SetMemberRole, SharedGroupLink, SharedInstance,
    SharedInstanceDetail, SharedMember, Synced, Transfer, UpdateSharedInstance, UpdatedItem,
};

use super::{ContentKind, ContentSource, Id, ManualDownload, ShareMode, Storage};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigConflict {
    pub path: String,
    pub mine: String,
    pub theirs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionChange {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherChange {
    pub minecraft_version: Option<VersionChange>,
    pub loader_version: Option<VersionChange>,
    /// true = Atlas edits the launcher's instance settings itself (Prism).
    pub automatic: bool,
}

/// Shown on the Review update page before syncing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreview {
    pub from_revision: u32,
    pub to_revision: u32,
    pub changes: RevisionChanges,
    pub download_bytes: u64,
    pub manual_downloads: Vec<ManualDownload>,
    pub overwritten_local_changes: Vec<String>,
    pub config_conflicts: Vec<ConfigConflict>,
    pub launcher_change: Option<LauncherChange>,
    pub game_running: bool,
    pub new_optional: Vec<OptionalItem>,
}

/// One file in the share/publish picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishItem {
    pub content_id: Id,
    pub name: String,
    pub icon_url: Option<String>,
    pub kind: ContentKind,
    pub source: ContentSource,
    pub path: String,
    pub size_bytes: u64,
    pub is_new: bool,
    pub share_mode: ShareMode,
    pub published_share_mode: Option<ShareMode>,
    pub needs_upload: bool,
    pub already_uploaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPlan {
    /// None on a first share.
    pub base_revision: Option<u32>,
    pub items: Vec<PublishItem>,
    pub removed: Vec<String>,
    pub manual_downloads: Vec<ManualDownload>,
    pub storage: Storage,
}
