// Shapes the frontend receives from the API layer (src/lib/api).
// Atlas is a sync companion: launchers own instances; Atlas tracks their folders and keeps them in sync.
// These are app DTOs, not database rows or server responses: the Rust side assembles them from
// SQLite + the Atlas server and serialises with #[serde(rename_all = "camelCase")].

/** UUIDv7 string. */
export type Id = string;
/** ISO-8601 UTC string, e.g. "2026-10-19T16:39:00.000Z". */
export type IsoDate = string;

// ── Enums ──────────────────────────────────────────────────────────────

export type Loader = "vanilla" | "fabric" | "quilt" | "forge" | "neoforge";
/** Launchers Atlas can read. "folder" = a game folder added by hand, no launcher metadata. */
export type LauncherKind =
  "prism" | "multimc" | "modrinth" | "curseforge" | "official" | "folder";
/**
 * - ready:   folder found and readable
 * - syncing: Atlas is backing up / downloading / writing files
 * - missing: folder is gone (deleted or moved in the launcher)
 */
export type InstanceStatus = "ready" | "syncing" | "missing";
export type ContentKind = "mod" | "resourcepack" | "shaderpack" | "datapack";
/**
 * - modrinth / curseforge: identified on that site by file hash
 * - upload: hosted on Atlas servers (custom file in a shared instance)
 * - local: unknown file that exists only on this PC
 */
export type ContentSource = "modrinth" | "curseforge" | "upload" | "local";
/**
 * How a file in an instance is shared (set by the owner/editors, per file):
 * - required: everyone who has the instance gets it
 * - optional: friends see it, unticked, and choose whether to download it
 * - private:  "Only me" — stays on this PC, never shared or shown to friends
 * New files start as "required".
 */
export type ShareMode = "required" | "optional" | "private";
export type Side = "client" | "server" | "both";
export type MemberRole = "owner" | "editor" | "viewer";
export type MemberStatus = "invited" | "active" | "declined";
export type GroupRole = "owner" | "admin" | "member";
export type PresenceState = "offline" | "online" | "playing";
export type AuthProvider = "minecraft" | "discord";

/**
 * Derived, never stored:
 * - local_only:       not linked to a shared instance
 * - up_to_date:       on the latest revision, no local edits
 * - update_available: newer revision exists, no local edits
 * - local_changes:    on the latest revision, but managed content differs from it
 * - conflict:         newer revision exists AND managed content differs
 * - frozen:           owner is gone; no more updates will come
 */
export type SyncState =
  | "local_only"
  | "up_to_date"
  | "update_available"
  | "local_changes"
  | "conflict"
  | "frozen";

// ── People ─────────────────────────────────────────────────────────────

/** Any Atlas user as seen by others. */
/** A friend or group link Atlas was opened with (`atlas://friend/<code>`). */
export interface OpenedLink {
  kind: "friend" | "group";
  /** Normalized: a friend code or a group invite code. */
  code: string;
}

export interface PublicUser {
  id: Id;
  username: string;
  displayName: string;
  /** Discord avatar if linked. Otherwise render the Minecraft head from minecraftUuid, else initials. */
  avatarUrl: string | null;
  minecraftUuid: string | null;
  minecraftUsername: string | null;
}

export interface LinkedIdentity {
  provider: AuthProvider;
  /** Minecraft name or Discord username at last login. */
  username: string;
  linkedAt: IsoDate;
}

/** The signed-in Atlas user, with private fields. */
export interface Me extends PublicUser {
  /** "7KQ29XMB" — show as "7KQ2-9XMB". */
  friendCode: string;
  /** e.g. "https://atlas.example/f/7KQ29XMB" */
  friendLink: string;
  identities: LinkedIdentity[];
  appearOffline: boolean;
  /** When the username can next be changed (null = now). */
  usernameChangeAvailableAt: IsoDate | null;
  storage: { usedBytes: number; quotaBytes: number; maxFileBytes: number };
}

export interface Session {
  /** null = not signed in to Atlas (tracking still works; friends/sharing don't). */
  user: Me | null;
  /** false = server unreachable; social data comes from the offline cache. */
  online: boolean;
}

export interface Presence {
  state: PresenceState;
  /** Set when playing, unless the friend hid it ("Playing Minecraft"). */
  instanceName: string | null;
  minecraftVersion: string | null;
  /** Set when playing a shared instance you also have access to. */
  sharedInstanceId: Id | null;
  /** When offline, this is "last seen". */
  updatedAt: IsoDate;
}

export interface Friend {
  user: PublicUser;
  friendsSince: IsoDate;
  presence: Presence;
}

export interface FriendRequest {
  direction: "incoming" | "outgoing";
  user: PublicUser;
  createdAt: IsoDate;
}

// ── Groups ─────────────────────────────────────────────────────────────

