import type {
  ActivityItem,
  AtlasEvent,
  AuthProvider,
  Backup,
  CreateGroupInviteInput,
  DetectedInstance,
  Friend,
  FriendRequest,
  Group,
  GroupDetail,
  GroupInvite,
  GroupInvitePreview,
  GroupRole,
  Id,
  InstanceDetail,
  InstanceSummary,
  Launcher,
  LauncherKind,
  Me,
  MemberRole,
  OpenedLink,
  PublicUser,
  PublishInput,
  PublishPlan,
  Session,
  ShareInstanceInput,
  ShareMode,
  SharedInstance,
  SharedInstanceDetail,
  SyncOptions,
  SyncPreview,
  TrackInstanceRef,
  UpdateInstanceInput,
  UpdateProfileInput,
} from "$lib/types";

/**
 * Everything the UI can ask for. Components only ever talk to this interface.
 *
 * Each method maps 1:1 to a Tauri command named in snake_case
 * (listInstances -> `list_instances`). Local-only methods are answered from SQLite;
 * social ones go through Rust to the Atlas server (and fall back to `cloud_cache` offline).
 * The UI never talks to the server directly: Rust holds the tokens.
 */
export interface AtlasApi {
  // ── Session & account ────────────────────────────────────────────────
  getSession(): Promise<Session>;
  /**
   * minecraft: one-time Microsoft login in the browser, then the Mojang ownership check. Atlas doesn't keep the token.
   * discord:   opens the browser for Discord OAuth and waits for the redirect.
   */
  signIn(provider: AuthProvider): Promise<Session>;
  signOut(): Promise<void>;
  linkIdentity(provider: AuthProvider): Promise<Me>;
  /** Fails with "conflict" if it's the last sign-in method. */
  unlinkIdentity(provider: AuthProvider): Promise<Me>;
  updateProfile(patch: UpdateProfileInput): Promise<Me>;
  resetFriendCode(): Promise<Me>;
  deleteAccount(): Promise<void>;

  // ── Launchers ────────────────────────────────────────────────────────
  listLaunchers(): Promise<Launcher[]>;
  /** Re-run auto-detection (default install locations). */
  detectLaunchers(): Promise<Launcher[]>;
  /** For portable/custom installs. Fails with "invalid_input" if the folder isn't a known launcher. */
  addLauncher(
    kind: Exclude<LauncherKind, "folder">,
    rootPath: string,
  ): Promise<Launcher>;
  removeLauncher(id: Id): Promise<void>;
  /** Every instance in every known launcher, tracked or not. */
  listDetectedInstances(): Promise<DetectedInstance[]>;

  // ── Instances (tracked folders) ──────────────────────────────────────
  listInstances(): Promise<InstanceSummary[]>;
  getInstance(id: Id): Promise<InstanceDetail>;
  trackInstances(refs: TrackInstanceRef[]): Promise<InstanceSummary[]>;
  /** Track a game folder that isn't in a known launcher. */
  trackFolder(gameDir: string): Promise<InstanceSummary>;
  /**
   * Stops tracking. The folder is never touched. A shared instance stays shared and you stay in it
   * (use leaveSharedInstance or archiveSharedInstance); acceptInvite adds it back.
   */
  untrackInstance(id: Id): Promise<void>;
  updateInstance(id: Id, patch: UpdateInstanceInput): Promise<InstanceSummary>;
  /** Re-read the folder and launcher metadata now (Atlas also watches folders and rescans on its own). */
  rescanInstance(id: Id): Promise<InstanceSummary>;
  /** Opens the launcher; Prism/MultiMC can launch the instance directly. */
  openInLauncher(id: Id): Promise<void>;
  openInstanceFolder(id: Id): Promise<void>;
  /** Renames the file to/from ".disabled". Fails with "conflict" while the game is running. */
  setContentEnabled(
    instanceId: Id,
    contentId: Id,
    enabled: boolean,
  ): Promise<void>;
  restoreBackup(instanceId: Id, backupId: Id): Promise<InstanceSummary>;
  /**
   * Owners/editors: how a file is shared (required / optional / only me).
   * Saved locally; it reaches friends with the next publish.
   */
  setShareMode(instanceId: Id, contentId: Id, mode: ShareMode): Promise<void>;
  /**
   * Receivers: download (true) or remove (false) an optional item from the shared version.
   * Fails with "conflict" while the game is running.
   */
  setOptionalInstalled(
    instanceId: Id,
    contentId: Id,
    installed: boolean,
  ): Promise<void>;

