// In-memory implementation of AtlasApi backed by the fixtures.
// Mutations really change the data (until reload), and it pushes fake real-time events
// (sync progress, game running, friends coming online), so UI flows can be clicked through end to end.
//
// Tweak with env vars in .env.local:
//   VITE_MOCK_SCENARIO=default | empty | signed_out | offline
//   VITE_MOCK_LATENCY=300        (ms, default 250)
//   VITE_MOCK_FAIL_RATE=0.1      (0–1, random ApiError("unknown") to test error states)
//   VITE_MOCK_EVENTS=false       (stop the random presence changes)
//
// From the browser console: __atlasMock.setRunning("<instance id>", true) pretends the game is open.

import * as fx from "$lib/mock/fixtures";
import type {
  AtlasEvent,
  Backup,
  ContentKind,
  DetectedInstance,
  FriendRequest,
  Group,
  GroupDetail,
  Id,
  InstanceContent,
  InstanceDetail,
  InstanceSummary,
  Launcher,
  Me,
  MemberRole,
  OptionalItem,
  PublicUser,
  RevisionChanges,
  ShareMode,
  SharedInstance,
  SharedInstanceDetail,
  SyncProgress,
} from "$lib/types";
import { ApiError, type AtlasApi } from "./api";

type Scenario = "default" | "empty" | "signed_out" | "offline";

const env = import.meta.env;
const scenario = (env.VITE_MOCK_SCENARIO ?? "default") as Scenario;
const latency = Number(env.VITE_MOCK_LATENCY ?? 250);
const failRate = Number(env.VITE_MOCK_FAIL_RATE ?? 0);
const randomEvents = env.VITE_MOCK_EVENTS !== "false" && scenario === "default";

const social = scenario === "default" || scenario === "offline";

// Deep copies so mutations never touch the fixture module.
const db = {
  user: (scenario === "signed_out"
    ? null
    : structuredClone(fx.me)) as Me | null,
  online: scenario !== "offline",
  launchers: structuredClone(fx.launchers) as Launcher[],
  instances: scenario === "empty" ? [] : structuredClone(fx.instances),
  untracked: structuredClone(fx.untrackedDetected) as DetectedInstance[],
  shared: social ? structuredClone(fx.sharedInstances) : [],
  friends: social ? structuredClone(fx.friends) : [],
  requests: social ? structuredClone(fx.friendRequests) : [],
  groups: social ? structuredClone(fx.groups) : [],
  activity: social ? structuredClone(fx.activity) : [],
};

// In the empty scenario the launchers still contain instances; they're just not tracked yet.
if (scenario === "empty") {
  db.untracked.unshift(
    ...fx.instances
      .filter((i) => i.status !== "missing")
      .map((i): DetectedInstance => ({
        launcherKind: i.launcher.kind,
        launcherInstanceId: i.name,
        name: i.name,
        gameDir: i.gameDir,
        minecraftVersion: i.minecraftVersion,
        loader: i.loader,
        loaderVersion: i.loaderVersion,
        instanceId: null,
      })),
  );
}

// ── Plumbing ───────────────────────────────────────────────────────────

const listeners = new Set<(e: AtlasEvent) => void>();
const emit = (e: AtlasEvent) => listeners.forEach((l) => l(structuredClone(e)));

async function respond<T>(value: T): Promise<T> {
  await new Promise((r) =>
    setTimeout(r, latency * (0.6 + Math.random() * 0.8)),
  );
  if (failRate > 0 && Math.random() < failRate)
    throw new ApiError("unknown", "Simulated failure");
  return structuredClone(value);
}

function me(): Me {
  if (!db.user)
    throw new ApiError("not_signed_in", "Sign in to use friends and sharing");
  return db.user;
}

/** Social calls: need a user, and in the offline scenario writes fail. */
function online(): Me {
  const user = me();
  if (!db.online) throw new ApiError("offline", "Can't reach Atlas servers");
  return user;
}

const publicMe = (): PublicUser => {
  const {
    id,
    username,
    displayName,
    avatarUrl,
    minecraftUuid,
    minecraftUsername,
  } = me();
  return {
    id,
    username,
    displayName,
    avatarUrl,
    minecraftUuid,
    minecraftUsername,
  };
};

function findInstance(id: Id): InstanceDetail {
  const inst = db.instances.find((i) => i.id === id);
  if (!inst) throw new ApiError("not_found", "Instance not found");
  return inst;
}

function findShared(id: Id): SharedInstanceDetail {
  const s = db.shared.find((i) => i.id === id);
  if (!s) throw new ApiError("not_found", "Shared instance not found");
  return s;
}

function findGroup(id: Id): GroupDetail {
  const g = db.groups.find((i) => i.id === id);
  if (!g) throw new ApiError("not_found", "Group not found");
  return g;
}

function requireRole(s: SharedInstance, ...roles: MemberRole[]) {
  if (s.frozen) throw new ApiError("conflict", "This instance is frozen");
  if (!roles.includes(s.myRole))
    throw new ApiError("forbidden", "You don't have permission to do that");
}

function toSummary({
  content,
  backups,
  ...summary
}: InstanceDetail): InstanceSummary {
  // Optional items the user didn't download are listed but have no file: don't count them.
  return {
    ...summary,
    contentCount: content.filter((c) => c.installed).length,
  };
}

function toSharedSummary({
  revisions,
  ...summary
}: SharedInstanceDetail): SharedInstance {
  return summary;
}

function toGroup({ invites, ...group }: GroupDetail): Group {
  return group;
}

const newId = () => crypto.randomUUID();
const nowIso = () => new Date().toISOString();
const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const randomCode = (len: number) =>
  Array.from(
    { length: len },
    () => CROCKFORD[Math.floor(Math.random() * 32)],
  ).join("");
/** Accepts "7KQ2-9XMB", "7kq29xmb" or a full link "https://atlas.example/f/7KQ29XMB". */
const normalizeCode = (input: string) =>
  input
    .trim()
    .split("/")
    .pop()!
    .toUpperCase()
    .replace(/[^0-9A-Z]/g, "");

