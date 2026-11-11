//! The manifest: a shareable description of an instance (see
//! tests/fixtures/manifest.example.json). The server validates every manifest it stores
//! and the app validates every one it applies, both with this code.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api::{RevisionChanges, UpdatedItem};
use crate::enums::{ContentKind, ContentSource, Loader, Side};

/// 2: `Override::keep`, files outside the config folders, more file types.
/// 3: `Entry::in_pack`, mods and packs taken from the modpack. Only manifests that have
/// one say 3, so the rest stay readable by apps that know 2.
pub const SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub minecraft: MinecraftSpec,
    #[serde(default)]
    pub recommended_memory_mb: Option<u32>,
    pub content: Vec<Entry>,
    #[serde(default)]
    pub overrides: Vec<Override>,
    /// The modpack it was installed from, when there is one: overrides marked `in_pack`
    /// come out of the pack's own download instead of being uploaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<PackBase>,
    /// Which other files go out, so an editor's update and the owner's other PCs send
    /// the same ones. Older versions don't have it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<FileRules>,
}

/// Which of the instance's other files friends get, as the Files tab saves them: paths,
/// folders ending in "/", "" for the whole folder. The longest rule covering a file wins;
/// without one, files from the modpack and the config folders go out.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRules {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Files members get once and then keep as their own (options.txt always is).
    #[serde(default)]
    pub first_install: Vec<String>,
    /// Files sent once by default (personal settings) that go out on every update instead.
    #[serde(default)]
    pub every_update: Vec<String>,
}

impl Manifest {
    /// The instance picture, when it's one uploaded to Atlas: `icon` is "sha256:<hex>".
    /// Anything else (older manifests used icon keys like "leaf") isn't a picture.
    pub fn icon_sha256(&self) -> Option<&str> {
        self.icon
            .as_deref()?
            .strip_prefix("sha256:")
            .filter(|h| is_sha256(h))
    }

