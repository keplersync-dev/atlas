//! Prism Launcher and MultiMC (same instance format; MultiMC is portable only).
//!
//! Not installed on the PC this was written on: the format comes from Prism's docs and
//! source. Verify against a real install and replace the fixtures with its files.
//!
//! <root>/prismlauncher.cfg (or multimc.cfg)  InstanceDir=instances
//! <root>/<InstanceDir>/<folder>/instance.cfg  name, iconKey, lastLaunchTime (ms), totalTimePlayed (s)
//! <root>/<InstanceDir>/<folder>/mmc-pack.json components[]: uid + version
//! game files in <folder>/minecraft/ (newer) or <folder>/.minecraft/
//! custom icons: <root>/icons/<iconKey>.<png|jpg|…> (built-in keys like "grass" have no file)

use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::util::{env_dir, first_existing, free_folder, from_unix_millis, image_ext, parse_ini};
use super::{FoundInstance, NewInstance, ReadResult};
use crate::dto::Loader;

#[derive(Debug, Clone, Copy)]
pub enum Flavor {
    Prism,
    MultiMc,
}

impl Flavor {
    fn config_file(self) -> &'static str {
        match self {
            Flavor::Prism => "prismlauncher.cfg",
            Flavor::MultiMc => "multimc.cfg",
        }
    }
    fn exe(self) -> &'static str {
        match self {
            Flavor::Prism => "prismlauncher.exe",
            Flavor::MultiMc => "MultiMC.exe",
        }
    }
}

pub fn default_roots(flavor: Flavor) -> Vec<PathBuf> {
    match flavor {
        Flavor::Prism => env_dir("APPDATA")
            .map(|d| d.join("PrismLauncher"))
            .into_iter()
            .collect(),
        // MultiMC has no default location; users add it with its folder.
        Flavor::MultiMc => vec![],
    }
}

pub fn is_root(flavor: Flavor, path: &Path) -> bool {
    path.join(flavor.config_file()).is_file()
}

/// The program to start: next to the config for portable installs, else the default install.
pub fn program(flavor: Flavor, root: &Path) -> Option<PathBuf> {
    let installed = env_dir("LOCALAPPDATA")
        .map(|d| d.join("Programs").join("PrismLauncher").join(flavor.exe()));
    first_existing(
        [Some(root.join(flavor.exe())), installed]
            .into_iter()
            .flatten(),
    )
}

/// Have Prism install a Modrinth pack as a new instance (its `--import`).
pub fn import(root: &Path, pack: &Path) -> std::io::Result<()> {
    let program = program(Flavor::Prism, root).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "prismlauncher.exe not found")
    })?;
    std::process::Command::new(program)
        .arg("--import")
        .arg(pack)
        .spawn()?;
    Ok(())
}

fn instances_dir(flavor: Flavor, root: &Path) -> PathBuf {
    let cfg = std::fs::read_to_string(root.join(flavor.config_file())).unwrap_or_default();
    let dir = parse_ini(&cfg)
        .remove("InstanceDir")
        .unwrap_or_else(|| "instances".into());
    root.join(dir) // join keeps `dir` as is when it's already absolute
}

pub fn list(flavor: Flavor, root: &Path) -> ReadResult<Vec<FoundInstance>> {
    let dir = instances_dir(flavor, root);
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        let Some(folder) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        // Prism's temp folders (_MMC_TEMP, .LAUNCHER_TEMP) aren't instances.
        if folder.starts_with(['.', '_']) || !path.join("instance.cfg").is_file() {
            continue;
        }
        match read_instance(&path, &folder, &root.join("icons")) {
            Ok(i) => found.push(i),
            Err(e) => {
                tracing::warn!(instance = %folder, error = %e, "skipping unreadable Prism instance")
            }
        }
    }
    Ok(found)
}

fn read_instance(dir: &Path, folder: &str, icons: &Path) -> ReadResult<FoundInstance> {
    let cfg = parse_ini(&std::fs::read_to_string(dir.join("instance.cfg"))?);
    let pack = read_pack(dir).unwrap_or_default();
    let game_dir = first_existing([dir.join("minecraft"), dir.join(".minecraft")])
        .unwrap_or_else(|| dir.join("minecraft"));
    Ok(FoundInstance {
        launcher_instance_id: folder.to_string(),
        name: cfg
            .get("name")
            .filter(|n| !n.is_empty())
            .cloned()
            .unwrap_or_else(|| folder.to_string()),
        game_dir,
        icon: cfg.get("iconKey").and_then(|key| custom_icon(icons, key)),
        minecraft_version: pack.minecraft,
        loader: pack
            .loader
            .as_ref()
            .map(|(l, _)| *l)
            .or(Some(Loader::Vanilla)),
        loader_version: pack.loader.and_then(|(_, v)| v),
        last_played_at: cfg
            .get("lastLaunchTime")
            .and_then(|v| v.parse().ok())
            .and_then(from_unix_millis),
        total_playtime_seconds: cfg.get("totalTimePlayed").and_then(|v| v.parse().ok()),
    })
}

