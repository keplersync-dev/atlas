// Each enum mirrors a string union in types.ts. snake_case on the wire. The ones the
// server uses too live in atlas-protocol.

use atlas_protocol::wire_enum;

pub use atlas_protocol::enums::*;

wire_enum!(
    /// Launchers Atlas can read. `Folder` = a game folder added by hand.
    LauncherKind { Prism, Multimc, Modrinth, Curseforge, Official, Folder }
);

wire_enum!(
    /// `Syncing` is never stored: it comes from the job manager at response time.
    InstanceStatus { Ready, Syncing, Missing }
);

wire_enum!(
    /// How a file in an instance is shared. New files start `Required`.
    ShareMode { Required, Optional, Private }
);

wire_enum!(
    /// Derived, never stored.
    SyncState { LocalOnly, UpToDate, UpdateAvailable, LocalChanges, Conflict, Frozen }
);

wire_enum!(BackupReason {
    BeforeUpdate,
    BeforeRestore,
    BeforeRevert
});

wire_enum!(
    /// Work on an instance outside a job: see `Work`.
    WorkStage { Reading, Uploading, Saving }
);

wire_enum!(SyncStage {
    WaitingForGameExit,
    Backup,
    Download,
    WaitingForManualDownloads,
    Apply,
});

wire_enum!(
    /// SyncOptions.configChoices values.
    ConfigChoice { Mine, Theirs }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_snake_case_strings() {
        let s = |v: &dyn erased::Ser| v.json();
        assert_eq!(s(&SyncState::UpdateAvailable), "\"update_available\"");
        assert_eq!(
            s(&SyncStage::WaitingForGameExit),
            "\"waiting_for_game_exit\""
        );
        assert_eq!(s(&LauncherKind::Curseforge), "\"curseforge\"");
        assert_eq!(s(&Loader::Neoforge), "\"neoforge\"");
        assert_eq!(
            s(&ActivityType::InstanceSharedToGroup),
            "\"instance_shared_to_group\""
        );
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
