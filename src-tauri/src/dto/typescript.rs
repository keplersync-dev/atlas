//! src/lib/types.ts, the UI's copy of these types, is written from them. This test fails
//! when it's out of date; `ATLAS_WRITE_TYPES=1 cargo test types_ts` writes it again.

use std::path::Path;

use ts_rs::{Config, TS};

use super::*;
use crate::events::AtlasEvent;

const HEADER: &str = "\
// Written from the Rust types (atlas-protocol, src-tauri/src/dto and src-tauri/src/events.rs)
// by src-tauri/src/dto/typescript.rs. Don't edit it: change the Rust side, then run
// `ATLAS_WRITE_TYPES=1 cargo test --manifest-path src-tauri/Cargo.toml types_ts`.

/** UUIDv7 string. */
export type Id = string;
/** ISO-8601 UTC string, e.g. \"2026-10-19T16:39:00.000Z\". */
export type IsoDate = string;
";

/// The declaration of each type, with its doc comment, in this order.
macro_rules! declare {
    ($cfg:expr; $($t:ty),* $(,)?) => {
        [$((<$t as TS>::docs(), <$t as TS>::decl($cfg))),*]
    };
}

fn types_ts() -> String {
    // Sizes and counts stay far below 2^53, and the UI does arithmetic on them.
    let cfg = Config::new().with_large_int("number");
    let decls = declare![&cfg;
        // Enums
        Loader, LauncherKind, InstanceStatus, ContentKind, ContentSource, ShareMode, Side,
        MemberRole, InviteRole, MemberStatus, GroupRole, PresenceState, AuthProvider,
        AvatarSource, RequestDirection, SyncState, SyncStage, WorkStage, BackupReason,
        LeftOutReason, ConfigChoice, ActivityType, ReportTarget, ReportReason, LinkKind,
        // App and people
        AppSettings, AppSettingsPatch, AvailableUpdate, OpenedLink, NewReport, PublicUser,
        LinkedIdentity, Storage, Me, Session, Presence, Friend, FriendRequest, MyUpload,
        RemovedUploads, UpdateProfileInput,
        // Groups
        GroupMember, GroupInvite, Group, GroupDetail, GroupInvitePreview, CreateGroup,
        CreateGroupInvite,
        // Launchers and instances
        Launcher, DetectedInstance, ManualDownload, Work, SyncProgress, InstanceSyncInfo,
        LauncherRef, InstanceSummary, InstanceContent, Backup, InstanceDetail,
        TrackInstanceRef, UpdateInstanceInput,
        // Shared instances
        GroupRef, SharedMember, GroupLinkGroup, SharedGroupLink, OptionalItem, UploadedFile, SharedInstance,
        UpdatedItem, RevisionChanges, RevisionSummary, SharedInstanceDetail, ConfigConflict,
        VersionChange, LauncherChange, SyncPreview, SyncOptions, PublishItem, PackInfo,
        LeftOutFile, ShareFile, FileRules, PublishPlan, ShareInstanceInput, PublishInput,
        InviteInput,
        // Activity and events
        ActivityItem, Notice, NoticeTarget, AtlasEvent,
    ];
    let mut out = HEADER.to_owned();
    for (docs, decl) in decls {
        out.push('\n');
        out.push_str(&docs.unwrap_or_default());
        out.push_str("export ");
        out.push_str(&decl);
        out.push('\n');
    }
    out
}

#[test]
fn types_ts_is_up_to_date() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/types.ts");
    let written = types_ts();
    if std::env::var_os("ATLAS_WRITE_TYPES").is_some() {
        std::fs::write(&file, &written).unwrap();
        return;
    }
    let now = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        now == written,
        "src/lib/types.ts doesn't match the Rust types: run \
         `ATLAS_WRITE_TYPES=1 cargo test --manifest-path src-tauri/Cargo.toml types_ts`"
    );
}