export interface GroupMember {
  user: PublicUser;
  role: GroupRole;
  joinedAt: IsoDate;
}

export interface GroupInvite {
  code: string;
  /** e.g. "https://atlas.example/g/K3M9PQ2XRT" */
  link: string;
  /** null if they deleted their account. */
  createdBy: PublicUser | null;
  maxUses: number | null;
  uses: number;
  expiresAt: IsoDate | null;
  createdAt: IsoDate;
}

export interface Group {
  id: Id;
  name: string;
  icon: string | null;
  myRole: GroupRole;
  members: GroupMember[];
  /** Shared instances available to this group. */
  sharedInstanceIds: Id[];
  createdAt: IsoDate;
}

export interface GroupDetail extends Group {
  /** Only filled for owner/admins. */
  invites: GroupInvite[];
}

/** What someone sees after opening a group invite link, before joining. */
export interface GroupInvitePreview {
  code: string;
  groupId: Id;
  groupName: string;
  groupIcon: string | null;
  memberCount: number;
  invitedBy: PublicUser | null;
  alreadyMember: boolean;
}

// ── Launchers & instances ──────────────────────────────────────────────

export interface Launcher {
  id: Id;
  kind: Exclude<LauncherKind, "folder">;
  /** "Prism Launcher", "Modrinth App", ... */
  name: string;
  rootPath: string;
  /** Instances found there, tracked or not. */
  instanceCount: number;
  /** Loaders Atlas can set up in a new instance there (for accepting invites); empty
   *  when it can't create instances. */
  creatableLoaders: Loader[];
  addedByUser: boolean;
}

/** An instance found in a launcher, before or after tracking. */
export interface DetectedInstance {
  launcherKind: LauncherKind;
  launcherInstanceId: string;
  name: string;
  gameDir: string;
  minecraftVersion: string | null;
  loader: Loader | null;
  loaderVersion: string | null;
  /** Set when it's already tracked. */
  instanceId: Id | null;
}

export interface ManualDownload {
  contentName: string;
  fileName: string;
  /** CurseForge page to open; Atlas picks the file up from the Downloads folder. */
  pageUrl: string;
}

/** Progress of an update/restore Atlas is applying to a folder. */
/**
 * What Atlas is doing to an instance outside an update, so it doesn't look stuck:
 * reading its files after it's tracked, or uploading and saving while sharing.
 * `done` of `total` files (both 0 while saving).
 */
export interface InstanceWork {
  stage: "reading" | "uploading" | "saving";
  done: number;
  total: number;
}

export interface SyncProgress {
  stage:
    | "waiting_for_game_exit"
    /** A new Modrinth App instance: its picture can only go in while the app is closed. */
    | "waiting_for_launcher_exit"
    | "backup"
    | "download"
    | "waiting_for_manual_downloads"
    | "apply";
  /** 0–100 */
  percent: number;
  bytesDone: number;
  bytesTotal: number;
  /** Non-empty only in stage "waiting_for_manual_downloads". */
  manualDownloads: ManualDownload[];
}

export interface InstanceSyncInfo {
  sharedInstanceId: Id;
  role: MemberRole;
  /** null when frozen with no owner. */
  owner: PublicUser | null;
  syncedRevision: number;
  latestRevision: number;
  state: Exclude<SyncState, "local_only">;
  /** Number of managed files changed locally (drives "N changes to publish"). */
  localChangeCount: number;
}

/** What the sidebar, tiles and lists need. Cheap to fetch for every instance. */
export interface InstanceSummary {
  id: Id;
  /** Mirrored from the launcher; rename it there. */
  name: string;
  /** Launcher icon key or URL; null = Atlas draws a generated block. */
  icon: string | null;
  launcher: { kind: LauncherKind; name: string };
  gameDir: string;
  /** null when the launcher metadata couldn't be read. */
  minecraftVersion: string | null;
  loader: Loader | null;
  loaderVersion: string | null;
  status: InstanceStatus;
  isFavorite: boolean;
  hideActivity: boolean;
  contentCount: number;
  /** From the launcher, when it records it. */
  lastPlayedAt: IsoDate | null;
  totalPlaytimeSeconds: number | null;
  /** The game is running from this folder right now (Atlas won't write files until it closes). */
  isRunning: boolean;
  trackedAt: IsoDate;
  lastScannedAt: IsoDate | null;
  /** null = only on this PC. */
  sync: InstanceSyncInfo | null;
  /** Present only while status is "syncing". */
  progress: SyncProgress | null;
}