/** Fake applying files: waits for the game to close, then backup → download → apply. */
function simulateSync(inst: InstanceDetail, done: () => void) {
  inst.status = "syncing";
  const total = 30_000_000 + Math.floor(Math.random() * 40_000_000);
  const set = (stage: SyncProgress["stage"], percent: number) => {
    inst.progress = {
      stage,
      percent: Math.round(percent),
      bytesDone: Math.round((total * percent) / 100),
      bytesTotal: total,
      manualDownloads: [],
    };
    emit({
      type: "instance.progress",
      instanceId: inst.id,
      status: inst.status,
      progress: inst.progress,
    });
  };
  let percent = 0;
  const timer = setInterval(() => {
    if (inst.isRunning) return set("waiting_for_game_exit", percent);
    percent = Math.min(100, percent + 6 + Math.random() * 8);
    // VITE_MOCK_FAIL_RATE also makes some jobs fail part-way (nothing is changed).
    const fails = failRate > 0 && percent > 40 && Math.random() < failRate / 4;
    if (percent >= 100 || fails) {
      clearInterval(timer);
      if (fails)
        emit({
          type: "instance.jobFailed",
          instanceId: inst.id,
          message: "Couldn't download Sodium (the connection was lost).",
        });
      else done();
      inst.status = "ready";
      inst.progress = null;
      emit({
        type: "instance.progress",
        instanceId: inst.id,
        status: inst.status,
        progress: null,
      });
      emit({ type: "instance.changed", instanceId: inst.id });
      return;
    }
    set(percent < 15 ? "backup" : percent < 80 ? "download" : "apply", percent);
  }, 300);
}

function addBackup(
  inst: InstanceDetail,
  from: number | null,
  to: number | null,
  reason: Backup["reason"],
) {
  const files = Math.max(
    1,
    inst.content.filter((c) => c.isManaged).length >> 2,
  );
  inst.backups.unshift({
    id: newId(),
    reason,
    fromRevision: from,
    toRevision: to,
    fileCount: files,
    sizeBytes: files * 1_900_000,
    createdAt: nowIso(),
  });
  inst.backups = inst.backups.slice(0, 3); // default retention
}

/** Random friends coming online/going offline, so presence UI can be built against live changes. */
let presenceTimer: ReturnType<typeof setInterval> | null = null;
function startRandomEvents() {
  if (!randomEvents || presenceTimer) return;
  presenceTimer = setInterval(() => {
    if (!db.user || db.friends.length === 0) return;
    const f = db.friends[Math.floor(Math.random() * db.friends.length)];
    const next = (["offline", "online", "playing"] as const)[
      Math.floor(Math.random() * 3)
    ];
    f.presence =
      next === "playing"
        ? {
            state: "playing",
            instanceName: "Vanilla 1.21.4",
            minecraftVersion: "1.21.4",
            sharedInstanceId: null,
            updatedAt: nowIso(),
          }
        : {
            state: next,
            instanceName: null,
            minecraftVersion: null,
            sharedInstanceId: null,
            updatedAt: nowIso(),
          };
    emit({ type: "presence.changed", userId: f.user.id, presence: f.presence });
  }, 12_000);
}

function mergeChanges(
  revisions: { changes: RevisionChanges }[],
): RevisionChanges {
  return {
    added: revisions.flatMap((r) => r.changes.added),
    removed: revisions.flatMap((r) => r.changes.removed),
    updated: revisions.flatMap((r) => r.changes.updated),
  };
}

/**
 * What differs from the synced revision.
 * Owners/editors (their folder is the source of the next version):
 * - removed:     a shared file they disabled
 * - modeChanged: a shared file whose setting changed (e.g. Required → Optional, or → Only me)
 * - added:       a file not in the shared version that would now be shared (setting isn't "Only me")
 * Viewers: only disabling a *required* file counts. Skipping or removing optional items is their choice,
 * and their own extras are "Only me" by nature.
 */
function localChanges(inst: InstanceDetail) {
  const live = inst.content.filter((c) => c.installed);
  if (!inst.sync || inst.sync.role === "viewer") {
    const removed = live.filter(
      (c) => c.isManaged && c.shareMode === "required" && !c.enabled,
    );
    return {
      removed,
      modeChanged: [] as InstanceContent[],
      added: [] as InstanceContent[],
    };
  }
  const removed = live.filter((c) => c.isManaged && !c.enabled);
  const modeChanged = live.filter(
    (c) => c.isManaged && c.enabled && c.shareMode !== c.publishedShareMode,
  );
  const added = live.filter(
    (c) =>
      !c.isManaged &&
      c.enabled &&
      c.shareMode !== "private" &&
      c.shareMode !== c.publishedShareMode,
  );
  return { removed, modeChanged, added };
}

function refreshSyncState(inst: InstanceDetail) {
  const s = inst.sync;
  if (!s || s.state === "frozen") return;
  const { removed, modeChanged, added } = localChanges(inst);
  s.localChangeCount = removed.length + modeChanged.length + added.length;
  const behind = s.syncedRevision < s.latestRevision;
  s.state = s.localChangeCount
    ? behind
      ? "conflict"
      : "local_changes"
    : behind
      ? "update_available"
      : "up_to_date";
}

const MODE_LABEL: Record<ShareMode, string> = {
  required: "Required",
  optional: "Optional",
  private: "Only me",
};

/**
 * Applies share settings to an owner's/editor's folder as a new published version:
 * uploads unknown files that will be shared, marks what's in the version, and
 * returns what changed (for the revision history).
 */
function applyPublish(
  inst: InstanceDetail,
  modes: Record<Id, ShareMode>,
  confirmedRights: boolean,
): RevisionChanges {
  const user = me();
  const live = inst.content.filter((c) => c.installed);
  const modeOf = (c: InstanceContent) => modes[c.id] ?? c.shareMode;

  // Unknown files that will be shared have to be uploaded.
  const uploads = live.filter(
    (c) => c.source === "local" && c.enabled && modeOf(c) !== "private",
  );
  if (uploads.length && !confirmedRights)
    throw new ApiError(
      "invalid_input",
      "Confirm you have the right to share the files being uploaded",
    );
  const tooBig = uploads.find((c) => c.sizeBytes > user.storage.maxFileBytes);
  if (tooBig)
    throw new ApiError(
      "quota_exceeded",
      `${tooBig.name} is larger than the per-file limit. Set it to Only me.`,
    );
  const total = uploads.reduce((n, c) => n + c.sizeBytes, 0);
  if (user.storage.usedBytes + total > user.storage.quotaBytes)
    throw new ApiError(
      "quota_exceeded",
      "Not enough storage left. Set some files to Only me.",
    );
  user.storage.usedBytes += total;
  for (const c of uploads) c.source = "upload";

  const changes: RevisionChanges = { added: [], removed: [], updated: [] };
  for (const c of live) {
    c.shareMode = modeOf(c);
    if (!c.enabled) {
      // Disabled files aren't part of the shared version.
      if (c.isManaged) changes.removed.push(c.name);
      c.isManaged = false;
      c.publishedShareMode = null;
    } else if (c.shareMode === "private") {
      if (c.isManaged) changes.removed.push(c.name);
      c.isManaged = false;
      c.publishedShareMode = "private";
    } else {
      if (!c.isManaged)
        changes.added.push(
          c.shareMode === "optional" ? `${c.name} (optional)` : c.name,
        );
      else if (c.publishedShareMode && c.publishedShareMode !== c.shareMode)
        changes.updated.push({
          name: c.name,
          from: MODE_LABEL[c.publishedShareMode],
          to: MODE_LABEL[c.shareMode],
        });
      c.isManaged = true;
      c.publishedShareMode = c.shareMode;
    }
  }
  return changes;
}

