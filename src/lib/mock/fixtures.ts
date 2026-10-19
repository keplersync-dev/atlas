// Mock data for building the UI without a backend.
// Every object matches src/lib/types.ts exactly. Timestamps are relative to page load so
// "2 hours ago" stays true. Hashes, version ids and download urls are fake.

import type {
  ActivityItem,
  Backup,
  DetectedInstance,
  Friend,
  FriendRequest,
  GroupDetail,
  GroupInvite,
  GroupMember,
  InstanceContent,
  InstanceDetail,
  Launcher,
  Me,
  PublicUser,
  RevisionSummary,
  SharedInstanceDetail,
  SharedMember,
} from "$lib/types";

// ── Helpers ────────────────────────────────────────────────────────────

const now = Date.now();
const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const MB = 1024 * 1024;
const ago = (ms: number) => new Date(now - ms).toISOString();
const ahead = (ms: number) => new Date(now + ms).toISOString();

/** Readable UUID-shaped ids: id(7) -> "0192a000-0000-7000-8000-000000000007". */
const id = (n: number) =>
  `0192a000-0000-7000-8000-${n.toString().padStart(12, "0")}`;

let seq = 1000;

export const SITE = "https://atlas.example";

// ── People ─────────────────────────────────────────────────────────────

const person = (
  n: number,
  username: string,
  displayName: string,
  mc: string | null,
): PublicUser => ({
  id: id(n),
  username,
  displayName,
  avatarUrl: null,
  minecraftUuid: mc
    ? `${n.toString(16).padStart(8, "0")}-1111-4222-8333-444455556666`
    : null,
  minecraftUsername: mc,
});

export const users = {
  me: person(1, "keplerhaloxx", "Alex", "KeplerHaloxx"),
  mia: person(2, "mia_builds", "Mia", "MiaBuilds"),
  jonah: person(3, "jonahcraft", "Jonah", "JonahCraft"),
  sam: person(4, "samuraisam", "Sam", "SamuraiSam_"),
  priya: person(5, "pri_ya", "Priya", "Priyaaa"),
  leo: person(6, "leo_redstone", "Leo", "LeoRedstone"),
  ava: person(7, "avaexe", "Ava", null), // Discord-only account
  noah: person(8, "noahplays", "Noah", "NoahPlays"),
  zoe: person(9, "zoe_mines", "Zoe", "ZoeMines"),
};

/** Friend codes that resolve in the mock (try them in the "Add friend" box). */
export const friendCodes: Record<string, PublicUser> = {
  "7KQ29XMB": users.me,
  NQ4RT8ZD: users.noah,
  ZM3W6PKA: users.zoe,
  B7H2XC9Q: person(10, "builder_bea", "Bea", "BuilderBea"),
};

export const me: Me = {
  ...users.me,
  friendCode: "7KQ29XMB",
  friendLink: `${SITE}/f/7KQ29XMB`,
  identities: [
    {
      provider: "minecraft",
      username: "KeplerHaloxx",
      linkedAt: ago(120 * DAY),
    },
    { provider: "discord", username: "keplerhaloxx", linkedAt: ago(90 * DAY) },
  ],
  appearOffline: false,
  usernameChangeAvailableAt: null,
  storage: {
    usedBytes: 118 * MB,
    quotaBytes: 500 * MB,
    maxFileBytes: 100 * MB,
  },
};

// ── Launchers ──────────────────────────────────────────────────────────

const HOME = "C:\\Users\\Alex";

export const launchers: Launcher[] = [
  {
    id: id(51),
    kind: "prism",
    name: "Prism Launcher",
    rootPath: `${HOME}\\AppData\\Roaming\\PrismLauncher`,
    instanceCount: 5,
    canCreateInstances: true,
    addedByUser: false,
  },
  {
    id: id(52),
    kind: "modrinth",
    name: "Modrinth App",
    rootPath: `${HOME}\\AppData\\Roaming\\ModrinthApp`,
    instanceCount: 2,
    canCreateInstances: true,
    addedByUser: false,
  },
  {
    id: id(53),
    kind: "curseforge",
    name: "CurseForge",
    rootPath: `${HOME}\\curseforge\\minecraft`,
    instanceCount: 2,
    canCreateInstances: false,
    addedByUser: false,
  },
  {
    id: id(54),
    kind: "official",
    name: "Minecraft Launcher",
    rootPath: `${HOME}\\AppData\\Roaming\\.minecraft`,
    instanceCount: 1,
    canCreateInstances: false,
    addedByUser: false,
  },
];

