//! The official Minecraft Launcher.
//!
//! <root>/launcher_profiles.json → profiles{id: {name, type, gameDir?, lastVersionId, lastUsed, icon}}
//! No gameDir = the root itself. Several profiles often share one game dir, but Atlas
//! tracks folders, so each game dir is one instance, named after its most recently
//! used profile. Checked against the launcher on Windows, 2026.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::util::{env_dir, first_existing, parse_time};
use super::{FoundInstance, ReadResult};
use crate::dto::Loader;

pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(appdata) = env_dir("APPDATA") {
        roots.push(appdata.join(".minecraft"));
    }
    // Microsoft Store / Xbox app version of the launcher.
    if let Some(local) = env_dir("LOCALAPPDATA") {
        roots.push(
            local.join(r"Packages\Microsoft.4297127D64EC6_8wekyb3d8bbwe\LocalCache\Roaming\.minecraft"),
        );
    }
    roots
}

pub fn is_root(path: &Path) -> bool {
    path.join("launcher_profiles.json").is_file()
}

pub fn program() -> Option<PathBuf> {
    let x86 = env_dir("ProgramFiles(x86)").map(|d| d.join(r"Minecraft Launcher\MinecraftLauncher.exe"));
    first_existing(
        [Some(PathBuf::from(r"C:\XboxGames\Minecraft Launcher\Content\Minecraft.exe")), x86]
            .into_iter()
            .flatten(),
    )
}

#[derive(Deserialize)]
struct Profiles {
    #[serde(default)]
    profiles: HashMap<String, Profile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    #[serde(default)]
    name: String,
    #[serde(rename = "type", default)]
    kind: String,
    game_dir: Option<String>,
    last_version_id: Option<String>,
    last_used: Option<String>,
    icon: Option<String>,
}

pub fn list(root: &Path) -> ReadResult<Vec<FoundInstance>> {
    let file = root.join("launcher_profiles.json");
    if !file.is_file() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(&file)?;
    let parsed: Profiles = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;

    // One instance per game dir: keep its most recently used profile.
    let mut by_dir: HashMap<String, (PathBuf, Profile, Option<String>)> = HashMap::new();
    for profile in parsed.profiles.into_values() {
        let dir = profile
            .game_dir
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| root.to_path_buf());
        let used = profile.last_used.as_deref().and_then(parse_time);
        let key = dir.to_string_lossy().to_lowercase();
        match by_dir.get(&key) {
            Some((_, _, best)) if *best >= used => {}
            _ => {
                by_dir.insert(key, (dir, profile, used));
            }
        }
    }

    Ok(by_dir
        .into_values()
        .map(|(game_dir, p, used)| {
            let version_id = p.last_version_id.clone().unwrap_or_default();
            let (minecraft_version, loader) = parse_version_id(&version_id);
            FoundInstance {
                launcher_instance_id: game_dir.to_string_lossy().into_owned(),
                name: display_name(&p),
                game_dir,
                // Custom pictures are embedded images; named ones ("Grass") are built into
                // the launcher and have no file Atlas can show.
                icon: p.icon.filter(|i| i.starts_with("data:image/")),
                minecraft_version,
                loader: loader.as_ref().map(|(l, _)| *l),
                loader_version: loader.and_then(|(_, v)| v),
                last_played_at: used,
                total_playtime_seconds: None, // the launcher doesn't record it
            }
        })
        .collect())
}

fn display_name(p: &Profile) -> String {
    if !p.name.trim().is_empty() {
        return p.name.clone();
    }
    match p.kind.as_str() {
        "latest-release" => "Latest release".into(),
        "latest-snapshot" => "Latest snapshot".into(),
        _ => p.last_version_id.clone().unwrap_or_else(|| "Minecraft".into()),
    }
}