/** The optional items in an owner's published version, as friends see them. */
const optionalItemsOf = (inst: InstanceDetail): OptionalItem[] =>
  inst.content
    .filter((c) => c.isManaged && c.shareMode === "optional")
    .map((c) => ({
      key: c.name,
      name: c.name,
      kind: c.kind,
      sizeBytes: c.sizeBytes,
    }));

const FOLDERS: Record<ContentKind, string> = {
  mod: "mods",
  resourcepack: "resourcepacks",
  shaderpack: "shaderpacks",
  datapack: "datapacks",
};

/** A file arriving from a shared version. The mock only knows names, so details are made up. */
function receivedItem(
  name: string,
  mode: ShareMode,
  installed: boolean,
  kind: ContentKind = "mod",
  sizeBytes = 900_000,
): InstanceContent {
  return {
    id: newId(),
    kind,
    source: "modrinth",
    projectId: null,
    versionId: null,
    name,
    versionLabel: "1.0.0",
    author: null,
    iconUrl: null,
    path: `${FOLDERS[kind]}/${name.toLowerCase().replace(/\W+/g, "-")}.${kind === "mod" ? "jar" : "zip"}`,
    sizeBytes,
    side: "both",
    enabled: true,
    isManaged: true,
    shareMode: mode,
    publishedShareMode: mode,
    installed,
  };
}

/** Materialise member rows for friends + group members, skipping people already in. */
function addMembers(
  s: SharedInstanceDetail,
  friendIds: Id[],
  groupIds: Id[],
  role: Exclude<MemberRole, "owner">,
) {
  const has = (uid: Id) => s.members.some((m) => m.user.id === uid);
  for (const f of db.friends.filter((f) => friendIds.includes(f.user.id))) {
    if (!has(f.user.id))
      s.members.push({
        user: f.user,
        role,
        status: "invited",
        viaGroup: null,
        lastSyncedRevision: null,
      });
  }
  for (const g of db.groups.filter((g) => groupIds.includes(g.id))) {
    if (!s.groups.some((l) => l.group.id === g.id))
      s.groups.push({ group: { id: g.id, name: g.name, icon: g.icon }, role });
    if (!g.sharedInstanceIds.includes(s.id)) g.sharedInstanceIds.push(s.id);
    for (const m of g.members) {
      if (!has(m.user.id))
        s.members.push({
          user: m.user,
          role,
          status: "invited",
          viaGroup: { id: g.id, name: g.name },
          lastSyncedRevision: null,
        });
    }
  }
}

function unlinkLocal(sharedId: Id) {
  const inst = db.instances.find((i) => i.sync?.sharedInstanceId === sharedId);
  if (!inst) return;
  inst.sync = null;
  // Optional items that were never downloaded have no file: drop them.
  inst.content = inst.content.filter((c) => c.installed);
  for (const c of inst.content) {
    c.isManaged = false;
    c.publishedShareMode = null;
  }
  emit({ type: "instance.changed", instanceId: inst.id });
}

// ── API ────────────────────────────────────────────────────────────────