    /// Every uploaded file this version needs: uploads, config files (not those from the
    /// modpack), the picture and the files' own pictures.
    pub fn blobs(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .content
            .iter()
            .filter(|e| e.source == ContentSource::Upload && !e.in_pack)
            .map(|e| e.sha256.clone())
            .chain(
                self.overrides
                    .iter()
                    .filter(|o| !o.in_pack)
                    .map(|o| o.sha256.clone()),
            )
            .chain(self.icon_sha256().map(str::to_owned))
            .chain(
                self.content
                    .iter()
                    .filter_map(|e| e.icon_sha256().map(str::to_owned)),
            )
            .collect();
        out.sort();
        out.dedup();
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftSpec {
    pub version: String,
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub kind: ContentKind,
    pub source: ContentSource,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub version_label: Option<String>,
    /// Relative, forward slashes: "mods/sodium.jar"
    pub path: String,
    pub sha1: String,
    pub sha256: String,
    pub size_bytes: u64,
    /// None = CurseForge file with third-party downloads disabled (see page_url), or an upload.
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_url: Option<String>,
    #[serde(default = "both")]
    pub side: Side,
    /// false = optional: members choose whether to download it.
    #[serde(default = "yes")]
    pub required: bool,
    /// Its picture: "sha256:<hex>" when uploaded to Atlas, or an https link on Modrinth's
    /// or CurseForge's image host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// An upload exactly as the modpack (`Manifest::base`) has it: members take it from the
    /// pack's download, so it isn't uploaded. Schema 3.
    #[serde(default, skip_serializing_if = "is_false")]
    pub in_pack: bool,
}

fn both() -> Side {
    Side::Both
}
fn yes() -> bool {
    true
}

impl Entry {
    /// Identifies "the same item" across versions: the provider's
    /// project id, or the file hash for uploads. Also `OptionalItem.key`.
    pub fn key(&self) -> &str {
        self.project_id.as_deref().unwrap_or(&self.sha256)
    }

    /// Its picture's sha256, when it's one uploaded to Atlas.
    pub fn icon_sha256(&self) -> Option<&str> {
        self.icon
            .as_deref()?
            .strip_prefix("sha256:")
            .filter(|h| is_sha256(h))
    }

    /// An uploaded mod or resource pack: members approve it before it installs. Shaders
    /// and datapacks can't reach outside the game, so they install like configs. One from
    /// the modpack comes out of the pack's own download from Modrinth or CurseForge.
    pub fn needs_approval(&self) -> bool {
        self.source == ContentSource::Upload
            && !self.in_pack
            && matches!(self.kind, ContentKind::Mod | ContentKind::Resourcepack)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Override {
    /// Relative: "config/sodium-options.json"
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
    /// Exactly as the modpack (`Manifest::base`) has it: members take it from the pack's
    /// download, so it isn't uploaded.
    #[serde(default, skip_serializing_if = "is_false")]
    pub in_pack: bool,
    /// How members' copies are kept, when not like every other file. Schema 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<Keep>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Files that are partly the member's own: their settings, their server list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keep {
    /// Written only when the member has no such file; never replaced or removed after.
    FirstInstall,
    /// `servers.dat`: the owner's servers join the member's list, which keeps its own.
    MergeServers,
}

/// A modpack file on CurseForge or Modrinth: a .zip or .mrpack whose `overrides/` folder
/// holds the pack's own files (configs, scripts, textures).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackBase {
    /// Modrinth or CurseForge.
    pub source: ContentSource,
    pub project_id: String,
    /// The pack version: Modrinth's version id, CurseForge's file id.
    pub version_id: String,
    /// "All the Mods 10: To the Sky"
    pub name: String,
    /// "2.0.4"
    #[serde(default)]
    pub version_label: Option<String>,
    pub download_url: String,
    pub sha1: String,
    pub size_bytes: u64,
}

// ── Validation ─────────────────────────────────────
// A manifest decides where files are written on other people's PCs: treat it as hostile.

pub const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_CONTENT: usize = 2000;
/// Config files, scripts and the like, from the pack and uploaded together. A big pack
/// has a few thousand; SkyFactory 5 ships about 7,900.
pub const MAX_OVERRIDES: usize = 20_000;
/// The largest one uploaded file of those. Files from the pack have no limit of their own.
pub const MAX_OVERRIDE_BYTES: u64 = 8 * 1024 * 1024;
/// All uploaded ones together.
pub const MAX_OVERRIDES_TOTAL: u64 = 150 * 1024 * 1024;
/// The largest instance picture that can be shared.
pub const MAX_ICON_BYTES: u64 = 1024 * 1024;
/// The largest file a manifest may list. Sizes are added up, so they stay far from overflowing.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const CONTENT_DIRS: [&str; 4] = ["mods/", "resourcepacks/", "shaderpacks/", "datapacks/"];
/// Never uploaded or written from a pack: worlds, what the game or launcher writes about
/// this PC, and the official launcher's own files, there when the game folder is its root
/// (`.minecraft`): its profiles and accounts, and the versions, libraries and Java it runs.
/// Also Atlas's staging folder and Tandem's: what they keep there decides what they do next.
pub const NEVER_SHARED: [&str; 19] = [
    "saves/",
    "screenshots/",
    "logs/",
    "crash-reports/",
    "backups/",
    ".fabric/",
    ".mixin.out/",
    "essential/",
    "minecraftinstance.json",
    "instance.cfg",
    "mmc-pack.json",
    "profile.json",
    "launcher_",
    "versions/",
    "libraries/",
    "assets/",
    "runtime/",
    ".atlas-staging/",
    ".tandem/",
];
/// What uploaded files may be: text, data and pictures, never programs.
const OVERRIDE_EXTS: [&str; 28] = [
    "json",
    "json5",
    "jsonc",
    "hjson",
    "toml",
    "cfg",
    "config",
    "ini",
    "properties",
    "txt",
    "yml",
    "yaml",
    "xml",
    "snbt",
    "js",
    "zs",
    "mcfunction",
    "conf",
    "lang",
    "mcmeta",
    "md",
    "csv",
    "nbt",
    "png",
    "jpg",
    "jpeg",
    "ogg",
    "ttf",
];
/// Never written from a pack either: programs, native libraries and scripts Windows runs.
const RISKY_EXTS: [&str; 21] = [
    "exe", "dll", "so", "dylib", "jnilib", "sys", "bat", "cmd", "com", "scr", "ps1", "psm1", "vbs",
    "vbe", "wsf", "hta", "msi", "lnk", "reg", "jar", "sh",
];
/// Files known to hold a Minecraft login (ReAuth keeps Microsoft tokens, readable when
/// its encryption is off), the official launcher's `launcher_accounts*.json`, and the
/// game's command history, which keeps passwords typed in `/login` and `/register`.
/// Never uploaded, whatever the owner ticks.
const LOGIN_FILES: [&str; 3] = [".reauth.cfg", "reauth.toml", "command_history.txt"];
const DOWNLOAD_HOSTS: [&str; 3] = [
    "cdn.modrinth.com",
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
];
/// Where a file's picture may be, when it isn't uploaded: what the app's CSP lets it show.
const ICON_HOSTS: [&str; 2] = ["cdn.modrinth.com", "media.forgecdn.net"];
/// Where a file's page may be: Atlas opens it in the browser for hand downloads.
const PAGE_HOSTS: [&str; 2] = ["modrinth.com", "www.curseforge.com"];

/// Why a manifest was refused, as a sentence players can read.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

/// Parse and validate a manifest from JSON bytes.
pub fn parse(bytes: &[u8]) -> Result<Manifest, Invalid> {
    if bytes.len() > MAX_BYTES {
        return Err(Invalid("The list of files is over 8 MB.".into()));
    }
    let m: Manifest = serde_json::from_slice(bytes)
        .map_err(|_| Invalid("The list of files is damaged.".into()))?;
    validate(&m)?;
    Ok(m)
}

pub fn validate(m: &Manifest) -> Result<(), Invalid> {
    let bad = |s: String| Err(Invalid(s));
    // Older versions stay readable: History and Undo go back to them.
    if m.schema_version == 0 || m.schema_version > SCHEMA_VERSION {
        return bad("It was made by a newer Atlas. Update Atlas.".into());
    }
    if m.content.len() > MAX_CONTENT || m.overrides.len() > MAX_OVERRIDES {
        return bad(format!(
            "It has too many files: at most {MAX_CONTENT} mods and packs, and {MAX_OVERRIDES} other files."
        ));
    }
    if !plain(&m.minecraft.version) || !m.minecraft.loader_version.as_deref().is_none_or(plain) {
        return bad("Its Minecraft or loader version can't be read.".into());
    }
    if let Some(b) = &m.base {
        if !matches!(
            b.source,
            ContentSource::Modrinth | ContentSource::Curseforge
        ) || b.project_id.is_empty()
            || b.version_id.is_empty()
            || !plain(&b.project_id)
            || !plain(&b.version_id)
            || b.name.trim().is_empty()
            || !is_sha1(&b.sha1)
            || b.size_bytes > MAX_FILE_BYTES
        {
            return bad("The modpack it's based on is damaged.".into());
        }
        if !allowed_download(&b.download_url) {
            return bad(format!(
                "{} downloads from a site Atlas doesn't allow.",
                b.name
            ));
        }
    }
    let mut seen = HashSet::new();
    for e in &m.content {
        // In its own kind's folder: a mod listed as a shader pack would skip approval.
        check_path(&e.path, Some(&[content_dir(e.kind)]))?;
        let ext = if e.kind == ContentKind::Mod {
            "jar"
        } else {
            "zip"
        };
        if !e.path.to_ascii_lowercase().ends_with(&format!(".{ext}")) {
            return bad(format!("{} has to be a .{ext} file.", e.path));
        }
        if !is_sha1(&e.sha1)
            || !is_sha256(&e.sha256)
            || ![&e.project_id, &e.version_id]
                .into_iter()
                .all(|id| id.as_deref().is_none_or(plain))
        {
            return bad(format!("{} couldn't be read properly. Try again.", e.path));
        }
        if e.size_bytes > MAX_FILE_BYTES {
            return bad(format!("{} is over 4 GB.", e.path));
        }
        if e.source == ContentSource::Local {
            return bad(format!("{} is set to Only me.", e.path));
        }
        if e.in_pack && (e.source != ContentSource::Upload || m.base.is_none()) {
            return bad(format!("{} comes from a modpack it doesn't name.", e.path));
        }
        if let Some(url) = &e.download_url
            && !allowed_download(url)
        {
            return bad(format!(
                "{} downloads from a site Atlas doesn't allow.",
                e.path
            ));
        }
        if let Some(url) = &e.page_url
            && !allowed_page(url)
        {
            return bad(format!("{} links to a site Atlas doesn't allow.", e.path));
        }
        if let Some(icon) = &e.icon
            && e.icon_sha256().is_none()
            && !allowed_icon(icon)
        {
            return bad(format!(
                "{}'s picture is on a site Atlas doesn't allow.",
                e.path
            ));
        }
        if !seen.insert(e.path.to_lowercase()) {
            return bad(format!("{} is in it twice.", e.path));
        }
    }
    let mut total = 0;
    for o in &m.overrides {
        if o.in_pack {
            if m.base.is_none() {
                return bad(format!("{} comes from a modpack it doesn't name.", o.path));
            }
            if !can_come_from_pack(&o.path) {
                return bad(format!(
                    "{} isn't a file Atlas writes from a modpack.",
                    o.path
                ));
            }
        } else {
            check_path(&o.path, None)?;
            if let Some(why) = upload_refusal(&o.path, o.size_bytes) {
                return bad(match why {
                    Refusal::Folder => format!("{} is in a folder Atlas never shares.", o.path),
                    Refusal::Login => {
                        format!("{} can hold a login, so Atlas never shares it.", o.path)
                    }
                    Refusal::Program => {
                        format!("{} is a program, so Atlas never shares it.", o.path)
                    }
                    Refusal::FileType => format!("{} isn't a file type Atlas shares.", o.path),
                    Refusal::TooBig => {
                        format!("{} is over 8 MB. Untick it to share the rest.", o.path)
                    }
                });
            }
            total += o.size_bytes;
        }
        if o.keep == Some(Keep::MergeServers) && o.path != "servers.dat" {
            return bad(format!("{} can't be merged like a server list.", o.path));
        }
        if !is_sha256(&o.sha256) {
            return bad(format!("{} couldn't be read properly. Try again.", o.path));
        }
        if o.size_bytes > MAX_FILE_BYTES {
            return bad(format!("{} is over 4 GB.", o.path));
        }
        if !seen.insert(o.path.to_lowercase()) {
            return bad(format!("{} is in it twice.", o.path));
        }
    }
    if total > MAX_OVERRIDES_TOTAL {
        return bad(
            "The uploaded files add up to more than 150 MB. Untick some to share the rest.".into(),
        );
    }
    if let Some(f) = &m.files {
        let lists = [&f.include, &f.exclude, &f.first_install, &f.every_update];
        if lists.iter().map(|l| l.len()).sum::<usize>() > MAX_OVERRIDES {
            return bad("It has too many file choices.".into());
        }
        for rule in lists.into_iter().flatten() {
            let path = rule.strip_suffix('/').unwrap_or(rule);
            if !path.is_empty() {
                check_path(path, None)?;
            }
        }
    }
    Ok(())
}

/// Why a file can't be uploaded, if it can't: where it is, what it is, or its size. (The
/// count and total limits apply to the whole set.)
pub fn upload_refusal(path: &str, size_bytes: u64) -> Option<Refusal> {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    let ext = extension(path);
    if check_path(path, None).is_err()
        || CONTENT_DIRS
            .iter()
            .chain(&NEVER_SHARED)
            .any(|p| lower.starts_with(p))
    {
        Some(Refusal::Folder)
    } else if LOGIN_FILES.contains(&name)
        || (name.starts_with("launcher_accounts") && name.ends_with(".json"))
    {
        Some(Refusal::Login)
    } else if RISKY_EXTS.contains(&ext.as_str()) {
        Some(Refusal::Program)
    } else if !shareable_type(path) {
        Some(Refusal::FileType)
    } else if size_bytes > MAX_OVERRIDE_BYTES {
        Some(Refusal::TooBig)
    } else {
        None
    }
}

/// Why a file can't be shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Mods and packs (shared as content), worlds, logs, launcher files.
    Folder,
    /// It can hold a Minecraft login.
    Login,
    /// A program, a native library or a script Windows runs.
    Program,
    FileType,
    TooBig,
}

/// Whether a modpack's own file may be written from the pack: a safe path, not where
/// mods and packs go (those are content) or a folder never shared, and nothing Windows runs.
pub fn can_come_from_pack(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    check_path(path, None).is_ok()
        && !CONTENT_DIRS
            .iter()
            .chain(&NEVER_SHARED)
            .any(|d| lower.starts_with(d))
        && !RISKY_EXTS.contains(&extension(path).as_str())
}

/// Whether a modpack's own file is a mod or pack it may stand in for: one in a content folder.
pub fn can_be_pack_content(path: &str) -> bool {
    check_path(path, Some(&CONTENT_DIRS)).is_ok()
}

/// The folder each kind of content is in (scan/files.rs in the app finds them there).
pub fn content_dir(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Mod => "mods/",
        ContentKind::Resourcepack => "resourcepacks/",
        ContentKind::Shaderpack => "shaderpacks/",
        ContentKind::Datapack => "datapacks/",
    }
}

fn shareable_type(path: &str) -> bool {
    OVERRIDE_EXTS.contains(&extension(path).as_str()) || path == "servers.dat"
}

fn extension(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

/// A relative path that stays inside the instance, in one of `dirs` when given.
fn check_path(path: &str, dirs: Option<&[&str]>) -> Result<(), Invalid> {
    // The reason is for whoever reads this code; players only need to know which file.
    let bad = |_why: &str| {
        Err(Invalid(format!(
            "{path} has a name or folder Atlas can't share."
        )))
    };
    if path.is_empty() || path.len() > 255 {
        return bad("bad length");
    }
    if path.contains('\\') || path.starts_with('/') || path.contains(':') {
        return bad("must be a relative path with forward slashes");
    }
    if path
        .split('/')
        .any(|seg| seg.is_empty() || seg.ends_with(['.', ' ']))
    {
        // Covers . and .., and names Windows would quietly trim ("x.bat." → "x.bat").
        return bad("empty segment, or one ending in . or a space");
    }
    if path.split('/').any(is_reserved_name) {
        return bad("a name Windows keeps for a device");
    }
    if path
        .chars()
        .any(|c| c.is_control() || "<>\"|?*".contains(c))
    {
        return bad("characters Windows doesn't allow");
    }
    let lower = path.to_ascii_lowercase();
    if dirs.is_some_and(|dirs| !dirs.iter().any(|d| lower.starts_with(d))) {
        return bad("not in an allowed folder");
    }
    Ok(())
}

/// A name Windows keeps for a device, with or without an extension ("CON", "nul.txt",
/// "com1.jar"): opening it opens the device, not a file.
pub fn is_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end();
    let stem = stem.to_ascii_lowercase();
    let port = |n: &str| {
        let mut c = n.chars();
        matches!((c.next(), c.next()), (Some(d), None) if d.is_ascii_digit() || "¹²³".contains(d))
    };
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || stem.strip_prefix("com").is_some_and(port)
        || stem.strip_prefix("lpt").is_some_and(port)
}

/// A version or id as a manifest may name one: these end up in launchers' files and in
/// lookups, so letters, digits, `.-_+` and spaces, up to 64.
fn plain(s: &str) -> bool {
    s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_+ ".contains(&b))
}

/// A sha1 as both sides write it: 40 lowercase hex digits.
pub fn is_sha1(s: &str) -> bool {
    is_lower_hex(s, 40)
}

/// A sha256 as both sides write it: 64 lowercase hex digits.
pub fn is_sha256(s: &str) -> bool {
    is_lower_hex(s, 64)
}

fn is_lower_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A download link a manifest may carry: https on Modrinth's or CurseForge's CDN.
pub fn allowed_download(url: &str) -> bool {
    https_on(url, &DOWNLOAD_HOSTS)
}

/// A file picture a manifest may link to rather than upload.
pub fn allowed_icon(url: &str) -> bool {
    https_on(url, &ICON_HOSTS)
}

/// A page link a manifest may carry: https on Modrinth or CurseForge. Anything else could
/// make Windows run a program when it's opened.
pub fn allowed_page(url: &str) -> bool {
    https_on(url, &PAGE_HOSTS)
}

fn https_on(url: &str, hosts: &[&str]) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '\\', '?', '#']).next().unwrap_or("");
    hosts.contains(&host)
}

