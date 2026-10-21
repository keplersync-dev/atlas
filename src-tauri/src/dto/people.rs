use serde::{Deserialize, Serialize};

use super::{AuthProvider, Id, IsoDate, PresenceState, RequestDirection};

/// Any Atlas user as seen by others.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: Id,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub minecraft_uuid: Option<String>,
    pub minecraft_username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedIdentity {
    pub provider: AuthProvider,
    pub username: String,
    pub linked_at: IsoDate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub used_bytes: u64,
    pub quota_bytes: u64,
    pub max_file_bytes: u64,
}

/// The signed-in Atlas user, with private fields. `extends PublicUser` in types.ts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    #[serde(flatten)]
    pub user: PublicUser,
    pub friend_code: String,
    pub friend_link: String,
    pub identities: Vec<LinkedIdentity>,
    pub appear_offline: bool,
    pub username_change_available_at: Option<IsoDate>,
    pub storage: Storage,
}

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
