use serde::{Deserialize, Serialize};

pub use atlas_protocol::api::{LinkedIdentity, Me, PublicUser, Storage};

use super::{Id, IsoDate, PresenceState, RequestDirection};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// None = not signed in (tracking still works; friends/sharing don't).
    pub user: Option<Me>,
    /// false = server unreachable; social data comes from the offline cache.
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub state: PresenceState,
    pub instance_name: Option<String>,
    pub minecraft_version: Option<String>,
    pub shared_instance_id: Option<Id>,
    pub updated_at: IsoDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub user: PublicUser,
    pub friends_since: IsoDate,
    pub presence: Presence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequest {
    pub direction: RequestDirection,
    pub user: PublicUser,
    pub created_at: IsoDate,
}