const L = (kind: Launcher["kind"]) => ({
  kind,
  name: launchers.find((l) => l.kind === kind)!.name,
});
export const gameDirFor = (kind: Launcher["kind"], name: string) => {
  const root = launchers.find((l) => l.kind === kind)!.rootPath;
  return kind === "prism"
    ? `${root}\\instances\\${name}\\minecraft`
    : kind === "modrinth"
      ? `${root}\\profiles\\${name.replace(/[\\/:*?"<>|]/g, "")}`
      : kind === "curseforge"
        ? `${root}\\Instances\\${name}`
        : root;
};

// ── Content catalog ────────────────────────────────────────────────────
// Real Modrinth / CurseForge project ids where known, so icons/links can be wired up later.

type CatalogEntry = Pick<
  InstanceContent,
  "kind" | "name" | "author" | "side"
> & {
  modrinth: string | null;
  curseforge: string | null;
  file: string;
};

const catalog = {
  fabricApi: {
    kind: "mod",
    name: "Fabric API",
    author: "modmuss50",
    side: "both",
    modrinth: "P7dR8mSH",
    curseforge: "306612",
    file: "fabric-api",
  },
  sodium: {
    kind: "mod",
    name: "Sodium",
    author: "jellysquid3",
    side: "client",
    modrinth: "AANobbMI",
    curseforge: "394468",
    file: "sodium-fabric",
  },
  lithium: {
    kind: "mod",
    name: "Lithium",
    author: "jellysquid3",
    side: "both",
    modrinth: "gvQqBUqZ",
    curseforge: "360438",
    file: "lithium-fabric",
  },
  modMenu: {
    kind: "mod",
    name: "Mod Menu",
    author: "Prospector",
    side: "client",
    modrinth: "mOgUt4GM",
    curseforge: "308702",
    file: "modmenu",
  },
  ferrite: {
    kind: "mod",
    name: "FerriteCore",
    author: "malte0811",
    side: "both",
    modrinth: "uXXizFIs",
    curseforge: "429235",
    file: "ferritecore",
  },
  entityCulling: {
    kind: "mod",
    name: "Entity Culling",
    author: "tr7zw",
    side: "client",
    modrinth: "NNAgCjsB",
    curseforge: "448233",
    file: "entityculling",
  },
  xaeroMinimap: {
    kind: "mod",
    name: "Xaero's Minimap",
    author: "xaero96",
    side: "client",
    modrinth: "1bokaNcj",
    curseforge: "263420",
    file: "xaerominimap",
  },
  voiceChat: {
    kind: "mod",
    name: "Simple Voice Chat",
    author: "henkelmax",
    side: "both",
    modrinth: "9eGKb6K1",
    curseforge: "416089",
    file: "voicechat",
  },
  jei: {
    kind: "mod",
    name: "Just Enough Items",
    author: "mezz",
    side: "both",
    modrinth: "u6dRKJwZ",
    curseforge: "238222",
    file: "jei",
  },
  create: {
    kind: "mod",
    name: "Create",
    author: "simibubi",
    side: "both",
    modrinth: "LNytGWDc",
    curseforge: "328085",
    file: "create",
  },
  cobblemon: {
    kind: "mod",
    name: "Cobblemon",
    author: "Cobblemon Team",
    side: "both",
    modrinth: "MdwFAVRL",
    curseforge: "687131",
    file: "Cobblemon-fabric",
  },
  appleskin: {
    kind: "mod",
    name: "AppleSkin",
    author: "squeek502",
    side: "both",
    modrinth: "EsAfCjCV",
    curseforge: "248787",
    file: "appleskin",
  },
  jade: {
    kind: "mod",
    name: "Jade",
    author: "Snownee",
    side: "both",
    modrinth: "nvQzSEkH",
    curseforge: "324717",
    file: "Jade",
  },
  luckyBlock: {
    kind: "mod",
    name: "Lucky Block",
    author: "PlayerInDistress",
    side: "both",
    modrinth: null,
    curseforge: "999001",
    file: "luckyblock",
  },
  freshAnimations: {
    kind: "resourcepack",
    name: "Fresh Animations",
    author: "FreshLX",
    side: "client",
    modrinth: "50dA9Sha",
    curseforge: "453763",
    file: "FreshAnimations",
  },
  faithful: {
    kind: "resourcepack",
    name: "Faithful 32x",
    author: "Faithful Resource Pack",
    side: "client",
    modrinth: null,
    curseforge: "236821",
    file: "Faithful-32x",
  },
  complementary: {
    kind: "shaderpack",
    name: "Complementary Reimagined",
    author: "EminGT",
    side: "client",
    modrinth: "HVnmMxH1",
    curseforge: "627557",
    file: "ComplementaryReimagined",
  },
  terralith: {
    kind: "datapack",
    name: "Terralith",
    author: "Stardust Labs",
    side: "server",
    modrinth: "8oi3bsk5",
    curseforge: "513688",
    file: "Terralith",
  },
  // Custom files: on no mod site
  smpTextures: {
    kind: "resourcepack",
    name: "SMP Custom Textures",
    author: null,
    side: "client",
    modrinth: null,
    curseforge: null,
    file: "smp-textures",
  },
  smpTweaks: {
    kind: "mod",
    name: "smp-tweaks",
    author: null,
    side: "both",
    modrinth: null,
    curseforge: null,
    file: "smp-tweaks",
  },
  schematics: {
    kind: "resourcepack",
    name: "create-schematics-pack",
    author: null,
    side: "client",
    modrinth: null,
    curseforge: null,
    file: "create-schematics-pack",
  },
} satisfies Record<string, CatalogEntry>;

