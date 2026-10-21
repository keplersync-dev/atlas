use serde::{Deserialize, Serialize};

// Each enum mirrors a string union in types.ts. snake_case on the wire.

macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

wire_enum!(Loader { Vanilla, Fabric, Quilt, Forge, Neoforge });

wire_enum!(
    /// Launchers Atlas can read. `Folder` = a game folder added by hand.
    LauncherKind { Prism, Multimc, Modrinth, Curseforge, Official, Folder }
);

wire_enum!(
    /// `Syncing` is never stored: it comes from the job manager at response time.
    InstanceStatus { Ready, Syncing, Missing }
);

wire_enum!(ContentKind { Mod, Resourcepack, Shaderpack, Datapack });
wire_enum!(ContentSource { Modrinth, Curseforge, Upload, Local });

wire_enum!(
    /// How a file in an instance is shared. New files start `Required`.
    ShareMode { Required, Optional, Private }
);

wire_enum!(Side { Client, Server, Both });
wire_enum!(MemberRole { Owner, Editor, Viewer });
wire_enum!(
    /// `Exclude<MemberRole, "owner">` in types.ts: roles you can give someone.
    InviteRole { Editor, Viewer }
);
wire_enum!(MemberStatus { Invited, Active, Declined });
wire_enum!(GroupRole { Owner, Admin, Member });
wire_enum!(
    /// `Exclude<GroupRole, "owner">` in types.ts.
    GroupMemberRole { Admin, Member }
);
wire_enum!(PresenceState { Offline, Online, Playing });
wire_enum!(AuthProvider { Minecraft, Discord });

wire_enum!(
    /// Derived, never stored (data-plan §7).
    SyncState { LocalOnly, UpToDate, UpdateAvailable, LocalChanges, Conflict, Frozen }
);

wire_enum!(RequestDirection { Incoming, Outgoing });
wire_enum!(BackupReason { BeforeUpdate, BeforeRestore, BeforeRevert });

wire_enum!(SyncStage {
    WaitingForGameExit,
    Backup,
    Download,
    WaitingForManualDownloads,
    Apply,
});

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

wire_enum!(
    /// SyncOptions.configChoices values.
    ConfigChoice { Mine, Theirs }
);

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
    fn values_are_snake_case_strings() {
        let s = |v: &dyn erased::Ser| v.json();
        assert_eq!(s(&SyncState::UpdateAvailable), "\"update_available\"");
        assert_eq!(s(&SyncStage::WaitingForGameExit), "\"waiting_for_game_exit\"");
        assert_eq!(s(&LauncherKind::Curseforge), "\"curseforge\"");
        assert_eq!(s(&Loader::Neoforge), "\"neoforge\"");
        assert_eq!(s(&ActivityType::InstanceSharedToGroup), "\"instance_shared_to_group\"");
    }

    // Tiny helper so the test can take different enum types in one closure.
    mod erased {
        pub trait Ser {
            fn json(&self) -> String;
        }
        impl<T: serde::Serialize> Ser for T {
            fn json(&self) -> String {
                serde_json::to_string(self).unwrap()
            }
        }
    }
}