/// "fabric-loader-0.16.9-1.17.1" → (Some("1.17.1"), Some((Fabric, Some("0.16.9")))).
/// "latest-release" has no fixed version: (None, Some((Vanilla, None))).
fn parse_version_id(id: &str) -> (Option<String>, Option<(Loader, Option<String>)>) {
    let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
    if id.starts_with("latest-") {
        return (None, Some((Loader::Vanilla, None)));
    }
    for (prefix, loader) in [("fabric-loader-", Loader::Fabric), ("quilt-loader-", Loader::Quilt)] {
        if let Some(rest) = id.strip_prefix(prefix) {
            let (version, mc) = rest.split_once('-').unwrap_or((rest, ""));
            return (some(mc), Some((loader, some(version))));
        }
    }
    if let Some(version) = id.strip_prefix("neoforge-") {
        return (neoforge_minecraft(version), Some((Loader::Neoforge, some(version))));
    }
    // "1.20.1-forge-47.2.0", older "1.12.2-forge1.12.2-14.23.5.2860"
    if let Some((mc, rest)) = id.split_once("-forge") {
        let version = rest.rsplit('-').next().unwrap_or("");
        return (some(mc), Some((Loader::Forge, some(version))));
    }
    if id.starts_with(|c: char| c.is_ascii_digit()) {
        // Plain versions and snapshots ("1.21.1", "24w39a") are vanilla. Anything with
        // extra parts (OptiFine etc.) keeps its Minecraft version, loader unknown.
        return match id.split_once('-') {
            None => (some(id), Some((Loader::Vanilla, None))),
            // pre-releases and release candidates are vanilla too: "1.21.2-pre3", "1.21.7-rc1"
            Some((_, tag)) if tag.starts_with("pre") || tag.starts_with("rc") => {
                (some(id), Some((Loader::Vanilla, None)))
            }
            Some((mc, _)) => (some(mc), None),
        };
    }
    (None, None)
}

/// NeoForge 21.1.x is for Minecraft 1.21.1, 20.4.x for 1.20.4, 21.0.x for 1.21.
fn neoforge_minecraft(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    if !(20..26).contains(&major) {
        return None;
    }
    Some(if minor == 0 { format!("1.{major}") } else { format!("1.{major}.{minor}") })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/official")
    }

    #[test]
    fn one_instance_per_game_dir_named_after_its_latest_profile() {
        let root = fixture();
        assert!(is_root(&root));
        let mut list = list(&root).unwrap();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(list.len(), 2, "{list:#?}");

        // three profiles share the root; "latest-release" was used most recently
        let main = &list[1];
        assert_eq!(main.name, "Latest release");
        assert_eq!(main.game_dir, root);
        assert_eq!(main.launcher_instance_id, root.to_string_lossy());
        assert_eq!(main.minecraft_version, None);
        assert_eq!(main.loader, Some(Loader::Vanilla));
        assert_eq!(main.icon, None, "built-in icon name");
        assert_eq!(main.last_played_at.as_deref(), Some("2025-12-31T13:02:19.217Z"));

        // its own game dir and a custom (embedded) picture
        let fabric = &list[0];
        assert_eq!(fabric.name, "Fabric 1.21.1");
        assert_eq!(fabric.game_dir, PathBuf::from(r"C:\Games\fabric-1.21.1"));
        assert_eq!(fabric.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(fabric.loader, Some(Loader::Fabric));
        assert_eq!(fabric.loader_version.as_deref(), Some("0.16.9"));
        assert_eq!(fabric.icon.as_deref(), Some("data:image/png;base64,iVBORw0KGgo="));
        assert_eq!(fabric.last_played_at.as_deref(), Some("2024-11-01T12:43:39.000Z"));
    }

    #[test]
    fn version_ids() {
        let v = |id| parse_version_id(id);
        assert_eq!(v("1.21.1"), (Some("1.21.1".into()), Some((Loader::Vanilla, None))));
        assert_eq!(v("24w39a"), (Some("24w39a".into()), Some((Loader::Vanilla, None))));
        assert_eq!(v("1.21.2-pre3"), (Some("1.21.2-pre3".into()), Some((Loader::Vanilla, None))));
        assert_eq!(
            v("quilt-loader-0.26.0-1.20.1"),
            (Some("1.20.1".into()), Some((Loader::Quilt, Some("0.26.0".into()))))
        );
        assert_eq!(
            v("1.20.1-forge-47.2.0"),
            (Some("1.20.1".into()), Some((Loader::Forge, Some("47.2.0".into()))))
        );
        assert_eq!(
            v("1.12.2-forge1.12.2-14.23.5.2860"),
            (Some("1.12.2".into()), Some((Loader::Forge, Some("14.23.5.2860".into()))))
        );
        assert_eq!(
            v("neoforge-21.1.77"),
            (Some("1.21.1".into()), Some((Loader::Neoforge, Some("21.1.77".into()))))
        );
        assert_eq!(v("neoforge-21.0.10").0, Some("1.21".into()));
        assert_eq!(v("1.20.1-OptiFine_HD_U_I6"), (Some("1.20.1".into()), None));
        assert_eq!(v("latest-snapshot"), (None, Some((Loader::Vanilla, None))));
        assert_eq!(v(""), (None, None));
    }
}
