//! CurseForge app.
//!
//! <root>/Instances/<folder>/minecraftinstance.json — the folder is the game dir.
//! Fields used: name, gameVersion, baseModLoader.name ("neoforge-21.1.250",
//! "forge-47.4.0", "fabric-0.16.5"), lastPlayed ("0001-01-01T00:00:00" = never),
//! profileImagePath (a custom picture, usually null), installedModpack.thumbnailUrl.
//! Checked against CurseForge on Windows, 2026. Files may start with a UTF-8 BOM.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::util::{env_dir, first_existing, loader_from, parse_time};
use super::{FoundInstance, ReadResult};
use crate::dto::Loader;

pub fn default_roots() -> Vec<PathBuf> {
    let Some(home) = env_dir("USERPROFILE") else { return vec![] };
    vec![
        home.join("curseforge").join("minecraft"),
        home.join("Documents").join("curseforge").join("minecraft"),
    ]
}

/// Instances/ alone isn't enough: Windows paths ignore case, so Prism's `instances/`
/// would match too. A CurseForge root also has the `Install/` folder it keeps runtimes in.
pub fn is_root(path: &Path) -> bool {
    path.join("Instances").is_dir() && path.join("Install").is_dir()
}

pub fn program() -> Option<PathBuf> {
    let local = env_dir("LOCALAPPDATA")?;
    first_existing([
        local.join("Programs").join("CurseForge Windows").join("CurseForge.exe"),
        local.join("Programs").join("CurseForge").join("CurseForge.exe"),
    ])
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Instance {
    name: Option<String>,
    game_version: Option<String>,
    base_mod_loader: Option<BaseModLoader>,
    last_played: Option<String>,
    profile_image_path: Option<String>,
    installed_modpack: Option<InstalledModpack>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledModpack {
    thumbnail_url: Option<String>,
}

#[derive(Deserialize)]
struct BaseModLoader {
    name: Option<String>,
}

pub fn list(root: &Path) -> ReadResult<Vec<FoundInstance>> {
    let dir = root.join("Instances");
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        let file = path.join("minecraftinstance.json");
        let Some(folder) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if !file.is_file() {
            continue;
        }
        match read_instance(&file) {
            Ok(i) => found.push(FoundInstance {
                name: i.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| folder.clone()),
                launcher_instance_id: folder,
                game_dir: path,
                icon: i.icon,
                minecraft_version: i.game_version,
                loader: i.loader.as_ref().map(|(l, _)| *l).or(Some(Loader::Vanilla)),
                loader_version: i.loader.and_then(|(_, v)| v),
                last_played_at: i.last_played.as_deref().and_then(parse_time),
                total_playtime_seconds: None, // CurseForge doesn't record it
            }),
            Err(e) => tracing::warn!(instance = %folder, error = %e, "skipping unreadable CurseForge instance"),
        }
    }
    Ok(found)
}

struct Read {
    icon: Option<String>,
    name: Option<String>,
    game_version: Option<String>,
    loader: Option<(Loader, Option<String>)>,
    last_played: Option<String>,
}

fn read_instance(file: &Path) -> ReadResult<Read> {
    let text = std::fs::read_to_string(file)?;
    let i: Instance = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    let loader = i
        .base_mod_loader
        .and_then(|b| b.name)
        .and_then(|n| parse_loader(&n));
    // A picture the user set wins; otherwise the modpack's thumbnail from CurseForge.
    let custom = i.profile_image_path.filter(|p| Path::new(p).is_file()).map(|p| format!("file:{p}"));
    let thumbnail = i.installed_modpack.and_then(|m| m.thumbnail_url).filter(|u| u.starts_with("https://"));
    Ok(Read { icon: custom.or(thumbnail), name: i.name, game_version: i.game_version, loader, last_played: i.last_played })
}

/// "neoforge-21.1.250" → (NeoForge, "21.1.250").
fn parse_loader(name: &str) -> Option<(Loader, Option<String>)> {
    let (kind, version) = name.split_once('-').unwrap_or((name, ""));
    let loader = loader_from(kind)?;
    Some((loader, (!version.is_empty()).then(|| version.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/curseforge")
    }

    #[test]
    fn reads_instances_with_bom_and_never_played() {
        let root = fixture();
        assert!(is_root(&root));
        let mut list = list(&root).unwrap();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(list.len(), 2, "{list:#?}");

        let atm = &list[0];
        assert_eq!(atm.name, "All the Mods 10");
        assert_eq!(atm.launcher_instance_id, "All the Mods 10 To the Sky");
        assert_eq!(atm.game_dir, root.join("Instances/All the Mods 10 To the Sky"));
        assert_eq!(atm.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(atm.loader, Some(Loader::Neoforge));
        assert_eq!(atm.loader_version.as_deref(), Some("21.1.250"));
        assert_eq!(atm.last_played_at.as_deref(), Some("2026-09-22T01:15:38.895Z"));
        assert_eq!(atm.icon.as_deref(), Some("https://media.forgecdn.net/avatars/thumbnails/1389/739/256/256/63890103359327238"));

        // folder name differs from the instance name; never played
        let sf = &list[1];
        assert_eq!(sf.name, "SkyFactory 4 fixed");
        assert_eq!(sf.launcher_instance_id, "SkyFactory 4 (1)");
        assert_eq!(sf.loader, Some(Loader::Forge));
        assert_eq!(sf.loader_version.as_deref(), Some("14.23.5.2860"));
        assert_eq!(sf.last_played_at, None);
        assert_eq!(sf.icon, None, "no thumbnail, no custom picture");
    }

    #[test]
    fn loader_names() {
        assert_eq!(parse_loader("fabric-0.16.5"), Some((Loader::Fabric, Some("0.16.5".into()))));
        assert_eq!(parse_loader("quilt-0.26.0"), Some((Loader::Quilt, Some("0.26.0".into()))));
        assert_eq!(parse_loader("weird-1.0"), None);
    }
}
