//! WebSocket messages (`GET /v1/ws`): `{ "type": "…", "data": { … } }`. REST stays the
//! source of truth; most server messages only say "something changed, refetch".

use serde::{Deserialize, Serialize};

use crate::Id;
use crate::api::{ActivityItem, Presence};
use crate::enums::PresenceState;

/// Seconds between client pings. The server closes a socket that's quiet for 3× this.
pub const HEARTBEAT_SECONDS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    /// On connect and whenever the game starts or stops.
    #[serde(rename = "presence.set")]
    PresenceSet(PresenceUpdate),
    #[serde(rename = "ping")]
    Ping {},
}

/// What a client says it's doing. The server decides what each friend gets to see.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceUpdate {
    /// `online` or `playing`; `offline` is treated as `online` while connected.
    pub state: PresenceState,
    /// Left out (None) when the player hides what they're playing.
    #[serde(default)]
    pub instance_name: Option<String>,
    #[serde(default)]
    pub minecraft_version: Option<String>,
    #[serde(default)]
    pub shared_instance_id: Option<Id>,
    /// Minecraft is open (Tandem runs inside it), not just the Atlas app.
    #[serde(default)]
    pub in_minecraft: bool,
}

impl PresenceUpdate {
    pub fn online() -> Self {
        Self {
            state: PresenceState::Online,
            instance_name: None,
            minecraft_version: None,
            shared_instance_id: None,
            in_minecraft: false,
        }
    }
}

/// Messages from the server. Read them with [`ServerMessage::parse`], which turns types
/// this build doesn't know into `Unknown`, so the server can add new ones without breaking
/// older apps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    #[serde(rename = "hello", rename_all = "camelCase")]
    Hello { heartbeat_seconds: u64 },
    #[serde(rename = "presence.changed", rename_all = "camelCase")]
    PresenceChanged { user_id: Id, presence: Presence },
    #[serde(rename = "friends.changed")]
    FriendsChanged {},
    /// Your own account changed somewhere else: storage used (an upload, or old files
    /// cleaned up), or a profile change from another PC. Fetch `/me` again.
    #[serde(rename = "me.changed")]
    MeChanged {},
    #[serde(rename = "shared.changed", rename_all = "camelCase")]
    SharedChanged {
        shared_instance_id: Id,
        #[serde(default)]
        latest_revision: Option<u32>,
    },
    #[serde(rename = "groups.changed", rename_all = "camelCase")]
    GroupsChanged { group_id: Id },
    /// A synced world changed: its lease, head, branches or members. Fetch it again.
    #[serde(rename = "world.changed", rename_all = "camelCase")]
    WorldChanged { world_id: Id },
    /// Someone invited you to their game (Tandem's Just play). Fetch `/play/invites`.
    #[serde(rename = "invite.received", rename_all = "camelCase")]
    InviteReceived { session_id: Id },
    /// Your invites changed: one was withdrawn, or its game ended. Fetch `/play/invites` again.
    #[serde(rename = "invites.changed")]
    InvitesChanged {},
    /// The host of a synced world is handing it to `to_id`: they take over, and everyone else
    /// joins them once they're hosting.
    #[serde(rename = "world.handoff", rename_all = "camelCase")]
    WorldHandoff {
        world_id: Id,
        from_id: Id,
        to_id: Id,
    },
    /// A member of a synced world whose download is slow asks you to host it, so they can join you.
    #[serde(rename = "host.requested", rename_all = "camelCase")]
    HostRequested { world_id: Id, from_id: Id },
    /// The friend you asked to host a synced world can't right now.
    #[serde(rename = "host.declined", rename_all = "camelCase")]
    HostDeclined { world_id: Id, by_id: Id },
    /// To the host of a game: the world's owner removed `user_id` from it. Kick them.
    #[serde(rename = "play.kick", rename_all = "camelCase")]
    PlayKick { session_id: Id, user_id: Id },
    #[serde(rename = "notification.new")]
    NotificationNew { item: Box<ActivityItem> },
    #[serde(rename = "session.revoked")]
    SessionRevoked {},
    #[serde(rename = "pong")]
    Pong {},
    /// A message this build doesn't understand. Never sent.
    #[serde(skip)]
    Unknown,
}

impl ServerMessage {
    pub fn parse(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or(Self::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn messages_use_type_and_data() {
        let msg = ClientMessage::PresenceSet(PresenceUpdate::online());
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({ "type": "presence.set", "data": {
                "state": "online", "instanceName": null, "minecraftVersion": null,
                "sharedInstanceId": null, "inMinecraft": false } })
        );
        assert_eq!(
            serde_json::to_value(ClientMessage::Ping {}).unwrap(),
            json!({ "type": "ping", "data": {} })
        );
        let hello: ServerMessage =
            serde_json::from_value(json!({ "type": "hello", "data": { "heartbeatSeconds": 30 } }))
                .unwrap();
        assert_eq!(
            hello,
            ServerMessage::Hello {
                heartbeat_seconds: 30
            }
        );
        assert_eq!(
            serde_json::to_value(ServerMessage::FriendsChanged {}).unwrap(),
            json!({ "type": "friends.changed", "data": {} })
        );
    }

    #[test]
    fn unknown_server_messages_are_tolerated() {
        for v in [
            json!({ "type": "world.joined", "data": { "worldId": "w1" } }),
            json!({ "type": "world.left" }),
            json!("not even an object"),
        ] {
            assert_eq!(ServerMessage::parse(&v.to_string()), ServerMessage::Unknown);
        }
        assert_eq!(
            ServerMessage::parse(r#"{"type":"pong","data":{}}"#),
            ServerMessage::Pong {}
        );
    }
}
