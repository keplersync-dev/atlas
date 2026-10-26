use serde::{Deserialize, Serialize};

pub use atlas_protocol::api::{
    AddMembers, CreateRevision, CreateSharedInstance, GroupLinkGroup, GroupRef, OptionalItem,
    Revision, RevisionChanges, RevisionSummary, SetMemberRole, ShareToGroup, SharedGroupLink,
    SharedInstance, SharedInstanceDetail, SharedMember, Synced, Transfer, UpdateSharedInstance,
    UpdatedItem,
};

use super::{ContentKind, ContentSource, Id, LeftOutReason, ManualDownload, ShareMode, Storage};

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
    /// Files this update brings that aren't from Modrinth or CurseForge: someone uploaded
    /// them, and nobody has checked what's inside.
    pub unverified: Vec<String>,
}

/// A config folder directly under config/ (usually one per mod) or a loose config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGroup {
    /// "config/create/" (a folder) or "config/a.json" (a file).
    pub key: String,
    pub files: u32,
    pub size_bytes: u64,
    pub shared: bool,
}

/// A config file that won't be shared as it is here, and why. Listed in the dialog
/// before anything uploads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftOutFile {
    pub path: String,
    pub reason: LeftOutReason,
}

/// The modpack a share is based on: its own files come from its download.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInfo {
    pub name: String,
    pub version_label: Option<String>,
    pub source: ContentSource,
    /// Files shared from it rather than uploaded.
    pub files: u32,
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
    /// Files Modrinth or CurseForge couldn't be asked about (unreachable). They count as
    /// uploads.
    pub unchecked: u32,
    /// Config folders and loose config files, for choosing which are shared.
    pub configs: Vec<ConfigGroup>,
    /// The modpack the instance is from, when its files can come from its download.
    pub pack: Option<PackInfo>,
    /// Config files that won't be shared as they are here.
    pub left_out: Vec<LeftOutFile>,
    /// The launcher runs other Minecraft or loader versions than the shared ones: `from`
    /// is shared now, `to` is what this version will say. `automatic` is unused here.
    pub versions: Option<LauncherChange>,
}
