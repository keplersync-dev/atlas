use serde::{Deserialize, Serialize};

use super::{ActivityType, Id, IsoDate, PublicUser};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: Id,
    #[serde(rename = "type")]
    pub kind: ActivityType,
    /// None if the actor deleted their account.
    pub actor: Option<PublicUser>,
    pub created_at: IsoDate,
    pub read_at: Option<IsoDate>,
    pub shared_instance_id: Option<Id>,
    pub shared_instance_name: Option<String>,
    pub group_id: Option<Id>,
    pub group_name: Option<String>,
    pub revision: Option<u32>,
}