  // ── Sharing ──────────────────────────────────────────────────────────
  listSharedInstances(): Promise<SharedInstance[]>;
  getSharedInstance(id: Id): Promise<SharedInstanceDetail>;
  /**
   * Creates a new instance in the chosen launcher, fills it with the latest revision, and tracks it.
   * Accepts a pending invite; members and owners can also add one that isn't on this PC.
   * `installOptional`: keys from SharedInstance.optionalItems the user ticked (optional items start unticked).
   */
  acceptInvite(
    sharedInstanceId: Id,
    launcherId: Id,
    installOptional: string[],
  ): Promise<InstanceSummary>;
  /**
   * Like acceptInvite, but into an instance already tracked here (any launcher, not shared yet).
   * The shared files are added; the user's own files stay.
   */
  acceptInviteInto(
    sharedInstanceId: Id,
    instanceId: Id,
    installOptional: string[],
  ): Promise<InstanceSummary>;
  declineInvite(sharedInstanceId: Id): Promise<void>;
  /** Step 1 of sharing/publishing: what would be shared, and which files need a decision. */
  preparePublish(instanceId: Id): Promise<PublishPlan>;
  /** First share: publishes revision 1 and invites friends/groups. */
  shareInstance(
    instanceId: Id,
    input: ShareInstanceInput,
  ): Promise<SharedInstance>;
  /** Owners/editors: publish local changes as a new revision. */
  publishInstance(
    instanceId: Id,
    input: PublishInput,
  ): Promise<InstanceSummary>;
  previewSync(instanceId: Id): Promise<SyncPreview>;
  /** Backs up, then writes the latest revision into the folder. Waits if the game is running. */
  syncInstance(instanceId: Id, options?: SyncOptions): Promise<InstanceSummary>;
  /** Viewers: throw away local changes to managed content. */
  revertLocalChanges(instanceId: Id): Promise<InstanceSummary>;
  updateSharedInstance(
    id: Id,
    patch: { name?: string; description?: string | null; icon?: string | null },
  ): Promise<SharedInstance>;
  inviteToSharedInstance(
    id: Id,
    input: {
      friendIds: Id[];
      groupIds: Id[];
      role: Exclude<MemberRole, "owner">;
    },
  ): Promise<SharedInstance>;
  setSharedMemberRole(
    id: Id,
    userId: Id,
    role: Exclude<MemberRole, "owner">,
  ): Promise<SharedInstance>;
  removeSharedMember(id: Id, userId: Id): Promise<SharedInstance>;
  unshareFromGroup(id: Id, groupId: Id): Promise<SharedInstance>;
  transferSharedInstance(id: Id, userId: Id): Promise<SharedInstance>;
  /** Members: stop receiving it. The folder stays as it is, now only on this PC. */
  leaveSharedInstance(id: Id): Promise<void>;
  /** Owner: stop sharing for everyone. Members keep their folders as they are. */
  archiveSharedInstance(id: Id): Promise<void>;

  // ── Friends ──────────────────────────────────────────────────────────
  listFriends(): Promise<Friend[]>;
  listFriendRequests(): Promise<FriendRequest[]>;
  /** Who a friend code belongs to, shown before sending the request. */
  previewFriendCode(code: string): Promise<PublicUser>;
  sendFriendRequest(code: string): Promise<FriendRequest>;
  respondToFriendRequest(userId: Id, accept: boolean): Promise<void>;
  cancelFriendRequest(userId: Id): Promise<void>;
  removeFriend(userId: Id): Promise<void>;
  blockUser(userId: Id): Promise<void>;
  /** The friend or group link Atlas was opened with, once (null if none is waiting). */
  takeOpenedLink(): Promise<OpenedLink | null>;

  // ── Groups ───────────────────────────────────────────────────────────
  listGroups(): Promise<Group[]>;
  getGroup(id: Id): Promise<GroupDetail>;
  createGroup(input: { name: string; icon?: string | null }): Promise<Group>;
  updateGroup(
    id: Id,
    patch: { name?: string; icon?: string | null },
  ): Promise<Group>;
  deleteGroup(id: Id): Promise<void>;
  createGroupInvite(
    groupId: Id,
    input: CreateGroupInviteInput,
  ): Promise<GroupInvite>;
  revokeGroupInvite(groupId: Id, code: string): Promise<void>;
  previewGroupInvite(code: string): Promise<GroupInvitePreview>;
  joinGroup(code: string): Promise<Group>;
  setGroupMemberRole(
    groupId: Id,
    userId: Id,
    role: Exclude<GroupRole, "owner">,
  ): Promise<Group>;
  /** Owner/admin kicks someone, or pass your own id to leave. */
  removeGroupMember(groupId: Id, userId: Id): Promise<void>;
  transferGroup(groupId: Id, userId: Id): Promise<Group>;

  // ── Activity ─────────────────────────────────────────────────────────
  listActivity(): Promise<ActivityItem[]>;
  markActivityRead(ids: Id[] | "all"): Promise<void>;

  // ── Real-time ────────────────────────────────────────────────────────
  /** Returns an unsubscribe function. */
  subscribe(listener: (event: AtlasEvent) => void): () => void;
}

export type ApiErrorCode =
  | "not_found"
  | "not_signed_in"
  | "forbidden"
  | "offline"
  | "conflict"
  | "invalid_input"
  | "rate_limited"
  | "quota_exceeded"
  | "unknown";

/** Thrown by any API method. `code` is stable and safe to switch on in the UI. */
export class ApiError extends Error {
  constructor(
    public code: ApiErrorCode,
    message: string,
  ) {
    super(message);
  }
}
