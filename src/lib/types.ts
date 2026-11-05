// Written from the Rust types (atlas-protocol, src-tauri/src/dto and src-tauri/src/events.rs)
// by src-tauri/src/dto/typescript.rs. Don't edit it: change the Rust side, then run
// `ATLAS_WRITE_TYPES=1 cargo test --manifest-path src-tauri/Cargo.toml types_ts`.

/** UUIDv7 string. */
export type Id = string;
/** ISO-8601 UTC string, e.g. "2026-10-19T16:39:00.000Z". */
export type IsoDate = string;

export type Loader = "vanilla" | "fabric" | "quilt" | "forge" | "neoforge";

/**
 * Launchers Atlas can read. `Folder` = a game folder added by hand.
 */
export type LauncherKind = "prism" | "multimc" | "modrinth" | "curseforge" | "official" | "folder";

/**
 * `Syncing` is never stored: it comes from the job manager at response time.
 */
export type InstanceStatus = "ready" | "syncing" | "missing";

export type ContentKind = "mod" | "resourcepack" | "shaderpack" | "datapack";

export type ContentSource = "modrinth" | "curseforge" | "upload" | "local";

/**
 * How a file in an instance is shared. New files start `Required`.
 */
export type ShareMode = "required" | "optional" | "private";

export type Side = "client" | "server" | "both";

export type MemberRole = "owner" | "editor" | "viewer";

/**
 * A role you can give someone: any but owner.
 */
export type InviteRole = "editor" | "viewer";

export type MemberStatus = "invited" | "active" | "declined";

export type GroupRole = "owner" | "admin" | "member";

export type PresenceState = "offline" | "online" | "playing";

export type AuthProvider = "minecraft" | "discord";

export type AvatarSource = "minecraft" | "discord";

export type RequestDirection = "incoming" | "outgoing";

/**
 * Derived, never stored.
 */
export type SyncState = "local_only" | "up_to_date" | "update_available" | "local_changes" | "conflict" | "frozen";

export type SyncStage = "waiting_for_game_exit" | "backup" | "download" | "waiting_for_manual_downloads" | "apply";

/**
 * Work on an instance outside a job: see `Work`.
 */
export type WorkStage = "reading" | "uploading" | "saving" | "downloading";

export type BackupReason = "before_update" | "before_restore" | "before_revert";

/**
 * Why a config file isn't shared as it is (see `LeftOutFile`): not in a folder
 * Atlas uploads from, not a file type it uploads, too big, past the count or total
 * limit, or changed here but not uploadable so the modpack's own copy goes instead.
 */
export type LeftOutReason = "folder" | "login" | "program" | "file_type" | "too_big" | "over_limit" | "pack_copy";

/**
 * SyncOptions.configChoices values.
 */
export type ConfigChoice = "mine" | "theirs";

export type ActivityType = "friend_request" | "friend_accepted" | "instance_invite" | "instance_updated" | "instance_shared_to_group" | "member_joined" | "member_left" | "ownership_transferred" | "instance_frozen" | "group_joined" | "role_changed";

/**
 * What a report is about.
 */
export type ReportTarget = "user" | "shared_instance" | "group" | "blob";

/**
 * Why something was reported.
 */
export type ReportReason = "copyright" | "malware" | "abuse" | "other";

export type LinkKind = "friend" | "group";

/**
 * How Atlas behaves on this PC, outside any account.
 */
export type AppSettings = { 
/**
 * Notices when something needs the player (see `Notice`).
 */
notifications: boolean, 
/**
 * The quiet sound that goes with a notice.
 */
notificationSound: boolean, 
/**
 * The close button hides Atlas to the tray instead of quitting it.
 */
closeToTray: boolean, 
/**
 * Atlas starts in the tray when you sign in to Windows.
 */
startWithWindows: boolean, 
/**
 * Where the accept dialog's "Keep it up to date automatically" starts.
 */
autoUpdate: boolean, 
/**
 * The "start with Windows" screen after the first accepted pack has been shown.
 */
startWithWindowsOffered: boolean, };

/**
 * Only the fields given change.
 */
export type AppSettingsPatch = { notifications?: boolean, notificationSound?: boolean, closeToTray?: boolean, startWithWindows?: boolean, autoUpdate?: boolean, startWithWindowsOffered?: boolean, };

