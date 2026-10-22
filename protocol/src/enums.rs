//! String enums used on the wire by both sides. snake_case values.

use serde::{Deserialize, Serialize};

/// A `Copy` enum that serializes as a snake_case string. Callers need `serde` as a dependency.
#[macro_export]
macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

wire_enum!(Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    Neoforge
});

wire_enum!(ContentKind {
    Mod,
    Resourcepack,
    Shaderpack,
    Datapack
});
wire_enum!(ContentSource {
    Modrinth,
    Curseforge,
    Upload,
    Local
});

wire_enum!(Side {
    Client,
    Server,
    Both
});
wire_enum!(MemberRole {
    Owner,
    Editor,
    Viewer
});
wire_enum!(
    /// A role you can give someone: any but owner.
    InviteRole { Editor, Viewer }
);
wire_enum!(MemberStatus {
    Invited,
    Active,
    Declined
});
wire_enum!(GroupRole {
    Owner,
    Admin,
    Member
});
wire_enum!(
    /// A group role you can give someone: any but owner.
    GroupMemberRole { Admin, Member }
);
wire_enum!(PresenceState {
    Offline,
    Online,
    Playing
});
wire_enum!(AuthProvider { Minecraft, Discord });

wire_enum!(RequestDirection { Incoming, Outgoing });

wire_enum!(ActivityType {
    FriendRequest,
    FriendAccepted,
    InstanceInvite,
    InstanceUpdated,
    InstanceSharedToGroup,
    MemberJoined,
    MemberLeft,
    OwnershipTransferred,
    InstanceFrozen,
    GroupJoined,
});

/// Which program a session belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Client {
    #[default]
    Atlas,
    Tandem,
}

/// An enum's wire string ("update_available"), for storing in TEXT columns.
pub fn to_wire<T: Serialize>(value: T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        other => panic!("not a string enum: {other:?}"),
    }
}

/// Parse a wire string from a TEXT column. None for values this build doesn't know.
pub fn from_wire<T: serde::de::DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(s.to_owned())).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_wire_strings() {
        assert_eq!(to_wire(Loader::Neoforge), "neoforge");
        assert_eq!(
            to_wire(ActivityType::InstanceSharedToGroup),
            "instance_shared_to_group"
        );
        assert_eq!(from_wire("discord"), Some(AuthProvider::Discord));
        assert_eq!(from_wire::<AuthProvider>("github"), None);
    }
}
