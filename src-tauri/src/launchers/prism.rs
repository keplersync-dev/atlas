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

use super::util::{env_dir, first_existing, from_unix_millis, parse_ini};
use super::{FoundInstance, ReadError, ReadResult};
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

/// Create an empty instance for a shared one: `instance.cfg`, `mmc-pack.json` with the
/// Minecraft and loader versions, and the game folder. Prism downloads the game itself the
/// first time it's played. Returns the instance's folder name.
pub fn create(
    flavor: Flavor,
    root: &Path,
    name: &str,
    minecraft: &str,
    loader: Loader,
    loader_version: Option<&str>,
) -> ReadResult<String> {
    let dir = instances_dir(flavor, root);
    std::fs::create_dir_all(&dir)?;
    let base: String = name
        .chars()
        .map(|c| {
            if r#"<>:"/\|?*"#.contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .to_owned();
    let base = if base.is_empty() {
        "Atlas".to_owned()
    } else {
        base
    };
    let folder = (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base} ({n})")
            }
        })
        .find(|f| !dir.join(f).exists())
        .expect("some folder name is free");

    let mut components = vec![serde_json::json!({
        "uid": "net.minecraft", "version": minecraft, "important": true
    })];
    let loader_uid = match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("net.fabricmc.fabric-loader"),
        Loader::Quilt => Some("org.quiltmc.quilt-loader"),
        Loader::Forge => Some("net.minecraftforge"),
        Loader::Neoforge => Some("net.neoforged"),
    };
    if let Some(uid) = loader_uid {
        let version = loader_version.ok_or_else(|| {
            ReadError::Format("the shared instance doesn't say which loader version".into())
        })?;
        if matches!(loader, Loader::Fabric | Loader::Quilt) {
            components.push(serde_json::json!({
                "uid": "net.fabricmc.intermediary", "version": minecraft, "dependencyOnly": true
            }));
        }
        components.push(serde_json::json!({ "uid": uid, "version": version }));
    }
    let pack = serde_json::json!({ "formatVersion": 1, "components": components });

    let instance = dir.join(&folder);
    std::fs::create_dir_all(instance.join("minecraft"))?;
    std::fs::write(
        instance.join("instance.cfg"),
        format!(
            "[General]\nConfigVersion=1.2\nInstanceType=OneSix\niconKey=default\nname={name}\n"
        ),
    )?;
    std::fs::write(
        instance.join("mmc-pack.json"),
        serde_json::to_vec_pretty(&pack)?,
    )?;
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prism")
    }

    #[test]
    fn created_instances_read_back() {
        let tmp = std::env::temp_dir().join(format!("atlas-prism-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("prismlauncher.cfg"), "InstanceDir=instances\n").unwrap();
        let a = create(
            Flavor::Prism,
            &tmp,
            "Friday: SMP",
            "1.21.1",
            Loader::Fabric,
            Some("0.16.5"),
        )
        .unwrap();
        let b = create(
            Flavor::Prism,
            &tmp,
            "Friday: SMP",
            "1.21.1",
            Loader::Fabric,
            Some("0.16.5"),
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
        assert!(create(Flavor::Prism, &tmp, "x", "1.21.1", Loader::Forge, None).is_err());
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