/**
 * A newer Atlas on the release page.
 */
export type AvailableUpdate = { version: string, 
/**
 * The release notes.
 */
notes: string | null, };

/**
 * A friend or group link Atlas was opened with (`atlas://friend/<code>`).
 */
export type OpenedLink = { kind: LinkKind, 
/**
 * Normalized: a friend code or a group invite code.
 */
code: string, };

/**
 * `POST /reports`: tell Atlas about something that breaks the rules. Only things the
 * reporter can see: a member's shared instance, a group they're in, any user, or an
 * uploaded file (`target_id` is its sha256) in `shared_instance_id`, which they're in.
 */
export type NewReport = { targetType: ReportTarget, targetId: string, reason: ReportReason, details?: string | null, sharedInstanceId?: string | null, };

/**
 * Any Atlas user as seen by others.
 */
export type PublicUser = { id: string, username: string, displayName: string, 
/**
 * The picture to show, whichever they chose: their Minecraft skin (draw its face) or
 * a picture (the Discord avatar). Both None = initials.
 */
skinUrl: string | null, avatarUrl: string | null, minecraftUuid: string | null, minecraftUsername: string | null, };

export type LinkedIdentity = { provider: AuthProvider, 
/**
 * Minecraft name or Discord username at the last sign-in.
 */
username: string, linkedAt: string, };

export type Storage = { usedBytes: number, quotaBytes: number, maxFileBytes: number, };

/**
 * The signed-in user, with private fields.
 */
export type Me = { 
/**
 * "7KQ29XMB"
 */
friendCode: string, 
/**
 * e.g. "https://atlas.example/f/7KQ29XMB"
 */
friendLink: string, identities: Array<LinkedIdentity>, appearOffline: boolean, 
/**
 * When the username can next be changed (None = now).
 */
usernameChangeAvailableAt: string | null, storage: Storage, 
/**
 * Which picture they chose, and both to choose from (None = not linked, or no
 * custom skin / avatar). Defaults so a `Me` the app saved before this existed loads.
 */
avatarSource: AvatarSource, minecraftSkinUrl: string | null, discordAvatarUrl: string | null, id: string, username: string, displayName: string, 
/**
 * The picture to show, whichever they chose: their Minecraft skin (draw its face) or
 * a picture (the Discord avatar). Both None = initials.
 */
skinUrl: string | null, avatarUrl: string | null, minecraftUuid: string | null, minecraftUsername: string | null, };

export type Session = { 
/**
 * None = not signed in (tracking still works; friends/sharing don't).
 */
user: Me | null, 
/**
 * false = server unreachable; social data comes from the offline cache.
 */
online: boolean, 
/**
 * false = this copy of Atlas has no server to sign in to.
 */
server: boolean, };

export type Presence = { state: PresenceState, 
/**
 * Set when playing, unless the friend hid it.
 */
instanceName: string | null, minecraftVersion: string | null, 
/**
 * Set when playing a shared instance you can see too.
 */
sharedInstanceId: string | null, 
/**
 * Minecraft is open, not just the Atlas app. Always true when playing.
 */
inMinecraft: boolean, 
/**
 * When offline, this is "last seen".
 */
updatedAt: string, };

export type Friend = { user: PublicUser, friendsSince: string, presence: Presence, };

export type FriendRequest = { direction: RequestDirection, user: PublicUser, createdAt: string, };

/**
 * `GET /me/uploads`: one file the user uploaded, and what still uses it.
 */
export type MyUpload = { sha256: string, fileName: string, sizeBytes: number, uploadedAt: string, 
/**
 * Shared instances whose current versions (or picture) use it. Empty: nothing does,
 * and it can be removed (the server also removes it by itself after a day).
 */
usedIn: Array<string>, 
/**
 * Taken down: it stays listed, so the same file can't be uploaded again.
 */
blocked: boolean, };

/**
 * `POST /me/uploads/remove-unused`: what removing every unused upload freed.
 */
export type RemovedUploads = { count: number, sizeBytes: number, };

/**
 * `PATCH /me`. Missing fields stay as they are.
 */
