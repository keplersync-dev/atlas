//! Modrinth packs (.mrpack).
//!
//! Export: an instance as a pack anyone can open in Prism or the Modrinth App, with files
//! from Modrinth as download links and everything else inside it (CurseForge files,
//! uploads, config files). Files set to Only me and config folders switched off stay out,
//! as when sharing.
//!
//! Import: hand a pack to a launcher that installs packs (Prism, the Modrinth App). Its new
//! instance then shows up to track.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha512};
use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;
use url::Url;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::db;
use crate::dto::{LauncherKind, Loader, Side, from_wire};
use crate::error::{AppError, AppResult};
use crate::launchers;
use crate::state::AppState;

/// Where a pack may send people to download from, by Modrinth's rules. Anything else goes
/// inside the pack.
const ALLOWED_HOSTS: &[&str] = &[
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

struct PackFile {
    /// Relative to the game folder, with forward slashes.
    path: String,
    file: PathBuf,
    size: u64,
    sha1: String,
    download: Option<String>,
    side: Side,
    optional: bool,
}

struct Pack {
    name: String,
    version_id: String,
    minecraft: String,
    loader: Loader,
    loader_version: Option<String>,
    files: Vec<PackFile>,
    configs: Vec<(String, PathBuf)>,
}

/// Ask where to save the instance as a pack, and save it there. None if the user cancelled.
pub async fn export<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    id: &str,
) -> AppResult<Option<String>> {
    let row = db::instances::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))?;
    let (Some(minecraft), Some(loader)) = (row.minecraft_version.clone(), row.loader()) else {
        return Err(AppError::invalid_input(
            "Atlas couldn't tell which Minecraft version and loader this folder uses",
        ));
    };
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(format!("{}.mrpack", launchers::file_stem(&row.name)))
        .add_filter("Modrinth pack", &["mrpack"])
        .save_file(move |picked| {
            let _ = tx.send(picked);
        });
    let Some(picked) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    let to = picked
        .into_path()
        .map_err(|_| AppError::invalid_input("Choose a folder on this PC"))?;

    let game = PathBuf::from(&row.game_dir);
    let files = db::content::list(&state.db, id)
        .await?
        .into_iter()
        .filter(|r| r.installed && r.enabled && r.share_mode != "private")
        .map(|r| PackFile {
            file: game.join(&r.path),
            size: r.size_bytes.max(0) as u64,
            download: r.download_url.clone().filter(|u| downloadable(u)),
            side: from_wire(&r.side).unwrap_or(Side::Both),
            optional: r.share_mode == "optional",
            path: r.path,
            sha1: r.sha1,
        })
        .collect();
    let pack = Pack {
        version_id: match row.synced_revision {
            Some(n) if n > 0 => format!("v{n}"),
            _ => "1".to_owned(),
        },
        name: row.name.clone(),
        minecraft,
        loader,
        loader_version: row.loader_version.clone(),
        files,
        configs: super::publish::shared_config_files(state, &row).await?,
    };
    let target = to.clone();
    tokio::task::spawn_blocking(move || write(&target, &pack))
        .await
        .map_err(|e| AppError::unknown(e.to_string()))??;
    super::reveal(app, &to);
    Ok(Some(to.to_string_lossy().into_owned()))
}

fn downloadable(url: &str) -> bool {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| ALLOWED_HOSTS.contains(&h)))
        .unwrap_or(false)
}

/// Modrinth's `env`: whether the client and the server need the file.
fn env(side: Side, optional: bool) -> Value {
    let client = match side {
        Side::Server => "unsupported",
        _ if optional => "optional",
        _ => "required",
    };
    let server = if side == Side::Client {
        "unsupported"
    } else {
        "required"
    };
    json!({ "client": client, "server": server })
}

fn zip_error(e: zip::result::ZipError) -> AppError {
    tracing::warn!(error = %e, "couldn't write a pack");
    AppError::unknown("Couldn't write the pack")
}