const folderFor: Record<InstanceContent["kind"], string> = {
  mod: "mods",
  resourcepack: "resourcepacks",
  shaderpack: "shaderpacks",
  datapack: "datapacks",
};

function item(
  key: keyof typeof catalog,
  version: string | null,
  opts: Partial<Pick<InstanceContent, "enabled" | "isManaged" | "source">> = {},
): InstanceContent {
  const c: CatalogEntry = catalog[key];
  const source =
    opts.source ??
    (c.modrinth ? "modrinth" : c.curseforge ? "curseforge" : "local");
  const projectId =
    source === "modrinth"
      ? c.modrinth
      : source === "curseforge"
        ? c.curseforge
        : null;
  const ext = c.kind === "mod" ? "jar" : "zip";
  seq++;
  return {
    id: id(seq),
    kind: c.kind,
    source,
    projectId,
    versionId: projectId
      ? source === "curseforge"
        ? String(5_000_000 + seq)
        : `v${(seq * 7919).toString(36).slice(0, 7)}`
      : null,
    // unknown files show their file name until resolved
    name:
      source === "local"
        ? `${c.file}${version ? `-${version}` : ""}.${ext}`
        : c.name,
    versionLabel: source === "local" ? null : version,
    author: c.author,
    iconUrl: null,
    path: `${folderFor[c.kind]}/${c.file}${version ? `-${version}` : ""}.${ext}`,
    sizeBytes: 200_000 + ((seq * 104_729) % 9_000_000),
    side: c.side,
    enabled: opts.enabled ?? true,
    isManaged: opts.isManaged ?? false,
  };
}

const managed = { isManaged: true };

function backup(
  n: number,
  msAgo: number,
  from: number,
  to: number,
  files: number,
): Backup {
  return {
    id: id(6000 + n),
    reason: "before_update",
    fromRevision: from,
    toRevision: to,
    fileCount: files,
    sizeBytes: files * 1_900_000,
    createdAt: ago(msAgo),
  };
}

// ── Ids ────────────────────────────────────────────────────────────────

export const ids = {
  // tracked instances
  survival: id(101),
  cobblemon: id(102),
  create: id(103),
  vanilla: id(104),
  skyblock: id(105),
  hardcore: id(107),
  lucky: id(108),
  // shared (cloud) instances
  sharedSurvival: id(201),
  sharedCobblemon: id(202),
  sharedHardcore: id(203),
  sharedCozyFarm: id(204),
  sharedRedstoneLab: id(205),
  sharedBoysS2: id(206),
  sharedLucky: id(207),
  // groups
  theBoys: id(401),
  cobbleCrew: id(402),
};

const base = () => ({
  icon: null,
  status: "ready" as const,
  isFavorite: false,
  hideActivity: false,
  isRunning: false,
  contentCount: 0, // filled below
  sync: null,
  progress: null,
  backups: [] as Backup[],
});

// ── Tracked instances ──────────────────────────────────────────────────
// Covers every status, every sync state, every launcher and every content source.