export type UpdateProfileInput = { username?: string, displayName?: string, appearOffline?: boolean, avatarSource?: AvatarSource, };

export type GroupMember = { user: PublicUser, role: GroupRole, joinedAt: string, };

export type GroupInvite = { code: string, 
/**
 * A web page that opens the invite in Atlas.
 */
link: string, 
/**
 * None if they deleted their account.
 */
createdBy: PublicUser | null, maxUses: number | null, uses: number, expiresAt: string | null, createdAt: string, };

export type Group = { id: string, name: string, icon: string | null, myRole: GroupRole, members: Array<GroupMember>, 
/**
 * Shared instances available to the group.
 */
sharedInstanceIds: Array<string>, createdAt: string, };

/**
 * `GET /groups/{id}`.
 */
export type GroupDetail = { 
/**
 * Only filled for the owner and admins.
 */
invites: Array<GroupInvite>, 
/**
 * Friends with an invite sent in Atlas they haven't answered. Only filled for the
 * owner and admins; missing from older servers.
 */
invited: Array<string>, id: string, name: string, icon: string | null, myRole: GroupRole, members: Array<GroupMember>, 
/**
 * Shared instances available to the group.
 */
sharedInstanceIds: Array<string>, createdAt: string, };

/**
 * `GET /group-invites/{code}`: what an invite leads to, before joining.
 */
export type GroupInvitePreview = { code: string, groupId: string, groupName: string, groupIcon: string | null, memberCount: number, invitedBy: PublicUser | null, alreadyMember: boolean, };

/**
 * `POST /groups`.
 */
export type CreateGroup = { name: string, icon?: string | null, };

/**
 * `POST /groups/{id}/invites`.
 */
export type CreateGroupInvite = { 
/**
 * None = any number of people.
 */
maxUses?: number | null, 
/**
 * None = never expires.
 */
expiresInHours?: number | null, };

export type Launcher = { id: string, 
/**
 * Never `Folder` here.
 */
kind: LauncherKind, name: string, rootPath: string, instanceCount: number, 
/**
 * The loaders Atlas can set up in a new instance there; empty when it can't create any.
 */
creatableLoaders: Array<Loader>, addedByUser: boolean, };

/**
 * An instance found in a launcher, before or after tracking.
 */
export type DetectedInstance = { launcherKind: LauncherKind, launcherInstanceId: string, name: string, gameDir: string, minecraftVersion: string | null, loader: Loader | null, loaderVersion: string | null, 
/**
 * The launcher's picture, cached like a tracked instance's; None draws a block.
 */
icon: string | null, 
/**
 * Set when it's already tracked.
 */
instanceId: string | null, };

export type ManualDownload = { contentName: string, fileName: string, pageUrl: string, };

/**
 * What Atlas is doing to an instance outside an update, so it doesn't look stuck:
 * reading its files after it's tracked, or uploading files and saving a version while
 * sharing. `done` of `total`: files while reading, bytes while uploading, both 0 for
 * `Saving`.
 */
export type InstanceWork = { stage: WorkStage, done: number, total: number, };

/**
 * Progress of an update/restore Atlas is applying to a folder.
 */
export type SyncProgress = { stage: SyncStage, 
/**
 * 0–100
 */
percent: number, bytesDone: number, bytesTotal: number, manualDownloads: Array<ManualDownload>, };

export type InstanceSyncInfo = { sharedInstanceId: string, role: MemberRole, owner: PublicUser | null, syncedRevision: number, latestRevision: number, 
/**
 * Never `LocalOnly` here (that's `sync: null`).
 */
state: SyncState, localChangeCount: number, 
/**
 * New versions install by themselves, when nothing needs the player's say.
 */
autoUpdate: boolean, };

/**
 * The launcher an instance is in, as InstanceSummary shows it.
 */
export type LauncherRef = { kind: LauncherKind, name: string, };

/**
 * What the sidebar, tiles and lists need. Cheap to build for every instance.
 */
