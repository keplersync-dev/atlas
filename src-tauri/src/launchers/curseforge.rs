//! CurseForge app.
//!
//! <root>/Instances/<folder>/minecraftinstance.json — the folder is the game dir.
//! Fields used: name, gameVersion, baseModLoader.name ("neoforge-21.1.250",
//! "forge-47.4.0", "fabric-0.16.5"), lastPlayed ("0001-01-01T00:00:00" = never),
//! profileImagePath (a custom picture, usually null), installedModpack.thumbnailUrl.
//! Checked against CurseForge on Windows, 2026. Files may start with a UTF-8 BOM.
//!
//! New instances: a folder with a minecraftinstance.json shaped like the ones CurseForge
//! writes for a custom profile. `baseModLoader` is CurseForge's own install data for the
//! loader (from its API, through the Atlas server, which holds the key); CurseForge
//! installs the loader from it on first play.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::util::{
    any_running, env_dir, first_existing, free_folder, image_ext, loader_from, parse_time,
};
use super::{FoundInstance, NewInstance, ReadError, ReadResult};
use crate::dto::Loader;

pub fn default_roots() -> Vec<PathBuf> {
    let Some(home) = env_dir("USERPROFILE") else {
        return vec![];
    };
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
        local
            .join("Programs")
            .join("CurseForge Windows")
            .join("CurseForge.exe"),
        local
            .join("Programs")
            .join("CurseForge")
            .join("CurseForge.exe"),
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
    #[serde(rename = "addonID")]
    addon_id: Option<u64>,
    name: Option<String>,
    allow_mod_distribution: Option<bool>,
    installed_file: Option<InstalledFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledFile {
    id: Option<u64>,
    download_url: Option<String>,
    file_length: Option<u64>,
    display_name: Option<String>,
    file_name_on_disk: Option<String>,
    #[serde(default)]
    hashes: Vec<FileHash>,
}

#[derive(Deserialize)]
struct FileHash {
    /// 1 = sha1, 2 = md5.
    #[serde(rename = "type")]
    kind: u32,
    value: String,
}

/// The modpack CurseForge installed the instance from, with its download. None when it
/// wasn't from one, or the pack's author doesn't allow downloads outside CurseForge.
pub fn installed_pack(instance_dir: &Path) -> Option<super::InstalledPack> {
    let text = std::fs::read_to_string(instance_dir.join("minecraftinstance.json")).ok()?;
    let i: Instance = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let pack = i.installed_modpack?;
    if pack.allow_mod_distribution == Some(false) {
        return None;
    }
    let file = pack.installed_file?;
    let sha1 = file
        .hashes
        .iter()
        .find(|h| h.kind == 1)
        .map(|h| h.value.to_ascii_lowercase())?;
    // The pack's own manifest has the version players know ("2.0.4").
    let version_label = std::fs::read_to_string(instance_dir.join("manifest.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|m| m["version"].as_str().map(str::to_owned))
        .or(file.display_name)
        .or(file.file_name_on_disk);
    Some(super::InstalledPack::Known(
        atlas_protocol::manifest::PackBase {
            source: atlas_protocol::enums::ContentSource::Curseforge,
            project_id: pack.addon_id?.to_string(),
            version_id: file.id?.to_string(),
            name: pack.name?.trim().to_owned(),
            version_label,
            download_url: file.download_url?,
            sha1,
            size_bytes: file.file_length?,
        },
    ))
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
                name: i
                    .name
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| folder.clone()),
                launcher_instance_id: folder,
                game_dir: path,
                icon: i.icon,
                minecraft_version: i.game_version,
                loader: i.loader.as_ref().map(|(l, _)| *l).or(Some(Loader::Vanilla)),
                loader_version: i.loader.and_then(|(_, v)| v),
                last_played_at: i.last_played.as_deref().and_then(parse_time),
                total_playtime_seconds: None, // CurseForge doesn't record it
            }),
            Err(e) => {
                tracing::warn!(instance = %folder, error = %e, "skipping unreadable CurseForge instance")
            }
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
        .and_then(|n| parse_loader(&n, i.game_version.as_deref()));
    // A picture the user set wins; otherwise the modpack's thumbnail from CurseForge.
    let custom = i
        .profile_image_path
        .filter(|p| Path::new(p).is_file())
        .map(|p| format!("file:{p}"));
    let thumbnail = i
        .installed_modpack
        .and_then(|m| m.thumbnail_url)
        .filter(|u| u.starts_with("https://"));
    Ok(Read {
        icon: custom.or(thumbnail),
        name: i.name,
        game_version: i.game_version,
        loader,
        last_played: i.last_played,
    })
}

