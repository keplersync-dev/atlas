//! What the UI sends: command arguments that are objects.
//!
//! Optional fields (`x?: T`) are `#[serde(default)] Option<T>`. Fields that can be
//! missing *or* null with different meanings (`x?: T | null`) are `Option<Option<T>>`:
//! `None` = leave as is, `Some(None)` = clear it.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

pub use atlas_protocol::api::{NewReport, UpdateProfileInput};

use super::{ConfigChoice, Id, InviteRole, LauncherKind, ShareMode};

/// Lets `Option<Option<T>>` tell "field missing" (outer None) from "field is null" (Some(None)).
fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Identifies a detected instance to track.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInstanceRef {
    pub launcher_kind: LauncherKind,
    pub launcher_instance_id: String,
}

/// Atlas-only preferences. Name, versions and settings belong to the launcher.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstanceInput {
    #[serde(default)]
    pub is_favorite: Option<bool>,
    #[serde(default)]
    pub hide_activity: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOptions {
    /// Missing path = theirs.
    #[serde(default)]
    pub config_choices: HashMap<String, ConfigChoice>,
    /// Keys from SyncPreview.newOptional to download. Missing = don't.
    #[serde(default)]
    pub install_optional: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInstanceInput {
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    pub friend_ids: Vec<Id>,
    pub group_ids: Vec<Id>,
    pub role: InviteRole,
    /// Setting per PublishItem.contentId. Missing = keep the item's current shareMode.
    pub share_modes: HashMap<Id, ShareMode>,
    pub confirmed_upload_rights: bool,
    /// ConfigGroup keys not to share, saved for later publishes. Missing = keep the saved ones.
    #[serde(default)]
    pub config_exclude: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishInput {
    pub changelog: Option<String>,
    pub share_modes: HashMap<Id, ShareMode>,
    pub confirmed_upload_rights: bool,
    /// As in ShareInstanceInput.
    #[serde(default)]
    pub config_exclude: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGroupInviteInput {
    pub max_uses: Option<u32>,
    /// None = never expires.
    pub expires_in_hours: Option<u32>,
}

/// `updateSharedInstance` patch.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSharedInstanceInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub icon: Option<Option<String>>,
}

/// `inviteToSharedInstance` input.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteInput {
    pub friend_ids: Vec<Id>,
    pub group_ids: Vec<Id>,
    pub role: InviteRole,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGroupInput {
    pub name: String,
    #[serde(default, deserialize_with = "double_option")]
    pub icon: Option<Option<String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGroupInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub icon: Option<Option<String>>,
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
    fn missing_and_null_mean_different_things() {
        let p: UpdateSharedInstanceInput = serde_json::from_value(json!({ "name": "x" })).unwrap();
        assert_eq!(p.description, None, "missing = leave as is");
        let p: UpdateSharedInstanceInput =
            serde_json::from_value(json!({ "description": null })).unwrap();
        assert_eq!(p.description, Some(None), "null = clear it");
        let p: UpdateSharedInstanceInput =
            serde_json::from_value(json!({ "description": "hi" })).unwrap();
        assert_eq!(p.description, Some(Some("hi".into())));
    }

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