// ── Canonical hash ─────────────────────────────────────────────────────

/// sha256 of the canonical JSON: keys sorted, no whitespace, content and overrides sorted
/// by path. Two identical instances always hash the same.
pub fn hash(m: &Manifest) -> String {
    let mut m = m.clone();
    m.content.sort_by(|a, b| a.path.cmp(&b.path));
    m.overrides.sort_by(|a, b| a.path.cmp(&b.path));
    // serde_json::Value keeps object keys in a BTreeMap (sorted).
    let value = serde_json::to_value(&m).expect("manifest serializes");
    let bytes = serde_json::to_vec(&value).expect("value serializes");
    hex::encode(Sha256::digest(bytes))
}

// ── Changes between versions ───────────────────────────────────────────

/// What changed from `old` to `new`, by content key, for revision summaries.
pub fn changes(old: Option<&Manifest>, new: &Manifest) -> RevisionChanges {
    let label = |e: &Entry| {
        e.version_label
            .clone()
            .unwrap_or_else(|| e.path.rsplit('/').next().unwrap_or(&e.path).to_string())
    };
    let kind = |required: bool| if required { "Required" } else { "Optional" };
    let before: HashMap<&str, &Entry> = old
        .map(|m| m.content.iter().map(|e| (e.key(), e)).collect())
        .unwrap_or_default();
    let after: HashSet<&str> = new.content.iter().map(Entry::key).collect();
    let mut out = RevisionChanges::default();
    for e in &new.content {
        match before.get(e.key()) {
            None => out.added.push(e.name.clone()),
            Some(b) if b.sha1 != e.sha1 => out.updated.push(UpdatedItem {
                name: e.name.clone(),
                from: label(b),
                to: label(e),
            }),
            Some(b) if b.required != e.required => out.updated.push(UpdatedItem {
                name: e.name.clone(),
                from: kind(b.required).into(),
                to: kind(e.required).into(),
            }),
            Some(_) => {}
        }
    }
    if let Some(old) = old {
        for e in &old.content {
            if !after.contains(e.key()) {
                out.removed.push(e.name.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/manifest.example.json"
        ))
        .unwrap()
    }

    #[test]
    fn the_documented_example_is_valid() {
        let m = parse(&example()).unwrap();
        assert_eq!(m.content.len(), 4);
        assert_eq!(m.minecraft.loader, Loader::Fabric);
        let jei = &m.content[1];
        assert!(jei.required);
        assert_eq!(jei.key(), "238222");
        let pack = &m.content[3];
        assert_eq!(pack.source, ContentSource::Upload);
        assert_eq!(pack.key(), pack.sha256, "uploads are keyed by hash");
        assert!(!pack.required);
    }

    #[test]
    fn only_uploaded_mods_and_resource_packs_need_approval() {
        let mut e = parse(&example()).unwrap().content[3].clone();
        e.source = ContentSource::Upload;
        for (kind, needs) in [
            (ContentKind::Mod, true),
            (ContentKind::Resourcepack, true),
            (ContentKind::Shaderpack, false),
            (ContentKind::Datapack, false),
        ] {
            e.kind = kind;
            assert_eq!(e.needs_approval(), needs, "{kind:?}");
        }
        e.kind = ContentKind::Mod;
        e.source = ContentSource::Modrinth;
        assert!(!e.needs_approval());
    }

    #[test]
    fn file_pictures_are_uploads_or_on_the_mod_sites_image_hosts() {
        let mut m = parse(&example()).unwrap();
        let sha = "a".repeat(64);
        m.content[0].icon = Some(format!("sha256:{sha}"));
        m.content[1].icon = Some("https://cdn.modrinth.com/data/AANobbMI/icon.png".into());
        m.content[2].icon = Some("https://media.forgecdn.net/avatars/1/2/x.png".into());
        assert!(validate(&m).is_ok());
        assert!(m.blobs().contains(&sha), "uploaded pictures are blobs");
        for bad in [
            "https://evil.example/x.png",
            "http://cdn.modrinth.com/x.png",
            "sha256:nothex",
            "file:C:/x.png",
        ] {
            m.content[1].icon = Some(bad.into());
            assert!(validate(&m).is_err(), "{bad:?} should be refused");
        }
    }

    fn with_path(path: &str) -> Result<(), Invalid> {
        let mut m = parse(&example()).unwrap();
        m.content[0].path = path.into();
        validate(&m)
    }

    #[test]
    fn paths_that_could_escape_the_folder_are_refused() {
        for p in [
            "mods/../../evil.jar",
            "../mods/evil.jar",
            "mods/./x.jar",
            "/mods/x.jar",
            "C:/mods/x.jar",
            "mods\\x.jar",
            "mods//x.jar",
            "saves/x.jar",
            "mods/x.exe",
            "mods/con<1>.jar",
            "mods/sub./x.jar",
            "mods/sub /x.jar",
            "mods/CON.jar",
            "mods/nul",
            "mods/aux/x.jar",
            "mods/Com1.jar",
            "mods/lpt¹.jar",
            "mods/prn .x.jar",
            "",
        ] {
            assert!(with_path(p).is_err(), "{p:?} should be refused");
        }
        assert!(with_path("mods/sub/ok.jar").is_ok());
        assert!(with_path("mods/console.jar").is_ok());
        assert!(with_path("mods/com10.jar").is_ok());
    }

    #[test]
    fn content_is_in_its_own_kinds_folder() {
        let m = parse(&example()).unwrap();
        let with = |kind: ContentKind, path: &str| {
            let mut m = m.clone();
            m.content[3].kind = kind;
            m.content[3].path = path.into();
            validate(&m)
        };
        assert!(with(ContentKind::Shaderpack, "shaderpacks/x.zip").is_ok());
        assert!(with(ContentKind::Datapack, "datapacks/x.zip").is_ok());
        // A mod's bytes listed as a shader pack would install with no approval.
        assert!(with(ContentKind::Shaderpack, "mods/x.zip").is_err());
        assert!(with(ContentKind::Resourcepack, "shaderpacks/x.zip").is_err());
        assert!(with(ContentKind::Mod, "resourcepacks/x.jar").is_err());
    }

    #[test]
    fn hashes_are_lowercase_and_sizes_bounded() {
        let mut m = parse(&example()).unwrap();
        m.content[0].sha1 = m.content[0].sha1.to_uppercase();
        assert!(validate(&m).is_err(), "uppercase sha1");
        let mut m = parse(&example()).unwrap();
        m.overrides[0].sha256 = m.overrides[0].sha256.to_uppercase();
        assert!(validate(&m).is_err(), "uppercase sha256");
        let mut m = parse(&example()).unwrap();
        m.content[0].size_bytes = MAX_FILE_BYTES + 1;
        assert!(validate(&m).is_err(), "over 4 GB");
        assert!(is_sha1(&"a".repeat(40)) && !is_sha1(&"g".repeat(40)));
        assert!(is_sha256(&"0".repeat(64)) && !is_sha256(&"0".repeat(63)));
    }

    #[test]
    fn page_links_only_go_to_mod_sites() {
        let with_page = |url: &str| {
            let mut m = parse(&example()).unwrap();
            m.content[0].page_url = Some(url.into());
            validate(&m)
        };
        for url in [
            "file://server/share/setup.exe",
            "ms-settings:",
            "http://www.curseforge.com/minecraft/mc-mods/x/files/1",
            "https://www.curseforge.com.evil.example/x",
            "https://www.curseforge.com@evil.example/x",
        ] {
            assert!(with_page(url).is_err(), "{url}");
        }
        assert!(with_page("https://www.curseforge.com/minecraft/mc-mods/x/files/1").is_ok());
        assert!(with_page("https://modrinth.com/mod/sodium").is_ok());
        assert!(with_path("MODS/Upper.JAR").is_ok());
    }

    fn pack() -> PackBase {
        PackBase {
            source: ContentSource::Curseforge,
            project_id: "1298402".into(),
            version_id: "8854298".into(),
            name: "All the Mods 10: To the Sky".into(),
            version_label: Some("2.0.4".into()),
            download_url: "https://edge.forgecdn.net/files/8854/298/ATM10SKY-2.0.4.zip".into(),
            sha1: "f9f68e0cf0eb61326ea5b54a303386467fd885d0".into(),
            size_bytes: 56_892_260,
        }
    }

    #[test]
    fn files_from_the_pack() {
        let mut m = parse(&example()).unwrap();
        m.overrides[0].path = "fancymenu_data/menu.png".into();
        m.overrides[0].in_pack = true;
        assert!(validate(&m).is_err(), "only with a pack named");
        m.base = Some(pack());
        assert!(validate(&m).is_ok(), "any folder the pack has");
        assert!(
            !m.blobs().contains(&m.overrides[0].sha256),
            "and it isn't uploaded"
        );
        m.overrides[0].size_bytes = 50 * 1024 * 1024;
        assert!(validate(&m).is_ok(), "no size limit of its own");
        for risky in [
            "mods/extra.jar",
            "config/setup.ps1",
            "run.bat",
            "../evil.txt",
            "run.bat.",
            "run.bat ",
        ] {
            m.overrides[0].path = risky.into();
            assert!(validate(&m).is_err(), "{risky}");
        }

        let mut m = parse(&example()).unwrap();
        m.base = Some(PackBase {
            download_url: "https://evil.example/pack.zip".into(),
            ..pack()
        });
        assert!(validate(&m).is_err(), "download host allowlist");
    }

    #[test]
    fn mods_from_the_pack() {
        let mut m = parse(&example()).unwrap();
        let upload = m.content[3].clone();
        let mut jar = Entry {
            kind: ContentKind::Mod,
            path: "mods/agedaddition-1.1.3.jar".into(),
            sha256: "ab".repeat(32),
            in_pack: true,
            ..upload
        };
        m.content.push(jar.clone());
        assert!(validate(&m).is_err(), "only with a pack named");
        m.base = Some(pack());
        assert!(validate(&m).is_ok());
        assert!(!m.blobs().contains(&jar.sha256), "and it isn't uploaded");
        assert!(!jar.needs_approval(), "nor approved: it comes from the pack's download");

        jar.source = ContentSource::Modrinth;
        *m.content.last_mut().unwrap() = jar;
        assert!(
            validate(&m).is_err(),
            "only uploads stand in for the pack's copy"
        );

        assert!(can_be_pack_content("mods/agedaddition-1.1.3.jar"));
        assert!(!can_be_pack_content("config/a.toml"));
        assert!(!can_be_pack_content("mods/../evil.jar"));
    }

    #[test]
    fn old_manifests_hash_the_same() {
        let m = parse(&example()).unwrap();
        let json = serde_json::to_value(&m).unwrap();
        assert!(json.get("base").is_none());
        assert!(json["overrides"][0].get("inPack").is_none());
        assert!(json["content"][0].get("inPack").is_none());
    }

    #[test]
    fn duplicates_differing_only_in_case_are_refused() {
        let mut m = parse(&example()).unwrap();
        m.content[1].path = m.content[0].path.to_uppercase().replace("MODS/", "mods/");
        assert!(validate(&m).is_err());
    }

    #[test]
    fn other_rules() {
        let mut m = parse(&example()).unwrap();
        m.content[0].download_url = Some("https://evil.example/x.jar".into());
        assert!(validate(&m).is_err(), "download host allowlist");

        let mut m = parse(&example()).unwrap();
        m.content[0].source = ContentSource::Local;
        assert!(validate(&m).is_err());

        let long = "1".repeat(65);
        for odd in ["../../1.21.1", "1.21.1\"}", "1.21.1?x=1", long.as_str()] {
            let mut m = parse(&example()).unwrap();
            m.minecraft.version = odd.into();
            assert!(validate(&m).is_err(), "Minecraft {odd}");
            let mut m = parse(&example()).unwrap();
            m.minecraft.loader_version = Some(odd.into());
            assert!(validate(&m).is_err(), "loader {odd}");
            let mut m = parse(&example()).unwrap();
            m.content[0].version_id = Some(odd.into());
            assert!(validate(&m).is_err(), "version id {odd}");
        }
        for fine in [
            "1.21.1",
            "24w14a",
            "1.21.2-pre1",
            "0.16.5+build.1",
            "47.4.10",
            "1.20.1 forge",
        ] {
            let mut m = parse(&example()).unwrap();
            m.minecraft.loader_version = Some(fine.into());
            assert!(validate(&m).is_ok(), "{fine}");
        }

        let mut m = parse(&example()).unwrap();
        m.overrides[0].path = "config/run.bat".into();
        assert!(validate(&m).is_err(), "no programs");

        let mut m = parse(&example()).unwrap();
        m.overrides[0].path = "kubejs/assets/atm/textures/logo.png".into();
        assert!(validate(&m).is_ok(), "pictures next to scripts");

        let mut m = parse(&example()).unwrap();
        m.files = Some(FileRules {
            include: vec!["".into(), "config/".into(), "options.txt".into()],
            exclude: vec!["config/journeymap/".into()],
            ..Default::default()
        });
        assert!(validate(&m).is_ok(), "file choices");
        for odd in ["../secrets/", "C:/Users/", "/etc/", "config//x"] {
            m.files.as_mut().unwrap().every_update = vec![odd.into()];
            assert!(validate(&m).is_err(), "file choice {odd}");
        }

        let mut m = parse(&example()).unwrap();
        m.schema_version = 2;
        assert!(validate(&m).is_ok(), "an older version");
        m.schema_version = SCHEMA_VERSION;
        assert!(validate(&m).is_ok(), "this version");
        m.schema_version = SCHEMA_VERSION + 1;
        assert!(validate(&m).is_err(), "a newer one");

        assert!(parse(b"{}").is_err());
        assert!(parse(&vec![b' '; MAX_BYTES + 1]).is_err());
    }

    #[test]
    fn shareable_configs() {
        assert_eq!(upload_refusal("config/sodium-options.json", 100), None);
        assert_eq!(upload_refusal("kubejs/server_scripts/a.js", 100), None);
        assert_eq!(upload_refusal("config/fancymenu/assets/bg.png", 100), None);
        assert_eq!(
            upload_refusal("config/big.json", 9 * 1024 * 1024),
            Some(Refusal::TooBig)
        );
        assert_eq!(
            upload_refusal("config/native.dll", 100),
            Some(Refusal::Program)
        );
        assert_eq!(
            upload_refusal("libs/lwjgl.jar", 100),
            Some(Refusal::Program)
        );
        assert_eq!(upload_refusal("config/extractinator.jsonc", 100), None);
        assert_eq!(upload_refusal("config/sound.ini", 100), None);
        assert_eq!(upload_refusal("options.txt", 100), None, "anywhere now");
        assert_eq!(upload_refusal("global_packs/a/pack.mcmeta", 100), None);
        assert_eq!(upload_refusal("servers.dat", 100), None);
        assert_eq!(
            upload_refusal("data/servers.dat", 100),
            Some(Refusal::FileType)
        );
        assert_eq!(
            upload_refusal("config/.ReAuth.cfg", 100),
            Some(Refusal::Login)
        );
        assert_eq!(
            upload_refusal("command_history.txt", 100),
            Some(Refusal::Login)
        );
        for never in [
            "saves/World/level.dat",
            "logs/latest.log",
            "mods/extra.jar",
            "minecraftinstance.json",
        ] {
            assert_eq!(upload_refusal(never, 100), Some(Refusal::Folder), "{never}");
        }
        assert_eq!(
            upload_refusal("config/../options.txt", 100),
            Some(Refusal::Folder)
        );
        assert!(
            can_come_from_pack("options.txt"),
            "packs ship their own settings"
        );
        assert!(can_come_from_pack("fancymenu_data/menu.png"));
        assert!(!can_come_from_pack("mods/extra.jar"));
    }

    #[test]
    fn the_official_launchers_files_are_never_shared() {
        // A game folder can be the official launcher's own: what it runs is beside it.
        for path in [
            "launcher_profiles.json",
            "launcher_accounts.json",
            "versions/1.21.1/1.21.1.json",
            "libraries/org/x/x.json",
            "assets/indexes/17.json",
            "runtime/java-runtime-delta/windows-x64/jvm.cfg",
            "saves/World/level.dat",
            "minecraftinstance.json",
        ] {
            assert_eq!(upload_refusal(path, 100), Some(Refusal::Folder), "{path}");
            assert!(!can_come_from_pack(path), "{path}");
        }
        assert_eq!(
            upload_refusal("old/.minecraft/launcher_accounts_microsoft_store.json", 100),
            Some(Refusal::Login)
        );
        assert_eq!(upload_refusal("config/launcher_ideas.json", 100), None);
    }

    #[test]
    fn atlas_and_tandem_keep_their_own_folders() {
        // A crash-recovery journal is read from the staging folder at start-up.
        for path in [".atlas-staging/job/journal.json", ".tandem/state.json"] {
            assert_eq!(upload_refusal(path, 100), Some(Refusal::Folder), "{path}");
            assert!(!can_come_from_pack(path), "{path}");
        }
    }

    #[test]
    fn changes_between_versions() {
        let a = parse(&example()).unwrap();
        let first = changes(None, &a);
        assert_eq!(first.added.len(), a.content.len());
        let mut b = a.clone();
        b.content.remove(0);
        b.content[0].sha1 = "f".repeat(40);
        b.content[0].version_label = Some("2.0".into());
        b.content[1].required = !b.content[1].required;
        let c = changes(Some(&a), &b);
        assert_eq!(c.removed, vec![a.content[0].name.clone()]);
        assert_eq!(c.updated.len(), 2);
        assert_eq!(c.updated[0].to, "2.0");
        assert!(c.added.is_empty());
    }

    #[test]
    fn hash_ignores_order() {
        let a = parse(&example()).unwrap();
        let mut b = a.clone();
        b.content.reverse();
        assert_eq!(hash(&a), hash(&b));
        b.content[0].sha1 = "f".repeat(40);
        assert_ne!(hash(&a), hash(&b));
    }
}
