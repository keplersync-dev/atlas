//! Live events for the UI. Mirrors the `AtlasEvent` union in types.ts.
//!
//! Everything goes out on one channel, `atlas://event`, which src/lib/api/tauri.ts
//! listens to. Most events mean "refetch"; `instance.progress` and `presence.changed`
//! carry the new data. jobs.rs documents the order of events at the end of a job.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use crate::dto::{ActivityItem, Id, InstanceStatus, Presence, SyncProgress, Work};

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
    /// Work outside a job (reading files, uploading, saving a version). `work: None` = done.
    #[serde(rename = "instance.work")]
    InstanceWork { instance_id: Id, work: Option<Work> },
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
    /// Atlas was opened with a friend or group link: the UI takes it with
    /// `take_opened_link`.
    #[serde(rename = "link.opened")]
    LinkOpened {},
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
            to_json(AtlasEvent::InstanceRunning {
                instance_id: "i1".into(),
                is_running: true
            }),
            json!({ "type": "instance.running", "instanceId": "i1", "isRunning": true })
        );
        assert_eq!(
            to_json(AtlasEvent::InstanceWork {
                instance_id: "i1".into(),
                work: Some(crate::dto::Work {
                    stage: crate::dto::WorkStage::Reading,
                    done: 3,
                    total: 300
                })
            }),
            json!({ "type": "instance.work", "instanceId": "i1",
                    "work": { "stage": "reading", "done": 3, "total": 300 } })
        );
        assert_eq!(
            to_json(AtlasEvent::InstanceWork {
                instance_id: "i1".into(),
                work: None
            }),
            json!({ "type": "instance.work", "instanceId": "i1", "work": null })
        );
        assert_eq!(
            to_json(AtlasEvent::SessionChanged {}),
            json!({ "type": "session.changed" })
        );
        assert_eq!(
            to_json(AtlasEvent::LinkOpened {}),
            json!({ "type": "link.opened" })
        );
        assert_eq!(
            to_json(AtlasEvent::GroupsChanged { group_id: None }),
            json!({ "type": "groups.changed", "groupId": null })
        );
        assert_eq!(
            to_json(AtlasEvent::SharedChanged {
                shared_instance_id: "s1".into()
            }),
            json!({ "type": "shared.changed", "sharedInstanceId": "s1" })
        );
    }
}