export type InstanceSummary = { id: string, name: string, icon: string | null, launcher: LauncherRef, gameDir: string, minecraftVersion: string | null, loader: Loader | null, loaderVersion: string | null, status: InstanceStatus, isFavorite: boolean, hideActivity: boolean, contentCount: number, lastPlayedAt: string | null, totalPlaytimeSeconds: number | null, isRunning: boolean, trackedAt: string, lastScannedAt: string | null, sync: InstanceSyncInfo | null, 
/**
 * Shared while signed in to another server or account: not shared on this one
 * (`sync` is None), and signing back in to that one brings the link back.
 */
sharedElsewhere: boolean, progress: SyncProgress | null, };

export type InstanceContent = { id: string, kind: ContentKind, source: ContentSource, projectId: string | null, versionId: string | null, name: string, versionLabel: string | null, author: string | null, iconUrl: string | null, 
/**
 * Relative to the game folder, without ".disabled".
 */
path: string, sizeBytes: number, side: Side, enabled: boolean, isManaged: boolean, shareMode: ShareMode, publishedShareMode: ShareMode | null, installed: boolean, };

export type Backup = { id: string, reason: BackupReason, fromRevision: number | null, toRevision: number | null, fileCount: number, sizeBytes: number, createdAt: string, };

/**
 * What the instance page needs.
 */
export type InstanceDetail = { content: Array<InstanceContent>, 
/**
 * Newest first.
 */
backups: Array<Backup>, id: string, name: string, icon: string | null, launcher: LauncherRef, gameDir: string, minecraftVersion: string | null, loader: Loader | null, loaderVersion: string | null, status: InstanceStatus, isFavorite: boolean, hideActivity: boolean, contentCount: number, lastPlayedAt: string | null, totalPlaytimeSeconds: number | null, isRunning: boolean, trackedAt: string, lastScannedAt: string | null, sync: InstanceSyncInfo | null, 
/**
 * Shared while signed in to another server or account: not shared on this one
 * (`sync` is None), and signing back in to that one brings the link back.
 */
sharedElsewhere: boolean, progress: SyncProgress | null, };

/**
 * Identifies a detected instance to track.
 */
export type TrackInstanceRef = { launcherKind: LauncherKind, launcherInstanceId: string, };

/**
 * Atlas-only preferences. Name, versions and settings belong to the launcher.
 */
export type UpdateInstanceInput = { isFavorite?: boolean, hideActivity?: boolean, autoUpdate?: boolean, };

/**
 * `{ id, name }`: how a group is named on a member row.
 */
export type GroupRef = { id: string, name: string, };

export type SharedMember = { user: PublicUser, role: MemberRole, status: MemberStatus, viaGroup: GroupRef | null, lastSyncedRevision: number | null, };

export type GroupLinkGroup = { id: string, name: string, icon: string | null, };

export type SharedGroupLink = { group: GroupLinkGroup, role: InviteRole, };

/**
 * An optional file a friend can choose to download.
 */
export type OptionalItem = { 
/**
 * Provider project id, or the file's sha256 for uploads.
 */
key: string, name: string, kind: ContentKind, sizeBytes: number, };

/**
 * A file in a shared pack that isn't from Modrinth or CurseForge: someone uploaded it.
 */
export type UploadedFile = { name: string, 
/**
 * The file's own name: "tandem-fabric-0.1.0.jar".
 */
fileName: string, kind: ContentKind, sizeBytes: number, };

export type SharedInstance = { id: string, name: string, description: string | null, icon: string | null, 
/**
 * None when frozen with no owner.
 */
owner: PublicUser | null, minecraftVersion: string, loader: Loader, loaderVersion: string | null, latestRevision: number, contentCount: number, 
/**
 * Total download size of the latest revision.
 */
sizeBytes: number, updatedAt: string, frozen: boolean, myRole: MemberRole, myStatus: MemberStatus, myViaGroup: GroupRef | null, invitedBy: PublicUser | null, invitedAt: string | null, members: Array<SharedMember>, groups: Array<SharedGroupLink>, 
/**
 * Filled in by the app from its own instances; the server always sends null.
 */
localInstanceId: string | null, optionalItems: Array<OptionalItem>, 
/**
 * Mods and resource packs in the latest version that aren't from Modrinth or
 * CurseForge: someone uploaded them, and nobody has checked what's inside.
 */
uploadedFiles: Array<string>, 
/**
 * The same files with their file names and sizes. Apps before 0.4.1 only read `uploaded_files`.
 */
uploaded: Array<UploadedFile>, };

