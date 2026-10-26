use serde::{Deserialize, Serialize};

pub use atlas_protocol::api::{
    Friend, FriendRequest, LinkedIdentity, Me, Presence, PublicUser, Storage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// None = not signed in (tracking still works; friends/sharing don't).
    pub user: Option<Me>,
    /// false = server unreachable; social data comes from the offline cache.
    pub online: bool,
    /// false = this copy of Atlas has no server to sign in to.
    pub server: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    Friend,
    Group,
}

/// A friend or group link Atlas was opened with (`atlas://friend/<code>`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedLink {
    pub kind: LinkKind,
    /// Normalized: a friend code or a group invite code.
    pub code: String,
}