export const instances: InstanceDetail[] = [
  {
    ...base(),
    id: ids.survival,
    name: "Survival SMP",
    launcher: L("prism"),
    gameDir: gameDirFor("prism", "Survival SMP"),
    minecraftVersion: "1.21.1",
    loader: "fabric",
    loaderVersion: "0.16.5",
    isFavorite: true,
    lastPlayedAt: ago(2 * HOUR),
    totalPlaytimeSeconds: 86 * 3600 + 14 * 60,
    trackedAt: ago(95 * DAY),
    lastScannedAt: ago(3 * MIN),
    // you updated Voice Chat in Prism (1 change to publish) and dropped in a custom jar (offered as an upload)
    sync: {
      sharedInstanceId: ids.sharedSurvival,
      role: "owner",
      owner: users.me,
      syncedRevision: 7,
      latestRevision: 7,
      state: "local_changes",
      localChangeCount: 1,
    },
    content: [
      item("fabricApi", "0.105.0+1.21.1", managed),
      item("sodium", "0.6.0+mc1.21.1", managed),
      item("lithium", "0.13.0+mc1.21.1", managed),
      item("ferrite", "7.0.0", managed),
      item("entityCulling", "1.7.1", managed),
      item("xaeroMinimap", "24.5.0", { ...managed, source: "curseforge" }),
      item("voiceChat", "2.5.26"), // updated locally (managed was 2.5.24)
      item("appleskin", "3.0.5", managed),
      item("jade", "15.8.0", { ...managed, source: "curseforge" }),
      item("modMenu", "11.0.2", managed),
      item("smpTweaks", "1.0"), // unknown file: publishing asks whether to upload it
      item("freshAnimations", "1.9.2", managed),
      item("smpTextures", null, { ...managed, source: "upload" }),
      item("terralith", "2.5.4", managed),
    ],
    backups: [backup(1, 4 * DAY, 5, 6, 1)],
  },
  {
    ...base(),
    id: ids.cobblemon,
    name: "Cobblemon w/ Mia",
    launcher: L("modrinth"),
    gameDir: gameDirFor("modrinth", "Cobblemon w/ Mia"),
    minecraftVersion: "1.21.1",
    loader: "fabric",
    loaderVersion: "0.16.5",
    isFavorite: true,
    lastPlayedAt: ago(26 * HOUR),
    totalPlaytimeSeconds: null, // this launcher doesn't record playtime in a form Atlas reads
    trackedAt: ago(40 * DAY),
    lastScannedAt: ago(3 * MIN),
    sync: {
      sharedInstanceId: ids.sharedCobblemon,
      role: "viewer",
      owner: users.mia,
      syncedRevision: 4,
      latestRevision: 5,
      state: "update_available",
      localChangeCount: 0,
    },
    content: [
      item("fabricApi", "0.104.0+1.21.1", managed),
      item("cobblemon", "1.5.2", managed),
      item("sodium", "0.6.0+mc1.21.1", managed),
      item("jei", "19.8.4", { ...managed, source: "curseforge" }),
      item("jade", "15.8.0", managed),
      item("xaeroMinimap", "24.4.0", managed),
      item("modMenu", "11.0.2", managed),
      // viewer's own extra: allowed, kept on update, doesn't count as a local change
      item("complementary", "r5.3"),
    ],
    backups: [backup(2, 6 * DAY, 3, 4, 1), backup(3, 20 * DAY, 2, 3, 1)],
  },
  {
    ...base(),
    id: ids.create,
    name: "Create Engineering",
    launcher: L("curseforge"),
    gameDir: gameDirFor("curseforge", "Create Engineering"),
    minecraftVersion: "1.21.1",
    loader: "neoforge",
    loaderVersion: "21.1.66",
    lastPlayedAt: ago(3 * DAY),
    totalPlaytimeSeconds: null,
    trackedAt: ago(18 * DAY),
    lastScannedAt: ago(3 * MIN),
    content: [
      item("create", "6.0.1", { source: "curseforge" }),
      item("jei", "19.8.4", { source: "curseforge" }),
      item("jade", "15.8.0"),
      item("ferrite", "7.0.0"),
      item("appleskin", "3.0.5", { enabled: false }),
      item("complementary", "r5.3"),
      item("schematics", null),
    ],
  },
  {
    ...base(),
    id: ids.hardcore,
    name: "Hardcore Trials",
    launcher: L("prism"),
    gameDir: gameDirFor("prism", "Hardcore Trials"),
    minecraftVersion: "1.21.1",
    loader: "fabric",
    loaderVersion: "0.16.5",
    hideActivity: true,
    lastPlayedAt: ago(5 * DAY),
    totalPlaytimeSeconds: 7 * 3600 + 45 * 60,
    trackedAt: ago(25 * DAY),
    lastScannedAt: ago(3 * MIN),
    sync: {
      sharedInstanceId: ids.sharedHardcore,
      role: "editor",
      owner: users.jonah,
      syncedRevision: 3,
      latestRevision: 3,
      state: "up_to_date",
      localChangeCount: 0,
    },
    content: [
      item("fabricApi", "0.105.0+1.21.1", managed),
      item("sodium", "0.6.0+mc1.21.1", managed),
      item("lithium", "0.13.0+mc1.21.1", managed),
      item("appleskin", "3.0.5", managed),
    ],
  },
  {
    ...base(),
    id: ids.vanilla,
    name: "Latest release",
    launcher: L("official"),
    gameDir: gameDirFor("official", ""),
    minecraftVersion: "1.21.4",
    loader: "vanilla",
    loaderVersion: null,
    lastPlayedAt: ago(9 * DAY),
    totalPlaytimeSeconds: null,
    trackedAt: ago(60 * DAY),
    lastScannedAt: ago(3 * MIN),
    content: [item("faithful", "1.21.4", { source: "curseforge" })],
  },
  {
    ...base(),
    id: ids.lucky,
    name: "Lucky Block Chaos",
    launcher: L("prism"),
    gameDir: gameDirFor("prism", "Lucky Block Chaos"),
    minecraftVersion: "1.20.1",
    loader: "forge",
    loaderVersion: "47.3.0",
    lastPlayedAt: ago(70 * DAY),
    totalPlaytimeSeconds: 2 * 3600,
    trackedAt: ago(100 * DAY),
    lastScannedAt: ago(3 * MIN),
    // owner deleted their account, nobody to transfer to
    sync: {
      sharedInstanceId: ids.sharedLucky,
      role: "viewer",
      owner: null,
      syncedRevision: 2,
      latestRevision: 2,
      state: "frozen",
      localChangeCount: 0,
    },
    content: [
      item("luckyBlock", "13.0", { ...managed, source: "curseforge" }),
      item("jei", "15.2.0.27", { ...managed, source: "curseforge" }),
    ],
  },
  {
    ...base(),
    id: ids.skyblock,
    name: "Skyblock Classic",
    launcher: L("prism"),
    gameDir: gameDirFor("prism", "Skyblock Classic"),
    minecraftVersion: "1.20.1",
    loader: "forge",
    loaderVersion: "47.3.0",
    // deleted in Prism after it was tracked
    status: "missing",
    lastPlayedAt: ago(41 * DAY),
    totalPlaytimeSeconds: 19 * 3600,
    trackedAt: ago(200 * DAY),
    lastScannedAt: ago(2 * DAY),
    content: [],
  },
];

