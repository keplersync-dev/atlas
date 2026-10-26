//! The manifest: a shareable description of an instance (see
//! tests/fixtures/manifest.example.json). The server validates every manifest it stores
//! and the app validates every one it applies, both with this code.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api::{RevisionChanges, UpdatedItem};
use crate::enums::{ContentKind, ContentSource, Loader, Side};

pub const SCHEMA_VERSION: u32 = 1;

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
}

impl Manifest {
    /// The instance picture, when it's one uploaded to Atlas: `icon` is "sha256:<hex>".
    /// Anything else (older manifests used icon keys like "leaf") isn't a picture.
    pub fn icon_sha256(&self) -> Option<&str> {
        self.icon
            .as_deref()?
            .strip_prefix("sha256:")
            .filter(|h| is_hex(h, 64))
    }

    /// Every uploaded file this version needs: uploads, config files (not those from the
    /// modpack) and the picture.
    pub fn blobs(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .content
            .iter()
            .filter(|e| e.source == ContentSource::Upload)
            .map(|e| e.sha256.clone())
            .chain(
                self.overrides
                    .iter()
                    .filter(|o| !o.in_pack)
                    .map(|o| o.sha256.clone()),
            )
            .chain(self.icon_sha256().map(str::to_owned))
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
}

fn is_false(b: &bool) -> bool {
    !*b
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

pub const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_CONTENT: usize = 2000;
/// Config files, scripts and the like, from the pack and uploaded together. A big pack
/// (All the Mods) has a few thousand.
pub const MAX_OVERRIDES: usize = 10_000;
/// The largest one uploaded file of those. Files from the pack have no limit of their own.
pub const MAX_OVERRIDE_BYTES: u64 = 8 * 1024 * 1024;
/// All uploaded ones together.
pub const MAX_OVERRIDES_TOTAL: u64 = 150 * 1024 * 1024;
/// The largest instance picture that can be shared.
pub const MAX_ICON_BYTES: u64 = 1024 * 1024;
const CONTENT_DIRS: [&str; 4] = ["mods/", "resourcepacks/", "shaderpacks/", "datapacks/"];
/// Where uploaded files may go: configs, and the scripts and assets packs keep next to them.
pub const OVERRIDE_DIRS: [&str; 4] = ["config/", "defaultconfigs/", "kubejs/", "scripts/"];
/// What uploaded files may be: text, data and pictures, never programs.
const OVERRIDE_EXTS: [&str; 22] = [
    "json",
    "json5",
    "toml",
    "cfg",
    "properties",
    "txt",
    "yml",
    "yaml",
    "snbt",
    "js",
    "zs",
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
/// Never written from a pack either: programs and scripts Windows runs.
const RISKY_EXTS: [&str; 16] = [
    "exe", "dll", "bat", "cmd", "com", "scr", "ps1", "psm1", "vbs", "vbe", "wsf", "msi", "lnk",
    "reg", "jar", "sh",
];
const DOWNLOAD_HOSTS: [&str; 3] = [
    "cdn.modrinth.com",
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
];

/// Why a manifest was refused, as a sentence players can read.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

/// Parse and validate a manifest from JSON bytes.
pub fn parse(bytes: &[u8]) -> Result<Manifest, Invalid> {
    if bytes.len() > MAX_BYTES {
        return Err(Invalid("The list of files is over 4 MB.".into()));
    }
    let m: Manifest = serde_json::from_slice(bytes)
        .map_err(|_| Invalid("The list of files is damaged.".into()))?;
    validate(&m)?;
    Ok(m)
}

pub fn validate(m: &Manifest) -> Result<(), Invalid> {
    let bad = |s: String| Err(Invalid(s));
    if m.schema_version != SCHEMA_VERSION {
        return bad("It was made by a newer Atlas. Update Atlas.".into());
    }
    if m.content.len() > MAX_CONTENT || m.overrides.len() > MAX_OVERRIDES {
        return bad(format!(
            "It has too many files: at most {MAX_CONTENT} mods and packs, and {MAX_OVERRIDES} config files."
        ));
    }
    if let Some(b) = &m.base {
        if !matches!(
            b.source,
            ContentSource::Modrinth | ContentSource::Curseforge
        ) || b.project_id.is_empty()
            || b.version_id.is_empty()
            || b.name.trim().is_empty()
            || !is_hex(&b.sha1, 40)
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
        check_path(&e.path, Some(&CONTENT_DIRS))?;
        let ext = if e.kind == ContentKind::Mod {
            "jar"
        } else {
            "zip"
        };
        if !e.path.to_ascii_lowercase().ends_with(&format!(".{ext}")) {
            return bad(format!("{} has to be a .{ext} file.", e.path));
        }
        if !is_hex(&e.sha1, 40) || !is_hex(&e.sha256, 64) {
            return bad(format!("{} couldn't be read properly. Try again.", e.path));
        }
        if e.source == ContentSource::Local {
            return bad(format!("{} is set to Only me.", e.path));
        }
        if let Some(url) = &e.download_url
            && !allowed_download(url)
        {
            return bad(format!(
                "{} downloads from a site Atlas doesn't allow.",
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
            check_path(&o.path, Some(&OVERRIDE_DIRS))?;
            if !shareable_type(&o.path) {
                return bad(format!("{} isn't a file type Atlas shares.", o.path));
            }
            if o.size_bytes > MAX_OVERRIDE_BYTES {
                return bad(format!(
                    "{} is over 8 MB. Leave it out under Config files.",
                    o.path
                ));
            }
            total += o.size_bytes;
        }
        if !is_hex(&o.sha256, 64) {
            return bad(format!("{} couldn't be read properly. Try again.", o.path));
        }
        if !seen.insert(o.path.to_lowercase()) {
            return bad(format!("{} is in it twice.", o.path));
        }
    }
    if total > MAX_OVERRIDES_TOTAL {
        return bad(
            "The uploaded config files add up to more than 150 MB. Leave some out under Config files."
                .into(),
        );
    }
    Ok(())
}

/// Why a file can't be uploaded as a config override, if it can't: its folder, its type
/// or its size. (The count and total limits apply to the whole set.)
pub fn upload_refusal(path: &str, size_bytes: u64) -> Option<Refusal> {
    if check_path(path, Some(&OVERRIDE_DIRS)).is_err() {
        Some(Refusal::Folder)
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
    Folder,
    FileType,
    TooBig,
}

/// Whether a modpack's own file may be written from the pack: a safe path, not where
/// mods and packs go (those are content), and nothing Windows runs.
pub fn can_come_from_pack(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    check_path(path, None).is_ok()
        && !CONTENT_DIRS.iter().any(|d| lower.starts_with(d))
        && !RISKY_EXTS.contains(&extension(path).as_str())
}

fn shareable_type(path: &str) -> bool {
    OVERRIDE_EXTS.contains(&extension(path).as_str())
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
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return bad("empty, . or .. segment");
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

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A download link a manifest may carry: https on Modrinth's or CurseForge's CDN.
pub fn allowed_download(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    DOWNLOAD_HOSTS.contains(&host)
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
            "",
        ] {
            assert!(with_path(p).is_err(), "{p:?} should be refused");
        }
        assert!(with_path("mods/sub/ok.jar").is_ok());
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
    fn old_manifests_hash_the_same() {
        let m = parse(&example()).unwrap();
        let json = serde_json::to_value(&m).unwrap();
        assert!(json.get("base").is_none());
        assert!(json["overrides"][0].get("inPack").is_none());
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

        let mut m = parse(&example()).unwrap();
        m.overrides[0].path = "config/run.bat".into();
        assert!(validate(&m).is_err(), "no programs");

        let mut m = parse(&example()).unwrap();
        m.overrides[0].path = "kubejs/assets/atm/textures/logo.png".into();
        assert!(validate(&m).is_ok(), "pictures next to scripts");

        let mut m = parse(&example()).unwrap();
        m.schema_version = 2;
        assert!(validate(&m).is_err());

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
            Some(Refusal::FileType)
        );
        assert_eq!(upload_refusal("options.txt", 100), Some(Refusal::Folder));
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
