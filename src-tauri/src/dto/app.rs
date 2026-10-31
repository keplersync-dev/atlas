use serde::{Deserialize, Serialize};

use super::Id;

/// How Atlas behaves on this PC, outside any account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Notices when something needs the player (see `Notice`).
    pub notifications: bool,
    /// The quiet sound that goes with a notice.
    pub notification_sound: bool,
    /// The close button hides Atlas to the tray instead of quitting it.
    pub close_to_tray: bool,
    /// Atlas starts in the tray when you sign in to Windows.
    pub start_with_windows: bool,
    /// Where the accept dialog's "Keep it up to date automatically" starts.
    pub auto_update: bool,
    /// The "start with Windows" screen after the first accepted pack has been shown.
    pub start_with_windows_offered: bool,
}

/// Only the fields given change.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub notifications: Option<bool>,
    pub notification_sound: Option<bool>,
    pub close_to_tray: Option<bool>,
    pub start_with_windows: Option<bool>,
    pub auto_update: Option<bool>,
    pub start_with_windows_offered: Option<bool>,
}

/// Something that needs the player: a pack shared with them, a friend request, or an
/// update that can't go ahead without them. Shown inside Atlas when its window is
/// visible, else as a Windows notification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub title: String,
    pub body: String,
    /// Where its button goes.
    pub target: NoticeTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum NoticeTarget {
    /// The pack's page.
    Pack {
        instance_id: Id,
    },
    /// The update review for the pack.
    Review {
        instance_id: Id,
    },
    /// The accept dialog for a pack shared with the player.
    Invite {
        shared_instance_id: Id,
    },
    Friends,
    /// Several notices held while a game ran.
    Home,
}

/// A newer Atlas on the release page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub version: String,
    /// The release notes.
    pub notes: Option<String>,
}