/// A custom icon the user picked in Prism (built-in ones like "grass" have no file).
fn custom_icon(icons: &Path, key: &str) -> Option<String> {
    if key.is_empty() || key.contains(['/', '\\', '.']) {
        return None;
    }
    ["png", "jpg", "jpeg", "gif", "webp"]
        .iter()
        .map(|ext| icons.join(format!("{key}.{ext}")))
        .find(|p| p.is_file())
        .map(|p| format!("file:{}", p.display()))
}

#[derive(Default)]
struct Pack {
    minecraft: Option<String>,
    loader: Option<(Loader, Option<String>)>,
}

fn read_pack(dir: &Path) -> ReadResult<Pack> {
    #[derive(Deserialize)]
    struct MmcPack {
        components: Vec<Component>,
    }
    #[derive(Deserialize)]
    struct Component {
        uid: String,
        version: Option<String>,
    }
    let pack: MmcPack = serde_json::from_str(&std::fs::read_to_string(dir.join("mmc-pack.json"))?)?;
    let mut out = Pack::default();
    for c in pack.components {
        let loader = match c.uid.as_str() {
            "net.minecraft" => {
                out.minecraft = c.version;
                continue;
            }
            "net.fabricmc.fabric-loader" => Loader::Fabric,
            "org.quiltmc.quilt-loader" => Loader::Quilt,
            "net.minecraftforge" => Loader::Forge,
            "net.neoforged" => Loader::Neoforge,
            _ => continue,
        };
        out.loader = Some((loader, c.version));
    }
    Ok(out)
}

/// The Modrinth modpack Prism installed the instance from (`ManagedPack…` in
/// instance.cfg, next to the game folder). CurseForge ones ("flame") aren't read: Prism
/// doesn't keep their download.
pub fn installed_pack(game_dir: &Path) -> Option<super::InstalledPack> {
    let cfg = parse_ini(&std::fs::read_to_string(game_dir.parent()?.join("instance.cfg")).ok()?);
    let get = |key: &str| cfg.get(key).filter(|v| !v.is_empty()).cloned();
    if get("ManagedPack").as_deref() != Some("true")
        || get("ManagedPackType").as_deref() != Some("modrinth")
    {
        return None;
    }
    Some(super::InstalledPack::Modrinth {
        project_id: get("ManagedPackID")?,
        version_id: get("ManagedPackVersionID")?,
    })
}

/// Create an empty instance for a shared one: `instance.cfg`, `mmc-pack.json` with the
/// Minecraft and loader versions, the game folder, and the picture in Prism's icons
/// folder. Prism downloads the game itself the first time it's played. Returns the
/// instance's folder name.
pub fn create(flavor: Flavor, root: &Path, new: &NewInstance) -> ReadResult<String> {
    let loader_version = new.loader_version()?;
    let dir = instances_dir(flavor, root);
    std::fs::create_dir_all(&dir)?;
    let folder = free_folder(&dir, &new.name);

    let mut components = vec![serde_json::json!({
        "uid": "net.minecraft", "version": new.minecraft, "important": true
    })];
    let loader_uid = match new.loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("net.fabricmc.fabric-loader"),
        Loader::Quilt => Some("org.quiltmc.quilt-loader"),
        Loader::Forge => Some("net.minecraftforge"),
        Loader::Neoforge => Some("net.neoforged"),
    };
    if let (Some(uid), Some(version)) = (loader_uid, loader_version) {
        if matches!(new.loader, Loader::Fabric | Loader::Quilt) {
            components.push(serde_json::json!({
                "uid": "net.fabricmc.intermediary", "version": new.minecraft, "dependencyOnly": true
            }));
        }
        components.push(serde_json::json!({ "uid": uid, "version": version }));
    }
    let pack = serde_json::json!({ "formatVersion": 1, "components": components });
    let icon_key = match &new.icon {
        Some(bytes) => save_icon(&root.join("icons"), bytes)?,
        None => None,
    };

    let instance = dir.join(&folder);
    std::fs::create_dir_all(instance.join("minecraft"))?;
    std::fs::write(
        instance.join("instance.cfg"),
        format!(
            "[General]\nConfigVersion=1.2\nInstanceType=OneSix\niconKey={}\nname={}\n",
            icon_key.as_deref().unwrap_or("default"),
            new.name.replace(['\r', '\n'], " ")
        ),
    )?;
    std::fs::write(
        instance.join("mmc-pack.json"),
        serde_json::to_vec_pretty(&pack)?,
    )?;
    Ok(folder)
}