/// "neoforge-21.1.250" → (NeoForge, "21.1.250"). Fabric's names end in the Minecraft
/// version: "fabric-0.16.5-1.21.1" → (Fabric, "0.16.5").
fn parse_loader(name: &str, minecraft: Option<&str>) -> Option<(Loader, Option<String>)> {
    let (kind, version) = name.split_once('-').unwrap_or((name, ""));
    let loader = loader_from(kind)?;
    let version = minecraft
        .and_then(|mc| version.strip_suffix(mc)?.strip_suffix('-'))
        .unwrap_or(version);
    Some((loader, (!version.is_empty()).then(|| version.to_string())))
}

/// Is the CurseForge app open? It may not notice an instance folder added meanwhile.
pub fn running() -> bool {
    any_running(&["curseforge.exe"])
}

/// The parts of CurseForge's loader data that it keeps in `baseModLoader`.
const LOADER_FIELDS: [&str; 12] = [
    "forgeVersion",
    "name",
    "type",
    "downloadUrl",
    "filename",
    "installMethod",
    "latest",
    "recommended",
    "versionJson",
    "librariesInstallLocation",
    "minecraftVersion",
    "installProfileJson",
];

/// Create an empty instance for a shared one. `loader_data` is CurseForge's data for the
/// loader version (None for vanilla). Returns the instance's folder name.
pub fn create(root: &Path, new: &NewInstance, loader_data: Option<Value>) -> ReadResult<String> {
    let base_mod_loader = match (new.loader, loader_data) {
        (Loader::Vanilla, _) => Value::Null,
        (_, Some(Value::Object(data))) => {
            let kept: Map<String, Value> = LOADER_FIELDS
                .iter()
                .filter_map(|&k| Some((k.to_owned(), data.get(k)?.clone())))
                .collect();
            if kept.get("versionJson").and_then(Value::as_str).is_none() {
                return Err(ReadError::Format(
                    "CurseForge's loader data has no version file".into(),
                ));
            }
            Value::Object(kept)
        }
        _ => {
            return Err(ReadError::Format(
                "no CurseForge loader data for a modded instance".into(),
            ));
        }
    };

    let instances = root.join("Instances");
    std::fs::create_dir_all(&instances)?;
    let folder = free_folder(&instances, &new.name);
    let dir = instances.join(&folder);
    std::fs::create_dir_all(&dir)?;

    // The picture goes in the instance folder; CurseForge shows it as the profile image.
    let image = match new.icon.as_deref().and_then(|b| Some((b, image_ext(b)?))) {
        Some((bytes, ext)) => {
            let file = dir.join(format!("atlas-icon.{ext}"));
            std::fs::write(&file, bytes)?;
            Value::String(file.to_string_lossy().into_owned())
        }
        None => Value::Null,
    };
    const NEVER: &str = "0001-01-01T00:00:00";
    // Two halves: one json! this long goes past the macro recursion limit.
    let mut instance = json!({
        "baseModLoader": base_mod_loader,
        "isUnlocked": true,
        "javaArgsOverride": null,
        "lastPlayed": NEVER,
        "playedCount": 0,
        "timePlayed": 0,
        "manifest": null,
        "fileDate": NEVER,
        "installedModpack": null,
        "projectID": 0,
        "fileID": 0,
        "customAuthor": null,
        "modpackOverrides": [],
        "isMemoryOverride": false,
        "allocatedMemory": 0,
        "profileImagePath": image,
        "groupId": null,
        "isVanilla": new.loader == Loader::Vanilla,
        "guid": uuid::Uuid::now_v7().to_string(),
        "gameTypeID": 432,
        "installPath": format!("{}\\", dir.display()),
        "name": new.name,
    });
    let rest = json!({
        "cachedScans": [],
        "isValid": true,
        "lastPreviousMatchUpdate": NEVER,
        "lastRefreshAttempt": NEVER,
        "isEnabled": true,
        "gameVersion": new.minecraft,
        "gameVersionFlavor": null,
        "gameVersionTypeId": null,
        "preferenceAlternateFile": false,
        "preferenceAutoInstallUpdates": false,
        "preferenceDeleteOrphanedDependencies": false,
        "preferenceDeleteSavedVariables": false,
        "preferenceReleaseType": 1,
        "preferenceModdingFolderPath": null,
        "syncProfile": {
            "PreferenceEnabled": false,
            "PreferenceAutoSync": true,
            "PreferenceAutoDelete": false,
            "PreferenceBackupSavedVariables": false,
            "GameInstanceGuid": "00000000-0000-0000-0000-000000000000",
            "SyncProfileID": 0,
            "SavedVariablesProfile": null,
            "LastSyncDate": NEVER
        },
        "installDate": crate::util::time::iso(chrono::Utc::now()),
        "installedAddons": [],
        "installedGamePrerequisites": [],
        "wasNameManuallyChanged": false,
        "wasGameVersionTypeIdManuallyChanged": false,
    });
    if let (Value::Object(all), Value::Object(rest)) = (&mut instance, rest) {
        all.extend(rest);
    }
    let file = dir.join("minecraftinstance.json");
    let tmp = dir.join("minecraftinstance.json.atlas");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&instance)?)?;
    std::fs::rename(&tmp, &file)?;
    Ok(folder)
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
        assert_eq!(
            atm.game_dir,
            root.join("Instances/All the Mods 10 To the Sky")
        );
        assert_eq!(atm.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(atm.loader, Some(Loader::Neoforge));
        assert_eq!(atm.loader_version.as_deref(), Some("21.1.250"));
        assert_eq!(
            atm.last_played_at.as_deref(),
            Some("2026-09-22T01:15:38.895Z")
        );
        assert_eq!(
            atm.icon.as_deref(),
            Some(
                "https://media.forgecdn.net/avatars/thumbnails/1389/739/256/256/63890103359327238"
            )
        );

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
        assert_eq!(
            parse_loader("fabric-0.16.5", Some("1.21.1")),
            Some((Loader::Fabric, Some("0.16.5".into())))
        );
        assert_eq!(
            parse_loader("fabric-0.16.5-1.21.1", Some("1.21.1")),
            Some((Loader::Fabric, Some("0.16.5".into())))
        );
        assert_eq!(
            parse_loader("quilt-0.26.0", None),
            Some((Loader::Quilt, Some("0.26.0".into())))
        );
        assert_eq!(parse_loader("weird-1.0", None), None);
    }

    /// Makes "Atlas test" (vanilla 1.21.1) in this PC's CurseForge, to check CurseForge
    /// takes it up. Opt-in, and close CurseForge first:
    /// `cargo test --lib real_curseforge_instance -- --ignored`
    #[test]
    #[ignore]
    fn real_curseforge_instance() {
        let root = default_roots().into_iter().find(|r| is_root(r)).unwrap();
        assert!(!running(), "close CurseForge first");
        let new = NewInstance {
            name: "Atlas test".into(),
            minecraft: "1.21.1".into(),
            loader: Loader::Vanilla,
            loader_version: None,
            icon: None,
        };
        let folder = create(&root, &new, None).unwrap();
        println!("created {}", root.join("Instances").join(folder).display());
    }

    #[test]
    fn created_instances_read_back() {
        let root = std::env::temp_dir().join(format!("atlas-cf-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(root.join("Install")).unwrap();
        let png = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prism/icons/smp.png"),
        )
        .unwrap();
        let neoforge = NewInstance {
            name: "Friday: SMP".into(),
            minecraft: "1.21.1".into(),
            loader: Loader::Neoforge,
            loader_version: Some("21.1.250".into()),
            icon: Some(png.clone()),
        };
        // What the server passes on from CurseForge, with fields it doesn't keep.
        let data = json!({
            "id": 12345, "gameVersionId": 7, "approved": true,
            "forgeVersion": "21.1.250", "name": "neoforge-21.1.250", "type": 6,
            "downloadUrl": "", "filename": "neoforge-21.1.250.jar", "installMethod": 6,
            "latest": false, "recommended": false, "versionJson": "{\"id\":\"neoforge-21.1.250\"}",
            "librariesInstallLocation": "{0}//libraries//net//neoforged//neoforge//21.1.250",
            "minecraftVersion": "1.21.1", "installProfileJson": "{\"spec\":1}"
        });
        let a = create(&root, &neoforge, Some(data.clone())).unwrap();
        let b = create(
            &root,
            &NewInstance {
                loader: Loader::Vanilla,
                loader_version: None,
                icon: None,
                ..neoforge.clone()
            },
            None,
        )
        .unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("Friday_ SMP", "Friday_ SMP (2)"));
        assert!(
            create(&root, &neoforge, None).is_err(),
            "modded needs the data"
        );

        let found = list(&root).unwrap();
        let first = found.iter().find(|f| f.launcher_instance_id == a).unwrap();
        assert_eq!(first.name, "Friday: SMP");
        assert_eq!(first.game_dir, root.join("Instances").join(&a));
        assert_eq!(first.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(first.loader, Some(Loader::Neoforge));
        assert_eq!(first.loader_version.as_deref(), Some("21.1.250"));
        assert_eq!(first.last_played_at, None);
        let icon = first
            .icon
            .as_deref()
            .unwrap()
            .strip_prefix("file:")
            .unwrap();
        assert_eq!(std::fs::read(icon).unwrap(), png);
        let second = found.iter().find(|f| f.launcher_instance_id == b).unwrap();
        assert_eq!(second.loader, Some(Loader::Vanilla));

        // Only the fields CurseForge keeps itself, as it writes them.
        let written: Value = serde_json::from_str(
            &std::fs::read_to_string(
                root.join("Instances")
                    .join(&a)
                    .join("minecraftinstance.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let base = written["baseModLoader"].as_object().unwrap();
        assert_eq!(base.len(), LOADER_FIELDS.len());
        assert!(base.get("id").is_none());
        assert_eq!(base["installMethod"], 6);
        assert_eq!(written["isVanilla"], false);
        assert!(written["installPath"].as_str().unwrap().ends_with('\\'));
        std::fs::remove_dir_all(&root).ok();
    }
}