export interface InstanceContent {
  id: Id;
  kind: ContentKind;
  source: ContentSource;
  projectId: string | null;
  versionId: string | null;
  name: string;
  versionLabel: string | null;
  author: string | null;
  iconUrl: string | null;
  /** Relative to the game folder, without ".disabled". */
  path: string;
  sizeBytes: number;
  side: Side;
  /** false = the file ends in ".disabled" (the convention every launcher uses). */
  enabled: boolean;
  /** Came from the shared manifest (sync may change it) vs. added by this user. */
  isManaged: boolean;
  /**
   * Owner/editors: how this file will be shared (their setting).
   * Viewers: how it was published ("required"/"optional"); their own additions are "private".
   */
  shareMode: ShareMode;
  /** How it was shared in the last published version; null = never published (new, or not shared). */
  publishedShareMode: ShareMode | null;
  /**
   * false = an optional item from the shared version that this user chose not to
   * download, so there's no file on disk. Always true for everything else.
   */
  installed: boolean;
}

export interface Backup {
  id: Id;
  reason: "before_update" | "before_restore" | "before_revert";
  fromRevision: number | null;
  toRevision: number | null;
  fileCount: number;
  sizeBytes: number;
  createdAt: IsoDate;
}

/** What the instance page needs. */
export interface InstanceDetail extends InstanceSummary {
  content: InstanceContent[];
  backups: Backup[]; // newest first
}

// ── Shared instances ───────────────────────────────────────────────────

export interface SharedMember {
  user: PublicUser;
  role: MemberRole;
  status: MemberStatus;
  /** Set when access comes from a group rather than a direct invite. */
  viaGroup: { id: Id; name: string } | null;
  lastSyncedRevision: number | null;
}

export interface SharedGroupLink {
  group: { id: Id; name: string; icon: string | null };
  role: Exclude<MemberRole, "owner">;
}

export interface SharedInstance {
  id: Id;
  name: string;
  description: string | null;
  icon: string | null;
  /** null when frozen with no owner. */
  owner: PublicUser | null;
  minecraftVersion: string;
  loader: Loader;
  loaderVersion: string | null;
  latestRevision: number;
  contentCount: number;
  /** Total download size of the latest revision. */
  sizeBytes: number;
  updatedAt: IsoDate;
  frozen: boolean;
  /** The current user's relationship to it. "invited" = shows in the inbox. */
  myRole: MemberRole;
  myStatus: MemberStatus;
  myViaGroup: { id: Id; name: string } | null;
  invitedBy: PublicUser | null;
  invitedAt: IsoDate | null;
  members: SharedMember[];
  groups: SharedGroupLink[];
  /** The local instance installed from this, if any. */
  localInstanceId: Id | null;
  /** Optional items in the latest version — what a friend can choose to download. */
  optionalItems: OptionalItem[];
}

/** An optional file a friend can choose to download. */
export interface OptionalItem {
  /** Stable across versions: provider project id, or the file hash for uploads. */
  key: string;
  name: string;
  kind: ContentKind;
  sizeBytes: number;
}

export interface RevisionChanges {
  added: string[];
  removed: string[];
  updated: { name: string; from: string; to: string }[];
}

export interface RevisionSummary {
  revision: number;
  changelog: string | null;
  createdBy: PublicUser | null;
  createdAt: IsoDate;
  changes: RevisionChanges;
}

export interface SharedInstanceDetail extends SharedInstance {
  revisions: RevisionSummary[]; // newest first
}

/** Shown in the Update dialog before syncing. */
export interface SyncPreview {
  fromRevision: number;
  toRevision: number;
  changes: RevisionChanges;
  downloadBytes: number;
  manualDownloads: ManualDownload[];
  /** Managed files the user changed locally that the update will overwrite. */
  overwrittenLocalChanges: string[];
  /** Config files both sides changed. The user picks per file; default keeps theirs. */
  configConflicts: { path: string; mine: string; theirs: string }[];
  /** The new revision needs a different Minecraft/loader version than the launcher instance has. */
  launcherChange: {
    minecraftVersion: { from: string; to: string } | null;
    loaderVersion: { from: string; to: string } | null;
    /** true = Atlas edits the launcher's instance settings itself (Prism); false = user must change it there. */
    automatic: boolean;
  } | null;
  /** The game is open; the update waits until it closes. */
  gameRunning: boolean;
  /** Optional items that are new in this update. Offered unticked; pass the chosen keys to syncInstance. */
  newOptional: OptionalItem[];
}

/** One file in the share/publish picker. */
export interface PublishItem {
  contentId: Id;
  name: string;
  iconUrl: string | null;
  kind: ContentKind;
  source: ContentSource;
  path: string;
  sizeBytes: number;
  /** Not in the last published version (everything is new on a first share). */
  isNew: boolean;
  /** The current setting; new files start "required". */
  shareMode: ShareMode;
  /** How it was shared last time; null if it's new. */
  publishedShareMode: ShareMode | null;
  /** On no mod site: sharing it (required/optional) means uploading it to Atlas. */
  needsUpload: boolean;
  /** Already on Atlas's servers (no upload or quota needed). */
  alreadyUploaded: boolean;
}