/// Put a picture in Prism's icons folder, named after its contents so the same picture
/// is saved once. Returns its icon key (the file name without extension).
fn save_icon(icons: &Path, bytes: &[u8]) -> ReadResult<Option<String>> {
    let Some(ext) = image_ext(bytes) else {
        return Ok(None);
    };
    let key = format!("atlas-{}", &hex::encode(Sha256::digest(bytes))[..16]);
    std::fs::create_dir_all(icons)?;
    let file = icons.join(format!("{key}.{ext}"));
    if !file.is_file() {
        std::fs::write(file, bytes)?;
    }
    Ok(Some(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prism")
    }

    fn new_instance(loader: Loader, version: Option<&str>) -> NewInstance {
        NewInstance {
            name: "Friday: SMP".into(),
            minecraft: "1.21.1".into(),
            loader,
            loader_version: version.map(str::to_owned),
            icon: None,
        }
    }

    #[test]
    fn created_instances_read_back() {
        let tmp = std::env::temp_dir().join(format!("atlas-prism-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("prismlauncher.cfg"),
            "InstanceDir=instances
",
        )
        .unwrap();
        let fabric = new_instance(Loader::Fabric, Some("0.16.5"));
        let a = create(Flavor::Prism, &tmp, &fabric).unwrap();
        let png = std::fs::read(fixture().join("icons/smp.png")).unwrap();
        let b = create(
            Flavor::Prism,
            &tmp,
            &NewInstance {
                icon: Some(png.clone()),
                ..fabric.clone()
            },
        )
        .unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("Friday_ SMP", "Friday_ SMP (2)"));
        let found = list(Flavor::Prism, &tmp).unwrap();
        let first = found.iter().find(|f| f.launcher_instance_id == a).unwrap();
        assert_eq!(first.name, "Friday: SMP");
        assert_eq!(first.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(first.loader, Some(Loader::Fabric));
        assert_eq!(first.loader_version.as_deref(), Some("0.16.5"));
        assert!(first.game_dir.ends_with("minecraft"));
        assert_eq!(first.icon, None, "built-in default icon");

        let second = found.iter().find(|f| f.launcher_instance_id == b).unwrap();
        let icon = second
            .icon
            .as_deref()
            .unwrap()
            .strip_prefix("file:")
            .unwrap();
        assert!(icon.ends_with(".png"), "{icon}");
        assert_eq!(std::fs::read(icon).unwrap(), png);

        let neo = create(
            Flavor::Prism,
            &tmp,
            &new_instance(Loader::Neoforge, Some("21.1.77")),
        )
        .unwrap();
        let neo = list(Flavor::Prism, &tmp)
            .unwrap()
            .into_iter()
            .find(|f| f.launcher_instance_id == neo)
            .unwrap();
        assert_eq!(neo.loader, Some(Loader::Neoforge));
        assert_eq!(neo.loader_version.as_deref(), Some("21.1.77"));
        assert!(create(Flavor::Prism, &tmp, &new_instance(Loader::Forge, None)).is_err());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn reads_instances_and_skips_temp_folders() {
        let root = fixture();
        assert!(is_root(Flavor::Prism, &root));
        assert!(!is_root(Flavor::MultiMc, &root));

        let mut list = list(Flavor::Prism, &root).unwrap();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(list.len(), 2, "{list:#?}");

        let smp = &list[1];
        assert_eq!(smp.name, "Survival SMP");
        assert_eq!(smp.launcher_instance_id, "Survival SMP");
        assert_eq!(smp.game_dir, root.join("instances/Survival SMP/minecraft"));
        assert_eq!(smp.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(smp.loader, Some(Loader::Fabric));
        assert_eq!(smp.loader_version.as_deref(), Some("0.16.5"));
        assert_eq!(
            smp.last_played_at.as_deref(),
            Some("2026-10-19T00:00:00.000Z")
        );
        assert_eq!(smp.total_playtime_seconds, Some(36_000));
        assert_eq!(
            smp.icon,
            Some(format!(
                "file:{}",
                root.join("icons").join("smp.png").display()
            )),
            "custom icon file"
        );

        // older instance: .minecraft folder, vanilla, no play data
        let old = &list[0];
        assert_eq!(old.name, "Old Vanilla");
        assert_eq!(old.game_dir, root.join("instances/oldvanilla/.minecraft"));
        assert_eq!(old.loader, Some(Loader::Vanilla));
        assert_eq!(old.minecraft_version.as_deref(), Some("1.8.9"));
        assert_eq!(old.last_played_at, None);
        assert_eq!(old.icon, None, "built-in key, no file");
    }
}
