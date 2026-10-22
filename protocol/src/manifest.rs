//! The manifest: a shareable description of an instance (see
//! tests/fixtures/manifest.example.json). The server validates every manifest it stores
//! and the app validates every one it applies, both with this code.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
}

// ── Validation ─────────────────────────────────────
// A manifest decides where files are written on other people's PCs: treat it as hostile.

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_CONTENT: usize = 2000;
const MAX_OVERRIDES: usize = 500;
const MAX_OVERRIDE_BYTES: u64 = 1024 * 1024;
const MAX_OVERRIDES_TOTAL: u64 = 20 * 1024 * 1024;
const CONTENT_DIRS: [&str; 4] = ["mods/", "resourcepacks/", "shaderpacks/", "datapacks/"];
const OVERRIDE_DIRS: [&str; 3] = ["config/", "defaultconfigs/", "kubejs/"];
const OVERRIDE_EXTS: [&str; 11] = [
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
    "conf",
];
const DOWNLOAD_HOSTS: [&str; 3] = [
    "cdn.modrinth.com",
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
];

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

/// Parse and validate a manifest from JSON bytes.
pub fn parse(bytes: &[u8]) -> Result<Manifest, Invalid> {
    if bytes.len() > MAX_BYTES {
        return Err(Invalid("manifest is larger than 2 MB".into()));
    }
    let m: Manifest =
        serde_json::from_slice(bytes).map_err(|e| Invalid(format!("not a manifest: {e}")))?;
    validate(&m)?;
    Ok(m)
}

pub fn validate(m: &Manifest) -> Result<(), Invalid> {
    let bad = |s: String| Err(Invalid(s));
    if m.schema_version != SCHEMA_VERSION {
        return bad(format!("unknown schemaVersion {}", m.schema_version));
    }
    if m.content.len() > MAX_CONTENT || m.overrides.len() > MAX_OVERRIDES {
        return bad("too many files".into());
    }
    let mut seen = HashSet::new();
    for e in &m.content {
        check_path(&e.path, &CONTENT_DIRS)?;
        let ext = if e.kind == ContentKind::Mod {
            "jar"
        } else {
            "zip"
        };
        if !e.path.to_ascii_lowercase().ends_with(&format!(".{ext}")) {
            return bad(format!("{}: a {:?} must be a .{ext} file", e.path, e.kind));
        }
        if !is_hex(&e.sha1, 40) || !is_hex(&e.sha256, 64) {
            return bad(format!("{}: bad hash", e.path));
        }
        if e.source == ContentSource::Local {
            return bad(format!("{}: local files can't be in a manifest", e.path));
        }
        if let Some(url) = &e.download_url
            && !allowed_download(url)
        {
            return bad(format!(
                "{}: downloads from that host aren't allowed",
                e.path
            ));
        }
        if !seen.insert(e.path.to_lowercase()) {
            return bad(format!("{}: listed twice", e.path));
        }
    }
    let mut total = 0;
    for o in &m.overrides {
        check_path(&o.path, &OVERRIDE_DIRS)?;
        let ext = o.path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if !OVERRIDE_EXTS.contains(&ext.as_str()) {
            return bad(format!("{}: only text config files can be shared", o.path));
        }
        if o.size_bytes > MAX_OVERRIDE_BYTES || !is_hex(&o.sha256, 64) {
            return bad(format!("{}: too big or bad hash", o.path));
        }
        total += o.size_bytes;
        if !seen.insert(o.path.to_lowercase()) {
            return bad(format!("{}: listed twice", o.path));
        }
    }
    if total > MAX_OVERRIDES_TOTAL {
        return bad("config files add up to more than 20 MB".into());
    }
    Ok(())
}

fn check_path(path: &str, dirs: &[&str]) -> Result<(), Invalid> {
    let bad = |why: &str| Err(Invalid(format!("{path}: {why}")));
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
    if !dirs.iter().any(|d| lower.starts_with(d)) {
        return bad("not in an allowed folder");
    }
    Ok(())
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn allowed_download(url: &str) -> bool {
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
        assert!(validate(&m).is_err(), "only text configs");

        let mut m = parse(&example()).unwrap();
        m.schema_version = 2;
        assert!(validate(&m).is_err());

        assert!(parse(b"{}").is_err());
        assert!(parse(&vec![b' '; MAX_BYTES + 1]).is_err());
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
