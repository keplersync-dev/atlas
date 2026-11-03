//! What the UI sends: command arguments that are objects.
//!
//! Optional fields (`x?: T`) are `#[serde(default)] Option<T>`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use atlas_protocol::api::{NewReport, UpdateProfileInput};

use super::{ConfigChoice, FileRules, Id, InviteRole, LauncherKind, ShareMode};

/// Identifies a detected instance to track.
#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInstanceRef {
    pub launcher_kind: LauncherKind,
    pub launcher_instance_id: String,
}

/// Atlas-only preferences. Name, versions and settings belong to the launcher.
#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, ts(optional_fields))]
pub struct UpdateInstanceInput {
    #[serde(default)]
    pub is_favorite: Option<bool>,
    #[serde(default)]
    pub hide_activity: Option<bool>,
    #[serde(default)]
    pub auto_update: Option<bool>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOptions {
    /// Missing path = theirs.
    #[serde(default)]
    #[cfg_attr(test, ts(as = "Option<_>", optional))]
    pub config_choices: HashMap<String, ConfigChoice>,
    /// Keys from SyncPreview.newOptional to download. Missing = don't.
    #[serde(default)]
    #[cfg_attr(test, ts(as = "Option<_>", optional))]
    pub install_optional: Vec<String>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInstanceInput {
    #[serde(default)]
    #[cfg_attr(test, ts(optional = nullable))]
    pub description: Option<String>,
    pub friend_ids: Vec<Id>,
    pub group_ids: Vec<Id>,
    pub role: InviteRole,
    /// Setting per PublishItem.contentId. Missing = keep the item's current shareMode.
    pub share_modes: HashMap<Id, ShareMode>,
    pub confirmed_upload_rights: bool,
    /// The tree's ticks, saved for later publishes. Missing = keep the saved ones.
    #[serde(default)]
    #[cfg_attr(test, ts(optional))]
    pub files: Option<FileRules>,
}

#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishInput {
    pub changelog: Option<String>,
    pub share_modes: HashMap<Id, ShareMode>,
    pub confirmed_upload_rights: bool,
    /// As in ShareInstanceInput.
    #[serde(default)]
    #[cfg_attr(test, ts(optional))]
    pub files: Option<FileRules>,
}

/// `inviteToSharedInstance` input.
#[cfg_attr(test, derive(ts_rs::TS))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteInput {
    pub friend_ids: Vec<Id>,
    pub group_ids: Vec<Id>,
    pub role: InviteRole,
}

/// `markActivityRead(ids: Id[] | "all")`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ActivityIds {
    All(AllLiteral),
    Some(Vec<Id>),
}

/// Matches the string "all" and nothing else.
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum AllLiteral {
    #[serde(rename = "all")]
    All,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn activity_ids_take_all_or_a_list() {
        assert!(matches!(
            serde_json::from_value::<ActivityIds>(json!("all")).unwrap(),
            ActivityIds::All(_)
        ));
        assert!(matches!(
            serde_json::from_value::<ActivityIds>(json!(["a", "b"])).unwrap(),
            ActivityIds::Some(v) if v.len() == 2
        ));
        assert!(serde_json::from_value::<ActivityIds>(json!("some")).is_err());
    }

    #[test]
    fn sync_options_default_when_empty() {
        let o: SyncOptions = serde_json::from_value(json!({})).unwrap();
        assert!(o.config_choices.is_empty() && o.install_optional.is_empty());
    }
}
