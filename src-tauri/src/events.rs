//! Live events for the UI. Mirrors the `AtlasEvent` union in types.ts.
//!
//! Everything goes out on one channel, `atlas://event`, which src/lib/api/tauri.ts
//! listens to. Most events mean "refetch"; `instance.progress` and `presence.changed`
//! carry the new data. See docs/tauri-plan.md §7 for who emits what, and the
//! required order of events at the end of a job.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use crate::dto::{ActivityItem, Id, InstanceStatus, Presence, SyncProgress};

pub const CHANNEL: &str = "atlas://event";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum AtlasEvent {
    #[serde(rename = "connection.changed")]
    ConnectionChanged { online: bool },
    #[serde(rename = "session.changed")]
    SessionChanged {},
    #[serde(rename = "instance.progress")]
    InstanceProgress {
        instance_id: Id,
        status: InstanceStatus,
        progress: Option<SyncProgress>,
    },
    #[serde(rename = "instance.running")]
    InstanceRunning { instance_id: Id, is_running: bool },
    #[serde(rename = "instance.changed")]
    InstanceChanged { instance_id: Id },
    /// A background job (update, accept, restore) failed. `message` is for the player.
    #[serde(rename = "instance.jobFailed")]
    InstanceJobFailed { instance_id: Id, message: String },
    #[serde(rename = "launchers.changed")]
    LaunchersChanged {},
    #[serde(rename = "presence.changed")]
    PresenceChanged { user_id: Id, presence: Presence },
    #[serde(rename = "friends.changed")]
    FriendsChanged {},
    #[serde(rename = "shared.changed")]
    SharedChanged { shared_instance_id: Id },
    #[serde(rename = "groups.changed")]
    GroupsChanged { group_id: Option<Id> },
    #[serde(rename = "activity.new")]
    ActivityNew { item: ActivityItem },
}

/// Send an event to the UI. A closed window isn't an error worth handling.
pub fn emit<R: Runtime>(app: &AppHandle<R>, event: AtlasEvent) {
    if let Err(e) = app.emit(CHANNEL, &event) {
        tracing::warn!(error = ?e, "couldn't emit event");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{SyncProgress, SyncStage};
    use serde_json::json;

    fn to_json(e: AtlasEvent) -> serde_json::Value {
        serde_json::to_value(e).unwrap()
    }

    #[test]
    fn matches_the_atlas_event_union_in_types_ts() {
        assert_eq!(
            to_json(AtlasEvent::InstanceProgress {
                instance_id: "i1".into(),
                status: InstanceStatus::Syncing,
                progress: Some(SyncProgress {
                    stage: SyncStage::Backup,
                    percent: 10,
                    bytes_done: 1,
                    bytes_total: 10,
                    manual_downloads: vec![],
                }),
            }),
            json!({
                "type": "instance.progress",
                "instanceId": "i1",
                "status": "syncing",
                "progress": { "stage": "backup", "percent": 10, "bytesDone": 1, "bytesTotal": 10, "manualDownloads": [] }
            })
        );
        assert_eq!(
            to_json(AtlasEvent::InstanceProgress {
                instance_id: "i1".into(),
                status: InstanceStatus::Ready,
                progress: None,
            }),
            json!({ "type": "instance.progress", "instanceId": "i1", "status": "ready", "progress": null })
        );
        assert_eq!(
            to_json(AtlasEvent::InstanceRunning { instance_id: "i1".into(), is_running: true }),
            json!({ "type": "instance.running", "instanceId": "i1", "isRunning": true })
        );
        assert_eq!(
            to_json(AtlasEvent::SessionChanged {}),
            json!({ "type": "session.changed" })
        );
        assert_eq!(
            to_json(AtlasEvent::GroupsChanged { group_id: None }),
            json!({ "type": "groups.changed", "groupId": null })
        );
        assert_eq!(
            to_json(AtlasEvent::SharedChanged { shared_instance_id: "s1".into() }),
            json!({ "type": "shared.changed", "sharedInstanceId": "s1" })
        );
    }
}
