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
}
