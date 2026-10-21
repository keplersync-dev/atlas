use serde::{Deserialize, Serialize};

use super::{GroupRole, Id, IsoDate, PublicUser};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMember {
    pub user: PublicUser,
    pub role: GroupRole,
    pub joined_at: IsoDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInvite {
    pub code: String,
    pub link: String,
    pub created_by: PublicUser,
    pub max_uses: Option<u32>,
    pub uses: u32,
    pub expires_at: Option<IsoDate>,
    pub created_at: IsoDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: Id,
    pub name: String,
    pub icon: Option<String>,
    pub my_role: GroupRole,
    pub members: Vec<GroupMember>,
    pub shared_instance_ids: Vec<Id>,
    pub created_at: IsoDate,
}

/// `GroupDetail extends Group` in types.ts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDetail {
    #[serde(flatten)]
    pub group: Group,
    /// Only filled for owner/admins.
    pub invites: Vec<GroupInvite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInvitePreview {
    pub code: String,
    pub group_id: Id,
    pub group_name: String,
    pub group_icon: Option<String>,
    pub member_count: u32,
    pub invited_by: Option<PublicUser>,
    pub already_member: bool,
}