export type UpdatedItem = { name: string, from: string, to: string, };

export type RevisionChanges = { added: Array<string>, removed: Array<string>, updated: Array<UpdatedItem>, };

export type RevisionSummary = { revision: number, changelog: string | null, createdBy: PublicUser | null, createdAt: string, changes: RevisionChanges, };

/**
 * A shared instance with its versions.
 */
export type SharedInstanceDetail = { 
/**
 * Newest first.
 */
revisions: Array<RevisionSummary>, id: string, name: string, description: string | null, icon: string | null, 
/**
 * None when frozen with no owner.
 */
owner: PublicUser | null, minecraftVersion: string, loader: Loader, loaderVersion: string | null, latestRevision: number, contentCount: number, 
/**
 * Total download size of the latest revision.
 */
sizeBytes: number, updatedAt: string, frozen: boolean, myRole: MemberRole, myStatus: MemberStatus, myViaGroup: GroupRef | null, invitedBy: PublicUser | null, invitedAt: string | null, members: Array<SharedMember>, groups: Array<SharedGroupLink>, 
/**
 * Filled in by the app from its own instances; the server always sends null.
 */
localInstanceId: string | null, optionalItems: Array<OptionalItem>, 
/**
 * Mods and resource packs in the latest version that aren't from Modrinth or
 * CurseForge: someone uploaded them, and nobody has checked what's inside.
 */
uploadedFiles: Array<string>, 
/**
 * The same files with their file names and sizes. Apps before 0.4.1 only read `uploaded_files`.
 */
uploaded: Array<UploadedFile>, };

export type ConfigConflict = { path: string, mine: string, theirs: string, };

export type VersionChange = { from: string, to: string, };

export type LauncherChange = { minecraftVersion: VersionChange | null, loaderVersion: VersionChange | null, 
/**
 * true = Atlas edits the launcher's instance settings itself (Prism).
 */
automatic: boolean, };

/**
 * Shown on the Review update page before syncing.
 */
export type SyncPreview = { fromRevision: number, toRevision: number, changes: RevisionChanges, downloadBytes: number, manualDownloads: Array<ManualDownload>, overwrittenLocalChanges: Array<string>, configConflicts: Array<ConfigConflict>, launcherChange: LauncherChange | null, gameRunning: boolean, newOptional: Array<OptionalItem>, 
/**
 * Mods and resource packs this update brings that aren't from Modrinth or CurseForge:
 * someone uploaded them, and nobody has checked what's inside.
 */
unverified: Array<string>, };

export type SyncOptions = { 
/**
 * Missing path = theirs.
 */
configChoices?: { [key in string]: ConfigChoice }, 
/**
 * Keys from SyncPreview.newOptional to download. Missing = don't.
 */
installOptional?: Array<string>, };

/**
 * One file in the share/publish picker.
 */
export type PublishItem = { contentId: string, name: string, iconUrl: string | null, kind: ContentKind, source: ContentSource, path: string, sizeBytes: number, isNew: boolean, shareMode: ShareMode, publishedShareMode: ShareMode | null, needsUpload: boolean, alreadyUploaded: boolean, 
/**
 * Modrinth and CurseForge couldn't be asked about it: not an upload until they can.
 */
unchecked: boolean, 
/**
 * Shared already, but this file's contents differ from the shared version's (a jar
 * replaced by a build with the same name).
 */
contentsChanged: boolean, };

/**
 * The modpack a share is based on: its own files come from its download.
 */
export type PackInfo = { name: string, versionLabel: string | null, source: ContentSource, 
/**
 * Files shared from it rather than uploaded.
 */
files: number, };

/**
 * A config file that won't be shared as it is here, and why. Listed in the dialog
 * before anything uploads.
 */
export type LeftOutFile = { path: string, reason: LeftOutReason, };

/**
 * A file in the share dialog's tree: anything in the instance folder except mods and
 * packs (listed apart) and what's never shared.
 */
export type ShareFile = { 
/**
 * "config/create/server.toml"
 */
path: string, sizeBytes: number, modified: string | null, 
/**
 * The modpack ships it: ticked by default, and not uploaded if unchanged.
 */
fromPack: boolean, 
/**
 * Ticked, from the pack, and set differently here: the owner's copy is uploaded.
 */
changed: boolean, 
/**
 * Why it can't be uploaded even when ticked. A file from the pack still goes out
 * as the pack has it.
 */
refusal: LeftOutReason | null, };

