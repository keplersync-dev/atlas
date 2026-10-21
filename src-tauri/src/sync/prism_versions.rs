//! When a new version needs a different Minecraft or loader version: Prism/MultiMC
//! instances are updated by editing `mmc-pack.json` (tauri-plan §12.3 step 7). Other
//! launchers can't be edited safely; the review screen tells the user what to change.

use std::path::Path;

use serde_json::Value;

use super::manifest::Manifest;
use crate::db::instances::InstanceRow;
use crate::dto::{LauncherChange, LauncherKind, Loader, VersionChange};

/// Can Atlas change this instance's versions itself?
pub fn automatic(kind: LauncherKind) -> bool {
    matches!(kind, LauncherKind::Prism | LauncherKind::Multimc)
}

/// What the review screen shows when versions differ (None when they match or are unknown).
pub fn change(row: &InstanceRow, m: &Manifest) -> Option<LauncherChange> {
    let differs = |now: &Option<String>, want: &Option<String>| match (now, want) {
        (Some(a), Some(b)) if a != b => Some(VersionChange { from: a.clone(), to: b.clone() }),
        _ => None,
    };
    let minecraft_version = differs(&row.minecraft_version, &Some(m.minecraft.version.clone()));
    let loader_version = differs(&row.loader_version, &m.minecraft.loader_version);
    (minecraft_version.is_some() || loader_version.is_some()).then(|| LauncherChange {
        minecraft_version,
        loader_version,
        automatic: automatic(row.kind()),
    })
}

fn loader_uid(l: Loader) -> Option<&'static str> {
    match l {
        Loader::Fabric => Some("net.fabricmc.fabric-loader"),
        Loader::Quilt => Some("org.quiltmc.quilt-loader"),
        Loader::Forge => Some("net.minecraftforge"),
        Loader::Neoforge => Some("net.neoforged"),
        Loader::Vanilla => None,
    }
}

/// Edit Prism's mmc-pack.json if the versions differ. Logs instead of failing: the files
/// are already in place, and the user can still change versions in Prism.
pub fn update_if_needed(row: &InstanceRow, m: &Manifest) {
    if !automatic(row.kind()) || change(row, m).is_none() {
        return;
    }
    let Some(instance_dir) = Path::new(&row.game_dir).parent() else { return };
    if let Err(e) = edit_pack(&instance_dir.join("mmc-pack.json"), m) {
        tracing::warn!(error = %e, "couldn't update Prism's versions");
    }
}

fn edit_pack(file: &Path, m: &Manifest) -> Result<(), Box<dyn std::error::Error>> {
    let mut pack: Value = serde_json::from_str(&std::fs::read_to_string(file)?)?;
    let components = pack["components"].as_array_mut().ok_or("no components")?;
    let loader = loader_uid(m.minecraft.loader);
    for c in components.iter_mut() {
        let uid = c["uid"].as_str().unwrap_or_default().to_string();
        if uid == "net.minecraft" {
            c["version"] = Value::String(m.minecraft.version.clone());
        } else if Some(uid.as_str()) == loader
            && let Some(v) = &m.minecraft.loader_version
        {
            c["version"] = Value::String(v.clone());
        }
        // Prism recomputes cached dependency versions itself.
        if let Some(obj) = c.as_object_mut() {
            obj.remove("cachedVersion");
        }
    }
    let tmp = file.with_extension("json.atlas-tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&pack)?)?;
    std::fs::rename(tmp, file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_minecraft_and_loader_versions_only() {
        let dir = std::env::temp_dir().join(format!("atlas-prism-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("mmc-pack.json");
        std::fs::write(
            &file,
            r#"{"components":[{"uid":"org.lwjgl3","version":"3.3.3"},{"uid":"net.minecraft","version":"1.21"},{"uid":"net.fabricmc.fabric-loader","version":"0.15.0"}],"formatVersion":1}"#,
        )
        .unwrap();
        let mut m: Manifest = serde_json::from_slice(&std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/schema/manifest.example.json")).unwrap()).unwrap();
        m.minecraft.version = "1.21.1".into();
        m.minecraft.loader_version = Some("0.16.5".into());
        edit_pack(&file, &m).unwrap();
        let pack: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        let v = |uid: &str| pack["components"].as_array().unwrap().iter().find(|c| c["uid"] == uid).unwrap()["version"].clone();
        assert_eq!(v("net.minecraft"), "1.21.1");
        assert_eq!(v("net.fabricmc.fabric-loader"), "0.16.5");
        assert_eq!(v("org.lwjgl3"), "3.3.3");
        std::fs::remove_dir_all(dir).ok();
    }
}