for (const inst of instances) inst.contentCount = inst.content.length;

/** Everything the launchers contain, tracked or not. */
export const untrackedDetected: DetectedInstance[] = [
  {
    launcherKind: "prism",
    launcherInstanceId: "Test 1.21",
    name: "Test 1.21",
    gameDir: gameDirFor("prism", "Test 1.21"),
    minecraftVersion: "1.21.1",
    loader: "fabric",
    loaderVersion: "0.16.5",
    instanceId: null,
  },
  {
    launcherKind: "modrinth",
    launcherInstanceId: "Fabulously Optimized",
    name: "Fabulously Optimized",
    gameDir: gameDirFor("modrinth", "Fabulously Optimized"),
    minecraftVersion: "1.21.4",
    loader: "fabric",
    loaderVersion: "0.16.9",
    instanceId: null,
  },
  {
    launcherKind: "curseforge",
    launcherInstanceId: "All the Mods 9",
    name: "All the Mods 9",
    gameDir: gameDirFor("curseforge", "All the Mods 9"),
    minecraftVersion: "1.20.1",
    loader: "forge",
    loaderVersion: "47.3.0",
    instanceId: null,
  },
];

// ── Friends ────────────────────────────────────────────────────────────

const offline = (msAgo: number) => ({
  state: "offline" as const,
  instanceName: null,
  minecraftVersion: null,
  sharedInstanceId: null,
  updatedAt: ago(msAgo),
});

export const friends: Friend[] = [
  {
    user: users.mia,
    friendsSince: ago(180 * DAY),
    presence: {
      state: "playing",
      instanceName: "Cobblemon w/ Mia",
      minecraftVersion: "1.21.1",
      sharedInstanceId: ids.sharedCobblemon,
      updatedAt: ago(1 * MIN),
    },
  },
  {
    user: users.sam,
    friendsSince: ago(150 * DAY),
    presence: {
      state: "playing",
      instanceName: "Survival SMP",
      minecraftVersion: "1.21.1",
      sharedInstanceId: ids.sharedSurvival,
      updatedAt: ago(30_000),
    },
  },
  {
    user: users.jonah,
    friendsSince: ago(90 * DAY),
    // playing with "hide activity" on
    presence: {
      state: "playing",
      instanceName: null,
      minecraftVersion: null,
      sharedInstanceId: null,
      updatedAt: ago(3 * MIN),
    },
  },
  {
    user: users.ava,
    friendsSince: ago(70 * DAY),
    presence: {
      state: "online",
      instanceName: null,
      minecraftVersion: null,
      sharedInstanceId: null,
      updatedAt: ago(45_000),
    },
  },
  {
    user: users.priya,
    friendsSince: ago(4 * DAY),
    presence: offline(3 * HOUR),
  },
  { user: users.leo, friendsSince: ago(60 * DAY), presence: offline(5 * DAY) },
];

export const friendRequests: FriendRequest[] = [
  { direction: "incoming", user: users.noah, createdAt: ago(1 * HOUR) },
  { direction: "outgoing", user: users.zoe, createdAt: ago(2 * DAY) },
];