/**
 * Which tree files are shared: paths ticked or unticked, a folder ending in '/' and ""
 * for everything. The longest matching rule wins; with none, config and script folders
 * and the pack's own files are shared. Saved with each share for the next one.
 */
export type FileRules = { include: Array<string>, exclude: Array<string>, 
/**
 * Files members get once and then keep as their own (options.txt always is).
 */
firstInstall: Array<string>, };

export type PublishPlan = { 
/**
 * None on a first share.
 */
baseRevision: number | null, items: Array<PublishItem>, removed: Array<string>, manualDownloads: Array<ManualDownload>, storage: Storage, 
/**
 * Every other file in the folder, for the tree.
 */
files: Array<ShareFile>, 
/**
 * Which of them are shared now.
 */
rules: FileRules, 
/**
 * The modpack the instance is from, when its files can come from its download.
 */
pack: PackInfo | null, 
/**
 * Ticked files that won't be shared as they are here.
 */
leftOut: Array<LeftOutFile>, 
/**
 * The launcher runs other Minecraft or loader versions than the shared ones: `from`
 * is shared now, `to` is what this version will say. `automatic` is unused here.
 */
versions: LauncherChange | null, 
/**
 * Other files whose shared copy would change, with the ticks as saved: edited,
 * newly shared or no longer shared.
 */
filesChanged: Array<string>, };

export type ShareInstanceInput = { description?: string | null, friendIds: Array<string>, groupIds: Array<string>, role: InviteRole, 
/**
 * Setting per PublishItem.contentId. Missing = keep the item's current shareMode.
 */
shareModes: { [key in string]: ShareMode }, confirmedUploadRights: boolean, 
/**
 * The tree's ticks, saved for later publishes. Missing = keep the saved ones.
 */
files?: FileRules, };

export type PublishInput = { changelog: string | null, shareModes: { [key in string]: ShareMode }, confirmedUploadRights: boolean, 
/**
 * As in ShareInstanceInput.
 */
files?: FileRules, };

/**
 * `inviteToSharedInstance` input.
 */
export type InviteInput = { friendIds: Array<string>, groupIds: Array<string>, role: InviteRole, };

export type ActivityItem = { id: string, type: ActivityType, 
/**
 * None if the actor deleted their account.
 */
actor: PublicUser | null, createdAt: string, readAt: string | null, sharedInstanceId: string | null, sharedInstanceName: string | null, groupId: string | null, groupName: string | null, revision: number | null, 
/**
 * `role_changed`: the role they have now.
 */
role?: MemberRole | null, };

/**
 * Something that needs the player: a pack shared with them, a friend request, or an
 * update that can't go ahead without them. Shown inside Atlas when its window is
 * visible, else as a Windows notification.
 */
export type Notice = { title: string, body: string, 
/**
 * Where its button goes.
 */
target: NoticeTarget, };

export type NoticeTarget = { "kind": "pack", instanceId: string, } | { "kind": "review", instanceId: string, } | { "kind": "invite", sharedInstanceId: string, } | { "kind": "friends" } | { "kind": "home" };

export type AtlasEvent = { "type": "connection.changed", online: boolean, } | { "type": "session.changed", } | { "type": "instance.progress", instanceId: string, status: InstanceStatus, progress: SyncProgress | null, } | { "type": "instance.work", instanceId: string, work: InstanceWork | null, } | { "type": "instance.running", instanceId: string, isRunning: boolean, } | { "type": "instance.changed", instanceId: string, } | { "type": "instance.jobFailed", instanceId: string, message: string, } | { "type": "launchers.changed", } | { "type": "presence.changed", userId: string, presence: Presence, } | { "type": "friends.changed", } | { "type": "shared.changed", sharedInstanceId: string, } | { "type": "groups.changed", groupId: string | null, } | { "type": "activity.new", item: ActivityItem, } | { "type": "link.opened", } | { "type": "update.progress", downloaded: number, total: number | null, } | { "type": "update.required", } | { "type": "notice", notice: Notice, };