/// Write the pack beside `to`, then move it into place, so a failure leaves no half pack.
fn write(to: &Path, pack: &Pack) -> AppResult<()> {
    let part = to.with_extension("mrpack.part");
    let result = write_to(&part, pack).and_then(|()| Ok(std::fs::rename(&part, to)?));
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

fn write_to(file: &Path, pack: &Pack) -> AppResult<()> {
    // Jars and zips are compressed already: store them as they are.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(file)?);
    let mut listed = Vec::new();
    for f in &pack.files {
        match &f.download {
            Some(url) => {
                let sha512 = hex::encode(Sha512::digest(std::fs::read(&f.file)?));
                listed.push(json!({
                    "path": f.path,
                    "hashes": { "sha1": f.sha1, "sha512": sha512 },
                    "env": env(f.side, f.optional),
                    "downloads": [url],
                    "fileSize": f.size,
                }));
            }
            None => {
                let archive = [".jar", ".zip"]
                    .iter()
                    .any(|ext| f.path.to_lowercase().ends_with(ext));
                zip.start_file(
                    format!("overrides/{}", f.path),
                    if archive { stored } else { deflated },
                )
                .map_err(zip_error)?;
                std::io::copy(&mut std::fs::File::open(&f.file)?, &mut zip)?;
            }
        }
    }
    for (path, file) in &pack.configs {
        zip.start_file(format!("overrides/{path}"), deflated)
            .map_err(zip_error)?;
        std::io::copy(&mut std::fs::File::open(file)?, &mut zip)?;
    }

    let mut dependencies = json!({ "minecraft": pack.minecraft });
    if let (Some(key), Some(version)) = (
        launchers::pack_dependency_key(pack.loader),
        &pack.loader_version,
    ) {
        dependencies[key] = version.clone().into();
    }
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": pack.version_id,
        "name": pack.name,
        "summary": "Exported from Atlas",
        "files": listed,
        "dependencies": dependencies,
    });
    zip.start_file("modrinth.index.json", deflated)
        .map_err(zip_error)?;
    zip.write_all(&serde_json::to_vec_pretty(&index)?)?;
    zip.finish().map_err(zip_error)?;
    Ok(())
}

/// Ask for a pack and give it to this launcher to install. False if the user cancelled.
pub async fn import<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    launcher_id: &str,
) -> AppResult<bool> {
    let launcher = db::launchers::list(&state.db)
        .await?
        .into_iter()
        .find(|l| l.id == launcher_id)
        .ok_or_else(|| AppError::not_found("Launcher not found"))?;
    let kind = launcher.kind();
    if !matches!(kind, Some(LauncherKind::Prism | LauncherKind::Modrinth)) {
        return Err(AppError::invalid_input(
            "Only Prism and the Modrinth App install Modrinth packs",
        ));
    }
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Modrinth pack", &["mrpack"])
        .pick_file(move |picked| {
            let _ = tx.send(picked);
        });
    let Some(picked) = rx.await.ok().flatten() else {
        return Ok(false);
    };
    let pack = picked
        .into_path()
        .map_err(|_| AppError::invalid_input("Choose a file on this PC"))?;
    let kind = kind.expect("checked above");
    launchers::import_pack(kind, Path::new(&launcher.root_path), &pack).map_err(|e| {
        tracing::warn!(error = %e, "couldn't hand the pack to the launcher");
        AppError::unknown("Couldn't open the launcher")
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn packs_link_modrinth_files_and_carry_the_rest() {
        let dir = std::env::temp_dir().join(format!("atlas-mrpack-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(dir.join("mods/sodium.jar"), b"sodium").unwrap();
        std::fs::write(dir.join("mods/custom.jar"), b"custom").unwrap();
        std::fs::write(dir.join("config/a.json"), b"{}").unwrap();
        let file = |path: &str, download: Option<&str>, side| PackFile {
            path: path.into(),
            file: dir.join(path),
            size: 6,
            sha1: "sha1".into(),
            download: download.map(str::to_owned),
            side,
            optional: false,
        };
        let pack = Pack {
            name: "Friday SMP".into(),
            version_id: "v3".into(),
            minecraft: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.5".into()),
            files: vec![
                file(
                    "mods/sodium.jar",
                    Some("https://cdn.modrinth.com/data/x/sodium.jar"),
                    Side::Client,
                ),
                file("mods/custom.jar", None, Side::Both),
            ],
            configs: vec![("config/a.json".into(), dir.join("config/a.json"))],
        };
        let out = dir.join("Friday SMP.mrpack");
        write(&out, &pack).unwrap();

        let mut zip = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let mut index = String::new();
        zip.by_name("modrinth.index.json")
            .unwrap()
            .read_to_string(&mut index)
            .unwrap();
        let index: Value = serde_json::from_str(&index).unwrap();
        assert_eq!(index["dependencies"]["fabric-loader"], "0.16.5");
        assert_eq!(
            index["files"].as_array().unwrap().len(),
            1,
            "only the Modrinth one"
        );
        assert_eq!(index["files"][0]["env"]["server"], "unsupported");
        assert_eq!(
            index["files"][0]["hashes"]["sha512"],
            hex::encode(Sha512::digest(b"sodium"))
        );
        assert!(zip.by_name("overrides/mods/custom.jar").is_ok());
        assert!(zip.by_name("overrides/config/a.json").is_ok());
        assert!(!dir.join("Friday SMP.mrpack.part").exists());
        assert!(!downloadable("https://edge.forgecdn.net/files/1/2/a.jar"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