// ── Groups ─────────────────────────────────────────────────────────────

const gm = (
  user: PublicUser,
  role: GroupMember["role"],
  msAgo: number,
): GroupMember => ({ user, role, joinedAt: ago(msAgo) });

const boysInvite: GroupInvite = {
  code: "K3M9PQ2XRT",
  link: `${SITE}/g/K3M9PQ2XRT`,
  createdBy: users.me,
  maxUses: 10,
  uses: 3,
  expiresAt: ahead(5 * DAY),
  createdAt: ago(2 * DAY),
};

export const groups: GroupDetail[] = [
  {
    id: ids.theBoys,
    name: "The Boys",
    icon: "users",
    myRole: "owner",
    members: [
      gm(users.me, "owner", 120 * DAY),
      gm(users.sam, "admin", 119 * DAY),
      gm(users.jonah, "member", 100 * DAY),
      gm(users.leo, "member", 20 * DAY),
    ],
    sharedInstanceIds: [ids.sharedSurvival, ids.sharedBoysS2],
    createdAt: ago(120 * DAY),
    invites: [boysInvite],
  },
  {
    id: ids.cobbleCrew,
    name: "Cobble Crew",
    icon: "leaf",
    myRole: "member",
    members: [
      gm(users.mia, "owner", 45 * DAY),
      gm(users.me, "member", 40 * DAY),
      gm(users.sam, "member", 39 * DAY),
    ],
    sharedInstanceIds: [ids.sharedCobblemon],
    createdAt: ago(45 * DAY),
    invites: [], // not visible to plain members
  },
];

/** Group invite codes that resolve in the mock (try them in "Join group"). */
export const groupInviteCodes: Record<
  string,
  {
    groupId: string | null;
    name: string;
    icon: string | null;
    memberCount: number;
    invitedBy: PublicUser;
  }
> = {
  K3M9PQ2XRT: {
    groupId: ids.theBoys,
    name: "The Boys",
    icon: "users",
    memberCount: 4,
    invitedBy: users.me,
  },
  R8T2VW5NYC: {
    groupId: null,
    name: "Redstone Nerds",
    icon: "cube",
    memberCount: 7,
    invitedBy: users.leo,
  },
};

// ── Shared instances ───────────────────────────────────────────────────

const boys = { id: ids.theBoys, name: "The Boys" };
const crew = { id: ids.cobbleCrew, name: "Cobble Crew" };

const member = (
  user: PublicUser,
  role: SharedMember["role"],
  status: SharedMember["status"],
  lastSyncedRevision: number | null,
  viaGroup: SharedMember["viaGroup"] = null,
): SharedMember => ({ user, role, status, viaGroup, lastSyncedRevision });

const rev = (
  revision: number,
  createdBy: PublicUser | null,
  msAgo: number,
  changelog: string | null,
  changes: Partial<RevisionSummary["changes"]> = {},
): RevisionSummary => ({
  revision,
  changelog,
  createdBy,
  createdAt: ago(msAgo),
  changes: { added: [], removed: [], updated: [], ...changes },
});

const sharedBase = {
  frozen: false,
  myViaGroup: null,
  invitedBy: null,
  invitedAt: null,
  localInstanceId: null,
} as const;