/** Built before sharing or publishing: every file with its setting, plus removals. */
export interface PublishPlan {
  /** null when this is the first share. */
  baseRevision: number | null;
  items: PublishItem[];
  /** Shared files that will disappear from the shared version (deleted or disabled locally). */
  removed: string[];
  /** CurseForge files whose authors disabled third-party downloads. Members download them by hand. */
  manualDownloads: ManualDownload[];
  storage: { usedBytes: number; quotaBytes: number; maxFileBytes: number };
  /** Files Modrinth or CurseForge couldn't be asked about (unreachable). They count as uploads. */
  unchecked: number;
  /**
   * The launcher runs other Minecraft or loader versions than the shared ones: `from` is
   * shared now, `to` is what this version will say. null when they match.
   */
  versions: {
    minecraftVersion: { from: string; to: string } | null;
    loaderVersion: { from: string; to: string } | null;
  } | null;
}

// ── Activity ───────────────────────────────────────────────────────────

export type ActivityType =
  | "friend_request"
  | "friend_accepted"
  | "instance_invite"
  | "instance_updated"
  | "instance_shared_to_group"
  | "member_joined"
  | "member_left"
  | "ownership_transferred"
  | "instance_frozen"
  | "group_joined";

export interface ActivityItem {
  id: Id;
  type: ActivityType;
  /** null if the actor deleted their account. */
  actor: PublicUser | null;
  createdAt: IsoDate;
  readAt: IsoDate | null;
  sharedInstanceId: Id | null;
  sharedInstanceName: string | null;
  groupId: Id | null;
  groupName: string | null;
  revision: number | null;
}

// ── Real-time events ───────────────────────────────────────────────────
// Pushed from Rust (WebSocket from the server, or local install/game process).
// Most are "something changed, refetch" hints: the REST data stays the source of truth.

export type AtlasEvent =
  | { type: "connection.changed"; online: boolean }
  | { type: "session.changed" }
  | {
      type: "instance.progress";
      instanceId: Id;
      status: InstanceStatus;
      progress: SyncProgress | null;
    }
  /** Work outside an update; `work: null` when it's over. */
  | { type: "instance.work"; instanceId: Id; work: InstanceWork | null }
  | { type: "instance.running"; instanceId: Id; isRunning: boolean }
  /** Rescan found changes (files edited in the launcher, metadata changed, folder missing). */
  | { type: "instance.changed"; instanceId: Id }
  /**
   * A background job (update, accept, restore) failed; the folder was left as it was.
   * Sent just before the job's final `instance.progress` with status "ready".
   */
  | { type: "instance.jobFailed"; instanceId: Id; message: string }
  | { type: "launchers.changed" }
  | { type: "presence.changed"; userId: Id; presence: Presence }
  | { type: "friends.changed" }
  | { type: "shared.changed"; sharedInstanceId: Id }
  | { type: "groups.changed"; groupId: Id | null }
  | { type: "activity.new"; item: ActivityItem }
  /** Atlas was opened with a friend or group link: take it with `takeOpenedLink`. */
  | { type: "link.opened" };

// ── Inputs ─────────────────────────────────────────────────────────────

/** Identifies a detected instance to track. */
export interface TrackInstanceRef {
  launcherKind: LauncherKind;
  launcherInstanceId: string;
}

/** Atlas-only preferences. Name, versions and settings belong to the launcher. */
export interface UpdateInstanceInput {
  isFavorite?: boolean;
  hideActivity?: boolean;
}

export interface SyncOptions {
  /** For each SyncPreview.configConflicts path: keep my file or take theirs. Missing = theirs. */
  configChoices?: Record<string, "mine" | "theirs">;
  /** Keys from SyncPreview.newOptional to download. Missing = don't. */
  installOptional?: string[];
}

export interface UpdateProfileInput {
  username?: string;
  displayName?: string;
  appearOffline?: boolean;
}

export interface ShareInstanceInput {
  description?: string | null;
  friendIds: Id[];
  groupIds: Id[];
  /** Role for everyone invited by this call. */
  role: Exclude<MemberRole, "owner">;
  /** Setting per PublishItem.contentId. Missing = keep the item's current shareMode. */
  shareModes: Record<Id, ShareMode>;
  /** User ticked "I have the right to share these files". Required when any needsUpload item isn't "private". */
  confirmedUploadRights: boolean;
}

export interface PublishInput {
  changelog: string | null;
  /** Setting per PublishItem.contentId. Missing = keep the item's current shareMode. */
  shareModes: Record<Id, ShareMode>;
  confirmedUploadRights: boolean;
}

export interface CreateGroupInviteInput {
  maxUses: number | null;
  /** null = never expires */
  expiresInHours: number | null;
}
