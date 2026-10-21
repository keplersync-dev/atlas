use serde::{Deserialize, Serialize};

use super::{
    ContentKind, ContentSource, Id, InviteRole, IsoDate, Loader, ManualDownload, MemberRole,
    MemberStatus, PublicUser, ShareMode, Storage,
};

/// `{ id, name }` — how a group is named on a member row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRef {
    pub id: Id,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMember {
    pub user: PublicUser,
    pub role: MemberRole,
    pub status: MemberStatus,
    pub via_group: Option<GroupRef>,
    pub last_synced_revision: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupLinkGroup {
    pub id: Id,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedGroupLink {
    pub group: GroupLinkGroup,
    pub role: InviteRole,
}

/// An optional file a friend can choose to download.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalItem {
    /// Provider project id, or the file's sha256 for uploads (tauri-plan §11.1).
    pub key: String,
    pub name: String,
    pub kind: ContentKind,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedInstance {
    pub id: Id,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub owner: Option<PublicUser>,
    pub minecraft_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub latest_revision: u32,
    pub content_count: u32,
    pub size_bytes: u64,
    pub updated_at: IsoDate,
    pub frozen: bool,
    pub my_role: MemberRole,
    pub my_status: MemberStatus,
    pub my_via_group: Option<GroupRef>,
    pub invited_by: Option<PublicUser>,
    pub invited_at: Option<IsoDate>,
    pub members: Vec<SharedMember>,
    pub groups: Vec<SharedGroupLink>,
    /// Filled in by Rust from the local instance table, not by the server.
    pub local_instance_id: Option<Id>,
    pub optional_items: Vec<OptionalItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedItem {
    pub name: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<UpdatedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionSummary {
    pub revision: u32,
    pub changelog: Option<String>,
    pub created_by: Option<PublicUser>,
    pub created_at: IsoDate,
    pub changes: RevisionChanges,
}

/// `SharedInstanceDetail extends SharedInstance` in types.ts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedInstanceDetail {
    #[serde(flatten)]
    pub shared: SharedInstance,
    /// Newest first.
    pub revisions: Vec<RevisionSummary>,
}

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