export const sharedInstances: SharedInstanceDetail[] = [
  // I own it; shared directly and with a group
  {
    ...sharedBase,
    id: ids.sharedSurvival,
    name: "Survival SMP",
    description: "Our main world. Performance mods + a few QoL ones.",
    icon: "leaf",
    owner: users.me,
    minecraftVersion: "1.21.1",
    loader: "fabric",
    loaderVersion: "0.16.5",
    latestRevision: 7,
    contentCount: 13,
    sizeBytes: 64 * MB,
    updatedAt: ago(1 * DAY),
    myRole: "owner",
    myStatus: "active",
    localInstanceId: ids.survival,
    groups: [{ group: { ...boys, icon: "users" }, role: "viewer" }],
    members: [
      member(users.me, "owner", "active", 7),
      member(users.sam, "editor", "active", 7),
      member(users.jonah, "viewer", "active", 6, boys),
      member(users.leo, "viewer", "invited", null, boys),
      member(users.priya, "viewer", "invited", null),
    ],
    revisions: [
      rev(7, users.me, 1 * DAY, "Added Terralith for new chunks", {
        added: ["Terralith"],
      }),
      rev(6, users.sam, 4 * DAY, "Voice chat update", {
        updated: [{ name: "Simple Voice Chat", from: "2.5.20", to: "2.5.24" }],
      }),
      rev(5, users.me, 12 * DAY, null, {
        added: ["Jade", "AppleSkin", "SMP Custom Textures"],
        removed: ["WTHIT"],
      }),
      rev(4, users.me, 30 * DAY, "Fresh Animations for everyone", {
        added: ["Fresh Animations"],
      }),
    ],
  },
  // Joined through a group; update waiting
  {
    ...sharedBase,
    id: ids.sharedCobblemon,
    name: "Cobblemon w/ Mia",
    description: "Gotta catch 'em all. Please don't add mods without asking!",
    icon: "cube",
    owner: users.mia,
    minecraftVersion: "1.21.1",
    loader: "fabric",
    loaderVersion: "0.16.5",
    latestRevision: 5,
    contentCount: 8,
    sizeBytes: 182 * MB,
    updatedAt: ago(35 * MIN),
    myRole: "viewer",
    myStatus: "active",
    myViaGroup: crew,
    invitedBy: users.mia,
    invitedAt: ago(40 * DAY),
    localInstanceId: ids.cobblemon,
    groups: [{ group: { ...crew, icon: "leaf" }, role: "viewer" }],
    members: [
      member(users.mia, "owner", "active", 5),
      member(users.me, "viewer", "active", 4, crew),
      member(users.sam, "viewer", "active", 5, crew),
    ],
    revisions: [
      rev(5, users.mia, 35 * MIN, "Cobblemon 1.6 + AppleSkin", {
        added: ["AppleSkin"],
        updated: [
          { name: "Cobblemon", from: "1.5.2", to: "1.6.0" },
          { name: "Fabric API", from: "0.104.0+1.21.1", to: "0.105.0+1.21.1" },
        ],
      }),
      rev(4, users.mia, 6 * DAY, "Minimap", { added: ["Xaero's Minimap"] }),
      rev(3, users.mia, 20 * DAY, null, { added: ["Jade"] }),
    ],
  },
  // Editor with unpublished local changes
  {
    ...sharedBase,
    groups: [],
    id: ids.sharedHardcore,
    name: "Hardcore Trials",
    description: "One life. No minimap. (We'll see.)",
    icon: "shield",
    owner: users.jonah,
    minecraftVersion: "1.21.1",
    loader: "fabric",
    loaderVersion: "0.16.5",
    latestRevision: 3,
    contentCount: 4,
    sizeBytes: 9 * MB,
    updatedAt: ago(8 * DAY),
    myRole: "editor",
    myStatus: "active",
    invitedBy: users.jonah,
    invitedAt: ago(25 * DAY),
    localInstanceId: ids.hardcore,
    members: [
      member(users.jonah, "owner", "active", 3),
      member(users.me, "editor", "active", 3),
    ],
    revisions: [
      rev(3, users.jonah, 8 * DAY, "Perf mods", { added: ["Lithium"] }),
      rev(2, users.me, 15 * DAY, null, { added: ["AppleSkin"] }),
      rev(1, users.jonah, 25 * DAY, "Initial setup", {
        added: ["Fabric API", "Sodium"],
      }),
    ],
  },
  // Frozen: owner deleted their account
  {
    ...sharedBase,
    groups: [],
    id: ids.sharedLucky,
    name: "Lucky Block Chaos",
    description: null,
    icon: "cube",
    owner: null,
    minecraftVersion: "1.20.1",
    loader: "forge",
    loaderVersion: "47.3.0",
    latestRevision: 2,
    contentCount: 2,
    sizeBytes: 6 * MB,
    updatedAt: ago(80 * DAY),
    frozen: true,
    myRole: "viewer",
    myStatus: "active",
    invitedAt: ago(100 * DAY),
    localInstanceId: ids.lucky,
    members: [member(users.me, "viewer", "active", 2)],
    revisions: [
      rev(2, null, 80 * DAY, null, { added: ["JEI"] }),
      rev(1, null, 100 * DAY, "Lucky blocks!", { added: ["Lucky Block"] }),
    ],
  },
  // ── Inbox: pending invites ──
  {
    ...sharedBase,
    groups: [],
    id: ids.sharedCozyFarm,
    name: "Mia's Cozy Farm",
    description: "Farming, cooking and decorating. Very chill.",
    icon: "leaf",
    owner: users.mia,
    minecraftVersion: "1.20.1",
    loader: "fabric",
    loaderVersion: "0.15.11",
    latestRevision: 2,
    contentCount: 34,
    sizeBytes: 310 * MB,
    updatedAt: ago(3 * HOUR),
    myRole: "viewer",
    myStatus: "invited",
    invitedBy: users.mia,
    invitedAt: ago(3 * HOUR),
    members: [
      member(users.mia, "owner", "active", 2),
      member(users.me, "viewer", "invited", null),
    ],
    revisions: [
      rev(2, users.mia, 3 * HOUR, "Added Farmer's Delight addons", {
        added: ["Farmer's Respite", "Brewin' and Chewin'"],
      }),
      rev(1, users.mia, 2 * DAY, "Initial setup"),
    ],
  },
  {
    ...sharedBase,
    groups: [],
    id: ids.sharedRedstoneLab,
    name: "Redstone Lab",
    description: null,
    icon: null,
    owner: users.leo,
    minecraftVersion: "1.21.4",
    loader: "vanilla",
    loaderVersion: null,
    latestRevision: 1,
    contentCount: 2,
    sizeBytes: 3 * MB,
    updatedAt: ago(1 * DAY),
    myRole: "editor",
    myStatus: "invited",
    invitedBy: users.leo,
    invitedAt: ago(1 * DAY),
    members: [
      member(users.leo, "owner", "active", 1),
      member(users.me, "editor", "invited", null),
      member(users.jonah, "viewer", "declined", null),
    ],
    revisions: [
      rev(1, users.leo, 1 * DAY, "Datapacks + textures", {
        added: ["Terralith", "Faithful 32x"],
      }),
    ],
  },
  // Shared to my group by Sam; shows in my inbox via the group
  {
    ...sharedBase,
    id: ids.sharedBoysS2,
    name: "Boys Modded S2",
    description: "Season 2. Tech + magic.",
    icon: "cube",
    owner: users.sam,
    minecraftVersion: "1.21.1",
    loader: "neoforge",
    loaderVersion: "21.1.66",
    latestRevision: 1,
    contentCount: 112,
    sizeBytes: 640 * MB,
    updatedAt: ago(5 * HOUR),
    myRole: "viewer",
    myStatus: "invited",
    myViaGroup: boys,
    invitedBy: users.sam,
    invitedAt: ago(5 * HOUR),
    groups: [{ group: { ...boys, icon: "users" }, role: "viewer" }],
    members: [
      member(users.sam, "owner", "active", 1),
      member(users.me, "viewer", "invited", null, boys),
      member(users.jonah, "viewer", "active", 1, boys),
      member(users.leo, "viewer", "invited", null, boys),
    ],
    revisions: [
      rev(1, users.sam, 5 * HOUR, "Season 2 begins", {
        added: ["Create", "Ars Nouveau", "Mekanism", "…109 more"],
      }),
    ],
  },
];