export const mockApi: AtlasApi = {
  // ── Session & account ────────────────────────────────────────────────

  getSession: () => respond({ user: db.user, online: db.online }),

  async signIn() {
    if (!db.online) throw new ApiError("offline", "Can't reach Atlas servers");
    db.user = structuredClone(fx.me);
    emit({ type: "session.changed" });
    return respond({ user: db.user, online: db.online });
  },

  async signOut() {
    db.user = null;
    emit({ type: "session.changed" });
    return respond(undefined);
  },

  async linkIdentity(provider) {
    const user = online();
    if (!user.identities.some((i) => i.provider === provider)) {
      user.identities.push({
        provider,
        username: provider === "discord" ? user.username : "Player123",
        linkedAt: nowIso(),
      });
    }
    return respond(user);
  },

  async unlinkIdentity(provider) {
    const user = online();
    if (user.identities.length <= 1)
      throw new ApiError("conflict", "You need at least one way to sign in");
    user.identities = user.identities.filter((i) => i.provider !== provider);
    if (provider === "minecraft")
      Object.assign(user, { minecraftUuid: null, minecraftUsername: null });
    return respond(user);
  },

  async updateProfile(patch) {
    const user = online();
    if (patch.username !== undefined) {
      const name = patch.username.trim().toLowerCase();
      if (!/^[a-z0-9_]{3,16}$/.test(name))
        throw new ApiError("invalid_input", "3–16 letters, numbers or _");
      if (
        user.usernameChangeAvailableAt &&
        user.usernameChangeAvailableAt > nowIso()
      )
        throw new ApiError(
          "rate_limited",
          "You can change your username once every 30 days",
        );
      if (
        Object.values(fx.users).some(
          (u) => u.username === name && u.id !== user.id,
        )
      )
        throw new ApiError("conflict", "That username is taken");
      user.username = name;
      user.usernameChangeAvailableAt = new Date(
        Date.now() + 30 * 86_400_000,
      ).toISOString();
    }
    if (patch.displayName !== undefined) {
      const dn = patch.displayName.trim();
      if (dn.length < 1 || dn.length > 32)
        throw new ApiError("invalid_input", "1–32 characters");
      user.displayName = dn;
    }
    if (patch.appearOffline !== undefined)
      user.appearOffline = patch.appearOffline;
    return respond(user);
  },

  async resetFriendCode() {
    const user = online();
    user.friendCode = randomCode(8);
    user.friendLink = `${fx.SITE}/f/${user.friendCode}`;
    return respond(user);
  },

  async deleteAccount() {
    online();
    db.user = null;
    emit({ type: "session.changed" });
    return respond(undefined);
  },

  // ── Launchers ────────────────────────────────────────────────────────

  listLaunchers: () => respond(db.launchers),

  detectLaunchers: () => respond(db.launchers),

  async addLauncher(kind, rootPath) {
    if (!/^[A-Za-z]:\\/.test(rootPath))
      throw new ApiError(
        "invalid_input",
        "Choose the launcher's folder, e.g. D:\\PrismLauncher",
      );
    if (
      db.launchers.some(
        (l) => l.rootPath.toLowerCase() === rootPath.toLowerCase(),
      )
    )
      throw new ApiError("conflict", "That launcher is already added");
    const launcher: Launcher = {
      id: newId(),
      kind,
      name: fx.launchers.find((l) => l.kind === kind)?.name ?? kind,
      rootPath,
      instanceCount: 0,
      canCreateInstances:
        kind === "prism" || kind === "multimc" || kind === "modrinth",
      addedByUser: true,
    };
    db.launchers.push(launcher);
    emit({ type: "launchers.changed" });
    return respond(launcher);
  },

  async removeLauncher(id) {
    db.launchers = db.launchers.filter((l) => l.id !== id);
    emit({ type: "launchers.changed" });
    return respond(undefined);
  },

  listDetectedInstances: () =>
    respond([
      ...db.instances
        .filter((i) => i.status !== "missing" && i.launcher.kind !== "folder")
        .map((i): DetectedInstance => ({
          launcherKind: i.launcher.kind,
          launcherInstanceId: i.name,
          name: i.name,
          gameDir: i.gameDir,
          minecraftVersion: i.minecraftVersion,
          loader: i.loader,
          loaderVersion: i.loaderVersion,
          instanceId: i.id,
        })),
      ...db.untracked,
    ]),

  // ── Instances (tracked folders) ──────────────────────────────────────

  listInstances: () => respond(db.instances.map(toSummary)),

  getInstance: async (id) => respond(findInstance(id)),

  async trackInstances(refs) {
    const added: InstanceDetail[] = [];
    for (const ref of refs) {
      const d = db.untracked.find(
        (u) =>
          u.launcherKind === ref.launcherKind &&
          u.launcherInstanceId === ref.launcherInstanceId,
      );
      if (!d) continue;
      const fromFixture = fx.instances.find((i) => i.gameDir === d.gameDir);
      const inst: InstanceDetail = fromFixture
        ? {
            ...structuredClone(fromFixture),
            sync: null,
            trackedAt: nowIso(),
            content: fromFixture.content
              .filter((c) => c.installed)
              .map((c) => ({
                ...c,
                isManaged: false,
                publishedShareMode: null,
              })),
            backups: [],
          }
        : {
            id: newId(),
            name: d.name,
            icon: null,
            launcher: {
              kind: d.launcherKind,
              name:
                db.launchers.find((l) => l.kind === d.launcherKind)?.name ??
                "Folder",
            },
            gameDir: d.gameDir,
            minecraftVersion: d.minecraftVersion,
            loader: d.loader,
            loaderVersion: d.loaderVersion,
            status: "ready",
            isFavorite: false,
            hideActivity: false,
            contentCount: 0,
            lastPlayedAt: null,
            totalPlaytimeSeconds: null,
            isRunning: false,
            trackedAt: nowIso(),
            lastScannedAt: nowIso(),
            sync: null,
            progress: null,
            content: [],
            backups: [],
          };
      db.untracked = db.untracked.filter((u) => u !== d);
      db.instances.push(inst);
      added.push(inst);
    }
    return respond(added.map(toSummary));
  },

  async trackFolder(gameDir) {
    if (!/^[A-Za-z]:\\/.test(gameDir))
      throw new ApiError(
        "invalid_input",
        "Choose a folder that contains mods or config",
      );
    if (
      db.instances.some(
        (i) => i.gameDir.toLowerCase() === gameDir.toLowerCase(),
      )
    )
      throw new ApiError("conflict", "That folder is already tracked");
    const inst: InstanceDetail = {
      id: newId(),
      name: gameDir.split("\\").filter(Boolean).pop() ?? "Folder",
      icon: null,
      launcher: { kind: "folder", name: "Folder" },
      gameDir,
      minecraftVersion: null, // no launcher metadata to read; Atlas guesses from mods where it can
      loader: null,
      loaderVersion: null,
      status: "ready",
      isFavorite: false,
      hideActivity: false,
      contentCount: 0,
      lastPlayedAt: null,
      totalPlaytimeSeconds: null,
      isRunning: false,
      trackedAt: nowIso(),
      lastScannedAt: nowIso(),
      sync: null,
      progress: null,
      content: [],
      backups: [],
    };
    db.instances.push(inst);
    return respond(toSummary(inst));
  },

  async untrackInstance(id) {
    const inst = findInstance(id);
    if (inst.status === "syncing")
      throw new ApiError("conflict", "Wait for the update to finish");
    db.instances = db.instances.filter((i) => i.id !== id);
    for (const s of db.shared)
      if (s.localInstanceId === id) s.localInstanceId = null;
    if (inst.status !== "missing" && inst.launcher.kind !== "folder") {
      db.untracked.push({
        launcherKind: inst.launcher.kind,
        launcherInstanceId: inst.name,
        name: inst.name,
        gameDir: inst.gameDir,
        minecraftVersion: inst.minecraftVersion,
        loader: inst.loader,
        loaderVersion: inst.loaderVersion,
        instanceId: null,
      });
    }
    return respond(undefined);
  },

  async updateInstance(id, patch) {
    const inst = findInstance(id);
    if (patch.isFavorite !== undefined) inst.isFavorite = patch.isFavorite;
    if (patch.hideActivity !== undefined)
      inst.hideActivity = patch.hideActivity;
    return respond(toSummary(inst));
  },

  async rescanInstance(id) {
    const inst = findInstance(id);
    inst.lastScannedAt = nowIso();
    return respond(toSummary(inst));
  },

  async openInLauncher(id) {
    const inst = findInstance(id);
    if (inst.status === "missing")
      throw new ApiError("conflict", "The folder is missing");
    if (inst.launcher.kind === "folder")
      throw new ApiError(
        "conflict",
        "This folder isn't from a launcher Atlas knows",
      );
    return respond(undefined);
  },

  openInstanceFolder: async (id) => {
    if (findInstance(id).status === "missing")
      throw new ApiError("conflict", "The folder is missing");
    return respond(undefined);
  },

  async setContentEnabled(instanceId, contentId, enabled) {
    const inst = findInstance(instanceId);
    if (inst.isRunning) throw new ApiError("conflict", "Close the game first");
    const item = inst.content.find((c) => c.id === contentId);
    if (!item) throw new ApiError("not_found", "Content not found");
    item.enabled = enabled;
    refreshSyncState(inst);
    return respond(undefined);
  },

  async restoreBackup(instanceId, backupId) {
    const inst = findInstance(instanceId);
    if (inst.status !== "ready")
      throw new ApiError("conflict", `Instance is ${inst.status}`);
    const b = inst.backups.find((x) => x.id === backupId);
    if (!b) throw new ApiError("not_found", "Backup not found");
    simulateSync(inst, () => {
      addBackup(
        inst,
        inst.sync?.syncedRevision ?? null,
        b.fromRevision,
        "before_restore",
      );
      if (inst.sync && b.fromRevision != null) {
        inst.sync.syncedRevision = b.fromRevision;
        refreshSyncState(inst);
      }
    });
    return respond(toSummary(inst));
  },

  async setShareMode(instanceId, contentId, mode) {
    const inst = findInstance(instanceId);
    if (inst.sync?.role === "viewer")
      throw new ApiError(
        "forbidden",
        "Only the owner and editors choose how files are shared",
      );
    const item = inst.content.find((c) => c.id === contentId);
    if (!item) throw new ApiError("not_found", "Content not found");
    item.shareMode = mode;
    refreshSyncState(inst);
    return respond(undefined);
  },

  async setOptionalInstalled(instanceId, contentId, installed) {
    const inst = findInstance(instanceId);
    if (inst.isRunning) throw new ApiError("conflict", "Close the game first");
    const item = inst.content.find((c) => c.id === contentId);
    if (!item) throw new ApiError("not_found", "Content not found");
    if (!item.isManaged || item.shareMode !== "optional")
      throw new ApiError(
        "invalid_input",
        "Only optional items can be added or removed this way",
      );
    item.installed = installed;
    item.enabled = true;
    refreshSyncState(inst);
    emit({ type: "instance.changed", instanceId });
    return respond(undefined);
  },

  // ── Sharing ──────────────────────────────────────────────────────────

  async listSharedInstances() {
    me();
    return respond(
      db.shared.filter((s) => s.myStatus !== "declined").map(toSharedSummary),
    );
  },

  getSharedInstance: async (id) => {
    me();
    return respond(findShared(id));
  },

  async acceptInvite(sharedInstanceId, launcherId, installOptional) {
    const user = online();
    const s = findShared(sharedInstanceId);
    if (s.myStatus !== "invited")
      throw new ApiError("conflict", "No pending invite");
    const launcher = db.launchers.find((l) => l.id === launcherId);
    if (!launcher) throw new ApiError("not_found", "Launcher not found");
    if (!launcher.canCreateInstances)
      throw new ApiError(
        "invalid_input",
        `Atlas can't create instances in ${launcher.name} yet`,
      );
    const name = db.instances.some((i) => i.name === s.name)
      ? `${s.name} (2)`
      : s.name;
    const self = s.members.find((m) => m.user.id === user.id);
    s.myStatus = "active";
    if (self)
      Object.assign(self, {
        status: "active",
        lastSyncedRevision: s.latestRevision,
      });
    const inst: InstanceDetail = {
      id: newId(),
      name,
      icon: s.icon,
      launcher: { kind: launcher.kind, name: launcher.name },
      gameDir: fx.gameDirFor(launcher.kind, name),
      minecraftVersion: s.minecraftVersion,
      loader: s.loader,
      loaderVersion: s.loaderVersion,
      status: "syncing",
      isFavorite: false,
      hideActivity: false,
      contentCount: 0,
      lastPlayedAt: null,
      totalPlaytimeSeconds: null,
      isRunning: false,
      trackedAt: nowIso(),
      lastScannedAt: nowIso(),
      sync: {
        sharedInstanceId: s.id,
        role: s.myRole,
        owner: s.owner,
        syncedRevision: s.latestRevision,
        latestRevision: s.latestRevision,
        state: "up_to_date",
        localChangeCount: 0,
      },
      progress: null,
      content: [],
      backups: [],
    };
    db.instances.unshift(inst);
    s.localInstanceId = inst.id;
    simulateSync(inst, () => {
      // Required files: everything any version added (revisions are newest first, so
      // collect from all of them), minus the optional ones.
      const optionalNames = new Set(s.optionalItems.map((o) => o.name));
      const required = [
        ...new Set(s.revisions.flatMap((r) => r.changes.added)),
      ].filter((n) => !n.endsWith(" (optional)") && !optionalNames.has(n));
      inst.content = [
        ...required.map((n) => receivedItem(n, "required", true)),
        // Optional files: listed either way, downloaded only if the user ticked them.
        ...s.optionalItems.map((o) =>
          receivedItem(
            o.name,
            "optional",
            installOptional.includes(o.key),
            o.kind,
            o.sizeBytes,
          ),
        ),
      ];
      inst.contentCount = inst.content.filter((c) => c.installed).length;
    });
    emit({ type: "shared.changed", sharedInstanceId });
    return respond(toSummary(inst));
  },

  async acceptInviteInto(sharedInstanceId, instanceId, installOptional) {
    const user = online();
    const s = findShared(sharedInstanceId);
    if (s.myStatus !== "invited")
      throw new ApiError("conflict", "No pending invite");
    const inst = findInstance(instanceId);
    if (inst.sync)
      throw new ApiError(
        "conflict",
        "That instance is already shared. Pick another one.",
      );
    const self = s.members.find((m) => m.user.id === user.id);
    s.myStatus = "active";
    if (self)
      Object.assign(self, {
        status: "active",
        lastSyncedRevision: s.latestRevision,
      });
    inst.status = "syncing";
    inst.sync = {
      sharedInstanceId: s.id,
      role: s.myRole,
      owner: s.owner,
      syncedRevision: s.latestRevision,
      latestRevision: s.latestRevision,
      state: "up_to_date",
      localChangeCount: 0,
    };
    s.localInstanceId = inst.id;
    simulateSync(inst, () => {
      const optionalNames = new Set(s.optionalItems.map((o) => o.name));
      const required = [
        ...new Set(s.revisions.flatMap((r) => r.changes.added)),
      ].filter((n) => !n.endsWith(" (optional)") && !optionalNames.has(n));
      // The user's own files stay; the shared ones are added next to them.
      inst.content = [
        ...inst.content,
        ...required.map((n) => receivedItem(n, "required", true)),
        ...s.optionalItems.map((o) =>
          receivedItem(
            o.name,
            "optional",
            installOptional.includes(o.key),
            o.kind,
            o.sizeBytes,
          ),
        ),
      ];
      inst.contentCount = inst.content.filter((c) => c.installed).length;
    });
    emit({ type: "shared.changed", sharedInstanceId });
    return respond(toSummary(inst));
  },

  async declineInvite(sharedInstanceId) {
    const user = online();
    const s = findShared(sharedInstanceId);
    if (s.myStatus !== "invited")
      throw new ApiError("conflict", "No pending invite");
    s.myStatus = "declined";
    const self = s.members.find((m) => m.user.id === user.id);
    if (self) self.status = "declined";
    emit({ type: "shared.changed", sharedInstanceId });
    return respond(undefined);
  },

  async preparePublish(instanceId) {
    const user = online();
    const inst = findInstance(instanceId);
    if (inst.sync)
      requireRole(findShared(inst.sync.sharedInstanceId), "owner", "editor");
    return respond({
      baseRevision: inst.sync?.latestRevision ?? null,
      // Disabled files aren't offered: they won't be in the shared version either way.
      items: inst.content
        .filter((c) => c.installed && c.enabled)
        .map((c) => ({
          contentId: c.id,
          name: c.name,
          iconUrl: c.iconUrl,
          kind: c.kind,
          source: c.source,
          path: c.path,
          sizeBytes: c.sizeBytes,
          isNew: c.publishedShareMode === null,
          shareMode: c.shareMode,
          publishedShareMode: c.publishedShareMode,
          needsUpload: c.source === "local",
          alreadyUploaded: false,
        })),
      removed: inst.content
        .filter((c) => c.installed && c.isManaged && !c.enabled)
        .map((c) => c.name),
      manualDownloads: [],
      storage: user.storage,
    });
  },

  async shareInstance(instanceId, input) {
    online();
    const inst = findInstance(instanceId);
    if (inst.sync) throw new ApiError("conflict", "Instance is already shared");
    if (inst.status !== "ready")
      throw new ApiError("conflict", `Instance is ${inst.status}`);
    const { minecraftVersion, loader } = inst;
    if (!minecraftVersion || !loader)
      throw new ApiError(
        "invalid_input",
        "Atlas couldn't tell which Minecraft version and loader this folder uses",
      );
    const changes = applyPublish(
      inst,
      input.shareModes,
      input.confirmedUploadRights,
    );
    const owner = publicMe();
    const shared: SharedInstanceDetail = {
      id: newId(),
      name: inst.name,
      description: input.description ?? null,
      icon: inst.icon,
      owner,
      minecraftVersion,
      loader,
      loaderVersion: inst.loaderVersion,
      latestRevision: 1,
      contentCount: inst.content.filter((c) => c.isManaged).length,
      sizeBytes: inst.content
        .filter((c) => c.isManaged)
        .reduce((n, c) => n + c.sizeBytes, 0),
      updatedAt: nowIso(),
      frozen: false,
      myRole: "owner",
      myStatus: "active",
      myViaGroup: null,
      invitedBy: null,
      invitedAt: null,
      localInstanceId: inst.id,
      optionalItems: optionalItemsOf(inst),
      groups: [],
      members: [
        {
          user: owner,
          role: "owner",
          status: "active",
          viaGroup: null,
          lastSyncedRevision: 1,
        },
      ],
      revisions: [
        {
          revision: 1,
          changelog: "Initial version",
          createdBy: owner,
          createdAt: nowIso(),
          changes: { added: changes.added, removed: [], updated: [] },
        },
      ],
    };
    addMembers(shared, input.friendIds, input.groupIds, input.role);
    db.shared.unshift(shared);
    inst.sync = {
      sharedInstanceId: shared.id,
      role: "owner",
      owner,
      syncedRevision: 1,
      latestRevision: 1,
      state: "up_to_date",
      localChangeCount: 0,
    };
    refreshSyncState(inst);
    return respond(toSharedSummary(shared));
  },

  async publishInstance(instanceId, input) {
    online();
    const inst = findInstance(instanceId);
    if (!inst.sync) throw new ApiError("conflict", "Instance is not shared");
    const s = findShared(inst.sync.sharedInstanceId);
    requireRole(s, "owner", "editor");
    if (inst.sync.syncedRevision < s.latestRevision)
      throw new ApiError(
        "conflict",
        "Someone published a newer version. Update first.",
      );
    // Work on a copy so a refused publish (rights, quota, nothing to do) changes nothing.
    const draft = structuredClone(inst);
    const changes = applyPublish(
      draft,
      input.shareModes,
      input.confirmedUploadRights,
    );
    if (
      !changes.added.length &&
      !changes.removed.length &&
      !changes.updated.length
    )
      throw new ApiError("invalid_input", "Nothing to publish");
    inst.content = draft.content;
    s.latestRevision += 1;
    s.updatedAt = nowIso();
    s.contentCount = inst.content.filter((c) => c.isManaged).length;
    s.optionalItems = optionalItemsOf(inst);
    s.revisions.unshift({
      revision: s.latestRevision,
      changelog: input.changelog,
      createdBy: publicMe(),
      createdAt: nowIso(),
      changes,
    });
    const self = s.members.find((m) => m.user.id === me().id);
    if (self) self.lastSyncedRevision = s.latestRevision;
    inst.sync = {
      ...inst.sync,
      syncedRevision: s.latestRevision,
      latestRevision: s.latestRevision,
    };
    refreshSyncState(inst);
    emit({ type: "shared.changed", sharedInstanceId: s.id });
    return respond(toSummary(inst));
  },

  async previewSync(instanceId) {
    const inst = findInstance(instanceId);
    if (!inst.sync) throw new ApiError("conflict", "Instance is not shared");
    const s = findShared(inst.sync.sharedInstanceId);
    const newer = s.revisions
      .filter((r) => r.revision > inst.sync!.syncedRevision)
      .reverse();
    const changes = mergeChanges(newer);
    // Optional items show up separately (unticked), not as plain additions.
    changes.added = changes.added.filter((n) => !n.endsWith(" (optional)"));
    return respond({
      fromRevision: inst.sync.syncedRevision,
      toRevision: s.latestRevision,
      changes,
      downloadBytes: newer.length * 14_000_000,
      manualDownloads: [],
      overwrittenLocalChanges: localChanges(inst).removed.map((c) => c.name),
      // One example conflict so the UI has to handle it.
      configConflicts:
        inst.id === fx.ids.cobblemon && newer.length
          ? [
              {
                path: "config/cobblemon/main.json",
                mine: '"shinyRate": 2048',
                theirs: '"shinyRate": 4096',
              },
            ]
          : [],
      launcherChange: null,
      gameRunning: inst.isRunning,
      newOptional: s.optionalItems.filter(
        (o) => !inst.content.some((c) => c.name === o.name),
      ),
    });
  },

  async syncInstance(instanceId, options) {
    online();
    const inst = findInstance(instanceId);
    if (!inst.sync) throw new ApiError("conflict", "Instance is not shared");
    if (inst.sync.state === "frozen")
      throw new ApiError("conflict", "This instance is frozen");
    if (inst.status !== "ready")
      throw new ApiError("conflict", `Instance is ${inst.status}`);
    const s = findShared(inst.sync.sharedInstanceId);
    const from = inst.sync.syncedRevision;
    const chosen = options?.installOptional ?? [];
    const newOptional = s.optionalItems.filter(
      (o) => !inst.content.some((c) => c.name === o.name),
    );
    simulateSync(inst, () => {
      addBackup(inst, from, s.latestRevision, "before_update");
      for (const c of inst.content) if (c.isManaged) c.enabled = true;
      // New optional items are listed either way; downloaded only if ticked.
      for (const o of newOptional)
        inst.content.push(
          receivedItem(
            o.name,
            "optional",
            chosen.includes(o.key),
            o.kind,
            o.sizeBytes,
          ),
        );
      inst.sync = {
        ...inst.sync!,
        syncedRevision: s.latestRevision,
        latestRevision: s.latestRevision,
      };
      refreshSyncState(inst);
      const self = s.members.find((m) => m.user.id === me().id);
      if (self) self.lastSyncedRevision = s.latestRevision;
    });
    return respond(toSummary(inst)); // status "syncing"; watch instance.progress events
  },

  async revertLocalChanges(instanceId) {
    const inst = findInstance(instanceId);
    if (!inst.sync) throw new ApiError("conflict", "Instance is not shared");
    if (inst.isRunning) throw new ApiError("conflict", "Close the game first");
    addBackup(
      inst,
      inst.sync.syncedRevision,
      inst.sync.syncedRevision,
      "before_revert",
    );
    for (const c of inst.content) {
      if (!c.isManaged) continue;
      c.enabled = true;
      if (c.publishedShareMode) c.shareMode = c.publishedShareMode;
    }
    refreshSyncState(inst);
    return respond(toSummary(inst));
  },

  async updateSharedInstance(id, patch) {
    online();
    const s = findShared(id);
    requireRole(s, "owner", "editor");
    Object.assign(s, patch, { updatedAt: nowIso() });
    return respond(toSharedSummary(s));
  },

  async inviteToSharedInstance(id, input) {
    online();
    const s = findShared(id);
    requireRole(s, "owner", "editor");
    addMembers(s, input.friendIds, input.groupIds, input.role);
    return respond(toSharedSummary(s));
  },

  async setSharedMemberRole(id, userId, role) {
    online();
    const s = findShared(id);
    requireRole(s, "owner");
    const m = s.members.find((m) => m.user.id === userId);
    if (!m || m.role === "owner")
      throw new ApiError("not_found", "Member not found");
    m.role = role;
    return respond(toSharedSummary(s));
  },

  async removeSharedMember(id, userId) {
    online();
    const s = findShared(id);
    requireRole(s, "owner");
    if (userId === me().id)
      throw new ApiError(
        "conflict",
        "Transfer or archive the instance instead",
      );
    s.members = s.members.filter((m) => m.user.id !== userId);
    return respond(toSharedSummary(s));
  },

  async unshareFromGroup(id, groupId) {
    online();
    const s = findShared(id);
    requireRole(s, "owner", "editor");
    s.groups = s.groups.filter((g) => g.group.id !== groupId);
    s.members = s.members.filter((m) => m.viaGroup?.id !== groupId);
    const g = db.groups.find((g) => g.id === groupId);
    if (g)
      g.sharedInstanceIds = g.sharedInstanceIds.filter((sid) => sid !== id);
    return respond(toSharedSummary(s));
  },

  async transferSharedInstance(id, userId) {
    online();
    const s = findShared(id);
    requireRole(s, "owner");
    const target = s.members.find(
      (m) => m.user.id === userId && m.status === "active",
    );
    if (!target)
      throw new ApiError("invalid_input", "New owner must be an active member");
    const self = s.members.find((m) => m.role === "owner")!;
    self.role = "editor";
    target.role = "owner";
    target.viaGroup = null; // ownership is always direct
    s.owner = target.user;
    s.myRole = "editor";
    const inst = db.instances.find((i) => i.id === s.localInstanceId);
    if (inst?.sync)
      inst.sync = { ...inst.sync, role: "editor", owner: target.user };
    return respond(toSharedSummary(s));
  },

  async leaveSharedInstance(id) {
    online();
    const s = findShared(id);
    if (s.myRole === "owner")
      throw new ApiError(
        "conflict",
        "Transfer or archive the instance instead",
      );
    db.shared = db.shared.filter((x) => x.id !== id);
    unlinkLocal(id);
    return respond(undefined);
  },

  async archiveSharedInstance(id) {
    online();
    requireRole(findShared(id), "owner");
    db.shared = db.shared.filter((x) => x.id !== id);
    for (const g of db.groups)
      g.sharedInstanceIds = g.sharedInstanceIds.filter((sid) => sid !== id);
    unlinkLocal(id);
    return respond(undefined);
  },

  // ── Friends ──────────────────────────────────────────────────────────

  listFriends: async () => {
    me();
    return respond(db.friends);
  },

  listFriendRequests: async () => {
    me();
    return respond(db.requests);
  },

  async previewFriendCode(code) {
    online();
    const user = fx.friendCodes[normalizeCode(code)];
    if (!user)
      throw new ApiError("not_found", "That friend code doesn't exist");
    return respond(user);
  },

  async sendFriendRequest(code) {
    const self = online();
    const user = fx.friendCodes[normalizeCode(code)];
    if (!user)
      throw new ApiError("not_found", "That friend code doesn't exist");
    if (user.id === self.id)
      throw new ApiError("invalid_input", "That's your own code");
    if (db.friends.some((f) => f.user.id === user.id))
      throw new ApiError("conflict", "You're already friends");
    if (
      db.requests.some(
        (r) => r.user.id === user.id && r.direction === "outgoing",
      )
    )
      throw new ApiError("conflict", "Request already sent");
    // They already asked you: using their code accepts.
    const incoming = db.requests.find(
      (r) => r.user.id === user.id && r.direction === "incoming",
    );
    if (incoming) {
      db.requests = db.requests.filter((r) => r !== incoming);
      db.friends.push({
        user,
        friendsSince: nowIso(),
        presence: {
          state: "online",
          instanceName: null,
          minecraftVersion: null,
          sharedInstanceId: null,
          updatedAt: nowIso(),
        },
      });
      emit({ type: "friends.changed" });
      return respond({ ...incoming, direction: "outgoing" as const });
    }
    const request: FriendRequest = {
      direction: "outgoing",
      user,
      createdAt: nowIso(),
    };
    db.requests.push(request);
    return respond(request);
  },

  async respondToFriendRequest(userId, accept) {
    online();
    const request = db.requests.find(
      (r) => r.user.id === userId && r.direction === "incoming",
    );
    if (!request) throw new ApiError("not_found", "No pending request");
    db.requests = db.requests.filter((r) => r !== request);
    if (accept) {
      db.friends.push({
        user: request.user,
        friendsSince: nowIso(),
        presence: {
          state: "online",
          instanceName: null,
          minecraftVersion: null,
          sharedInstanceId: null,
          updatedAt: nowIso(),
        },
      });
    }
    emit({ type: "friends.changed" });
    return respond(undefined);
  },

  async cancelFriendRequest(userId) {
    online();
    db.requests = db.requests.filter(
      (r) => !(r.user.id === userId && r.direction === "outgoing"),
    );
    return respond(undefined);
  },

  async removeFriend(userId) {
    online();
    db.friends = db.friends.filter((f) => f.user.id !== userId);
    emit({ type: "friends.changed" });
    return respond(undefined);
  },

  async blockUser(userId) {
    online();
    db.friends = db.friends.filter((f) => f.user.id !== userId);
    db.requests = db.requests.filter((r) => r.user.id !== userId);
    emit({ type: "friends.changed" });
    return respond(undefined);
  },

  // The browser mock is never opened with a link.
  takeOpenedLink: async () => null,

  // ── Groups ───────────────────────────────────────────────────────────

  listGroups: async () => {
    me();
    return respond(db.groups.map(toGroup));
  },

  async getGroup(id) {
    me();
    const g = findGroup(id);
    return respond(g.myRole === "member" ? { ...g, invites: [] } : g);
  },

  async createGroup(input) {
    online();
    const name = input.name.trim();
    if (name.length < 1 || name.length > 32)
      throw new ApiError("invalid_input", "1–32 characters");
    const g: GroupDetail = {
      id: newId(),
      name,
      icon: input.icon ?? null,
      myRole: "owner",
      members: [{ user: publicMe(), role: "owner", joinedAt: nowIso() }],
      sharedInstanceIds: [],
      createdAt: nowIso(),
      invites: [],
    };
    db.groups.push(g);
    return respond(toGroup(g));
  },

  async updateGroup(id, patch) {
    online();
    const g = findGroup(id);
    if (g.myRole === "member")
      throw new ApiError(
        "forbidden",
        "Only owners and admins can edit the group",
      );
    Object.assign(g, patch);
    emit({ type: "groups.changed", groupId: id });
    return respond(toGroup(g));
  },

  async deleteGroup(id) {
    online();
    if (findGroup(id).myRole !== "owner")
      throw new ApiError("forbidden", "Only the owner can delete the group");
    db.groups = db.groups.filter((g) => g.id !== id);
    for (const s of db.shared) {
      s.groups = s.groups.filter((l) => l.group.id !== id);
      s.members = s.members.filter((m) => m.viaGroup?.id !== id);
    }
    emit({ type: "groups.changed", groupId: id });
    return respond(undefined);
  },

  async createGroupInvite(groupId, input) {
    online();
    const g = findGroup(groupId);
    if (g.myRole === "member")
      throw new ApiError(
        "forbidden",
        "Only owners and admins can create invites",
      );
    const code = randomCode(10);
    const invite = {
      code,
      link: `${fx.SITE}/g/${code}`,
      createdBy: publicMe(),
      maxUses: input.maxUses,
      uses: 0,
      expiresAt: input.expiresInHours
        ? new Date(Date.now() + input.expiresInHours * 3_600_000).toISOString()
        : null,
      createdAt: nowIso(),
    };
    g.invites.push(invite);
    return respond(invite);
  },

  async revokeGroupInvite(groupId, code) {
    online();
    const g = findGroup(groupId);
    if (g.myRole === "member")
      throw new ApiError(
        "forbidden",
        "Only owners and admins can revoke invites",
      );
    g.invites = g.invites.filter((i) => i.code !== code);
    return respond(undefined);
  },

  async previewGroupInvite(code) {
    online();
    const c = normalizeCode(code);
    const info = fx.groupInviteCodes[c];
    if (!info)
      throw new ApiError("not_found", "This invite is invalid or has expired");
    return respond({
      code: c,
      groupId: info.groupId ?? "pending",
      groupName: info.name,
      groupIcon: info.icon,
      memberCount: info.memberCount,
      invitedBy: info.invitedBy,
      alreadyMember:
        !!info.groupId && db.groups.some((g) => g.id === info.groupId),
    });
  },

  async joinGroup(code) {
    online();
    const info = fx.groupInviteCodes[normalizeCode(code)];
    if (!info)
      throw new ApiError("not_found", "This invite is invalid or has expired");
    if (info.groupId && db.groups.some((g) => g.id === info.groupId))
      throw new ApiError("conflict", "You're already in this group");
    const g: GroupDetail = {
      id: info.groupId ?? newId(),
      name: info.name,
      icon: info.icon,
      myRole: "member",
      members: [
        { user: info.invitedBy, role: "owner", joinedAt: nowIso() },
        { user: publicMe(), role: "member", joinedAt: nowIso() },
      ],
      sharedInstanceIds: [],
      createdAt: nowIso(),
      invites: [],
    };
    db.groups.push(g);
    emit({ type: "groups.changed", groupId: g.id });
    return respond(toGroup(g));
  },

  async setGroupMemberRole(groupId, userId, role) {
    online();
    const g = findGroup(groupId);
    if (g.myRole !== "owner")
      throw new ApiError("forbidden", "Only the owner can change roles");
    const m = g.members.find((m) => m.user.id === userId);
    if (!m || m.role === "owner")
      throw new ApiError("not_found", "Member not found");
    m.role = role;
    return respond(toGroup(g));
  },

  async removeGroupMember(groupId, userId) {
    online();
    const g = findGroup(groupId);
    const self = me().id;
    if (userId === self) {
      if (g.myRole === "owner")
        throw new ApiError("conflict", "Transfer ownership before leaving");
      db.groups = db.groups.filter((x) => x.id !== groupId);
      for (const s of db.shared) {
        if (s.myViaGroup?.id === groupId && s.myStatus === "invited")
          s.myStatus = "declined";
        s.members = s.members.filter(
          (m) => !(m.user.id === self && m.viaGroup?.id === groupId),
        );
      }
    } else {
      const target = g.members.find((m) => m.user.id === userId);
      if (!target) throw new ApiError("not_found", "Member not found");
      const allowed =
        g.myRole === "owner" ||
        (g.myRole === "admin" && target.role === "member");
      if (!allowed)
        throw new ApiError("forbidden", "You can't remove this member");
      g.members = g.members.filter((m) => m.user.id !== userId);
    }
    emit({ type: "groups.changed", groupId });
    return respond(undefined);
  },

  async transferGroup(groupId, userId) {
    online();
    const g = findGroup(groupId);
    if (g.myRole !== "owner")
      throw new ApiError("forbidden", "Only the owner can transfer the group");
    const target = g.members.find((m) => m.user.id === userId);
    if (!target) throw new ApiError("not_found", "Member not found");
    g.members.find((m) => m.role === "owner")!.role = "admin";
    target.role = "owner";
    g.myRole = "admin";
    return respond(toGroup(g));
  },

  // ── Activity ─────────────────────────────────────────────────────────

  listActivity: async () => {
    me();
    return respond(db.activity);
  },

  async markActivityRead(ids) {
    const readAt = nowIso();
    for (const a of db.activity)
      if ((ids === "all" || ids.includes(a.id)) && !a.readAt) a.readAt = readAt;
    return respond(undefined);
  },

  // ── Real-time ────────────────────────────────────────────────────────

  subscribe(listener) {
    listeners.add(listener);
    startRandomEvents();
    return () => listeners.delete(listener);
  },
};

/** Dev-only hooks for poking the mock from the browser console. */
export const mockControls = {
  setRunning(instanceId: Id, isRunning: boolean) {
    const inst = findInstance(instanceId);
    inst.isRunning = isRunning;
    emit({ type: "instance.running", instanceId, isRunning });
  },
  emit,
};
if (typeof window !== "undefined")
  (window as unknown as { __atlasMock: typeof mockControls }).__atlasMock =
    mockControls;
