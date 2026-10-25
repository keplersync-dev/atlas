use serde::{Deserialize, Serialize};

/// How Atlas behaves on this PC, outside any account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// A Windows notification for new activity while Atlas isn't in front.
    pub notifications: bool,
    /// The close button hides Atlas to the tray instead of quitting it.
    pub close_to_tray: bool,
    /// Atlas starts in the tray when you sign in to Windows.
    pub start_with_windows: bool,
}

/// Only the fields given change.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsPatch {
    pub notifications: Option<bool>,
    pub close_to_tray: Option<bool>,
    pub start_with_windows: Option<bool>,
}

/// A newer Atlas on the release page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub version: String,
    /// The release notes.
    pub notes: Option<String>,
}