// ── Activity (Home feed) ───────────────────────────────────────────────

const act = (
  n: number,
  type: ActivityItem["type"],
  actor: PublicUser | null,
  msAgo: number,
  read: boolean,
  extra: Partial<
    Pick<
      ActivityItem,
      | "sharedInstanceId"
      | "sharedInstanceName"
      | "groupId"
      | "groupName"
      | "revision"
    >
  > = {},
): ActivityItem => ({
  id: id(n),
  type,
  actor,
  createdAt: ago(msAgo),
  readAt: read ? ago(msAgo / 2) : null,
  sharedInstanceId: null,
  sharedInstanceName: null,
  groupId: null,
  groupName: null,
  revision: null,
  ...extra,
});

export const activity: ActivityItem[] = [
  act(301, "instance_updated", users.mia, 35 * MIN, false, {
    sharedInstanceId: ids.sharedCobblemon,
    sharedInstanceName: "Cobblemon w/ Mia",
    revision: 5,
  }),
  act(302, "friend_request", users.noah, 1 * HOUR, false),
  act(303, "instance_invite", users.mia, 3 * HOUR, false, {
    sharedInstanceId: ids.sharedCozyFarm,
    sharedInstanceName: "Mia's Cozy Farm",
  }),
  act(304, "instance_shared_to_group", users.sam, 5 * HOUR, false, {
    sharedInstanceId: ids.sharedBoysS2,
    sharedInstanceName: "Boys Modded S2",
    groupId: ids.theBoys,
    groupName: "The Boys",
  }),
  act(305, "instance_invite", users.leo, 1 * DAY, true, {
    sharedInstanceId: ids.sharedRedstoneLab,
    sharedInstanceName: "Redstone Lab",
  }),
  act(306, "group_joined", users.leo, 20 * DAY, true, {
    groupId: ids.theBoys,
    groupName: "The Boys",
  }),
  act(307, "member_joined", users.jonah, 2 * DAY, true, {
    sharedInstanceId: ids.sharedSurvival,
    sharedInstanceName: "Survival SMP",
  }),
  act(308, "friend_accepted", users.priya, 4 * DAY, true),
  act(309, "instance_updated", users.sam, 4 * DAY, true, {
    sharedInstanceId: ids.sharedSurvival,
    sharedInstanceName: "Survival SMP",
    revision: 6,
  }),
  act(310, "instance_frozen", null, 80 * DAY, true, {
    sharedInstanceId: ids.sharedLucky,
    sharedInstanceName: "Lucky Block Chaos",
  }),
].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
