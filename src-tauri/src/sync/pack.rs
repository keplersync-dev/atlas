//! A shared instance's modpack (`Manifest::base`): the CurseForge .zip or Modrinth
//! .mrpack it was installed from. Its own files (configs, scripts, textures under the
//! pack's `overrides/` folder) come out of that download instead of being uploaded.
//!
//! The pack is downloaded once into the cache (by its sha1, like any file), and what its
//! overrides hold is listed next to it: cache/packs/<sha1>.json.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::fetch;
use super::manifest::{Override, PackBase};
use crate::db::instances::InstanceRow;
use crate::dto::ContentSource;
use crate::error::{AppError, AppResult};
use crate::launchers::{self, InstalledPack};
use crate::state::{AppPaths, AppState};
use crate::util::ids::new_id;

/// One of the pack's own files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackFile {
    pub sha256: String,
    pub size_bytes: u64,
    /// Its name inside the pack: "overrides/config/a.json".
    entry: String,
}

/// The pack's own files by where they go in the instance ("config/a.json").
pub type PackIndex = HashMap<String, PackFile>;

/// The modpack to share the instance on: the one its launcher installed it from, else
/// (a member's copy, made by Atlas) the one the shared instance already names.
pub async fn base_for(state: &AppState, row: &InstanceRow) -> Option<PackBase> {
    if let (Some(launcher_id), Some(instance)) = (&row.launcher_id, &row.launcher_instance_id)
        && let Ok(Some(launcher)) = crate::db::launchers::get(&state.db, launcher_id).await
    {
        let found = launchers::installed_pack(
            row.kind(),
            Path::new(&launcher.root_path),
            instance,
            Path::new(&row.game_dir),
        )
        .await;
        match found {
            Some(InstalledPack::Known(base)) => return Some(base),
            Some(InstalledPack::Modrinth {
                project_id,
                version_id,
            }) => match modrinth_base(state, &project_id, &version_id).await {
                Ok(base) => return base,
                Err(e) => tracing::info!(error = %e, "couldn't look up the Modrinth pack"),
            },
            None => {}
        }
    }
    row.synced_manifest().and_then(|m| m.base)
}

async fn modrinth_base(
    state: &AppState,
    project_id: &str,
    version_id: &str,
) -> reqwest::Result<Option<PackBase>> {
    let Some(api) = state.modrinth_api.as_deref() else {
        return Ok(None);
    };
    use crate::resolve::modrinth;
    let version = modrinth::version(&state.http, api, version_id).await?;
    let Some(file) = version.files.iter().find(|f| f.primary).or_else(|| {
        version
            .files
            .iter()
            .find(|f| f.filename.ends_with(".mrpack"))
    }) else {
        return Ok(None);
    };
    let projects = modrinth::projects(&state.http, api, &[project_id.to_owned()]).await?;
    let Some(project) = projects.get(project_id) else {
        return Ok(None);
    };
    Ok(Some(PackBase {
        source: ContentSource::Modrinth,
        project_id: project_id.to_owned(),
        version_id: version_id.to_owned(),
        name: project.title.clone(),
        version_label: Some(version.version_number.clone()),
        download_url: file.url.clone(),
        sha1: file.hashes.sha1.clone(),
        size_bytes: file.size,
    }))
}

fn pack_file(paths: &AppPaths, base: &PackBase) -> PathBuf {
    fetch::cache_file(paths, &base.sha1)
}

fn index_file(paths: &AppPaths, base: &PackBase) -> PathBuf {
    paths
        .cache
        .join("packs")
        .join(format!("{}.json", base.sha1))
}

/// Whether the pack still has to be downloaded (for the size an update shows).
pub fn needs_download(paths: &AppPaths, base: &PackBase) -> bool {
    !pack_file(paths, base).is_file()
}

/// Download the pack into the cache, if it isn't there. `on_bytes` gets its progress.
async fn download(
    state: &AppState,
    base: &PackBase,
    on_bytes: &mut (dyn FnMut(u64) + Send),
) -> AppResult<PathBuf> {
    let file = pack_file(&state.paths, base);
    if !file.is_file() {
        if !super::manifest::allowed_download(&base.download_url) {
            return Err(AppError::invalid_input(format!(
                "{} downloads from a site Atlas doesn't allow.",
                base.name
            )));
        }
        fetch::download(
            &state.downloads,
            &base.download_url,
            &file,
            Some(&base.sha1),
            None,
            on_bytes,
        )
        .await?;
    }
    Ok(file)
}

/// What the pack's overrides hold, downloading the pack first if needed.
pub async fn index(
    state: &AppState,
    base: &PackBase,
    on_bytes: &mut (dyn FnMut(u64) + Send),
) -> AppResult<PackIndex> {
    let saved = index_file(&state.paths, base);
    if let Some(index) = std::fs::read(&saved)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        return Ok(index);
    }
    let pack = download(state, base, on_bytes).await?;
    let source = base.source;
    let index = tokio::task::spawn_blocking(move || read_index(&pack, source)).await??;
    std::fs::create_dir_all(saved.parent().expect("has a parent"))?;
    std::fs::write(&saved, serde_json::to_vec(&index)?)?;
    Ok(index)
}

/// Where a pack keeps its own files: CurseForge names the folder in its manifest.json
/// ("overrides"); a Modrinth pack has overrides/ and, winning over it, client-overrides/.
fn read_index(pack: &Path, source: ContentSource) -> AppResult<PackIndex> {
    let mut zip = open(pack)?;
    let folders: Vec<String> = match source {
        ContentSource::Curseforge => {
            let folder = read_entry(&mut zip, "manifest.json")
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .and_then(|m| m["overrides"].as_str().map(str::to_owned))
                .unwrap_or_else(|| "overrides".to_owned());
            vec![folder]
        }
        _ => vec!["overrides".to_owned(), "client-overrides".to_owned()],
    };
    let mut index = PackIndex::new();
    for folder in folders {
        let prefix = format!("{}/", folder.trim_end_matches('/'));
        for i in 0..zip.len() {
            let mut file = zip.by_index(i).map_err(damaged)?;
            if !file.is_file() {
                continue;
            }
            let entry = file.name().map_err(damaged)?.into_owned();
            let Some(rel) = entry.strip_prefix(&prefix) else {
                continue;
            };
            if !super::manifest::can_come_from_pack(rel) {
                continue;
            }
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            index.insert(
                rel.to_owned(),
                PackFile {
                    sha256: hex::encode(Sha256::digest(&bytes)),
                    size_bytes: bytes.len() as u64,
                    entry: entry.clone(),
                },
            );
        }
    }
    Ok(index)
}

/// Put these files from the pack into the cache (as `fetch::blob_file`), downloading the
/// pack first if needed. Each is checked against its sha256.
pub async fn extract(
    state: &AppState,
    base: &PackBase,
    wanted: Vec<Override>,
    on_bytes: &mut (dyn FnMut(u64) + Send),
) -> AppResult<()> {
    let wanted: Vec<Override> = wanted
        .into_iter()
        .filter(|o| !fetch::blob_file(&state.paths, &o.sha256).is_file())
        .collect();
    if wanted.is_empty() {
        return Ok(());
    }
    let index = index(state, base, on_bytes).await?;
    let pack = download(state, base, on_bytes).await?;
    let paths = state.paths.clone();
    let name = base.name.clone();
    tokio::task::spawn_blocking(move || {
        let mut zip = open(&pack)?;
        for o in wanted {
            let missing = || {
                AppError::conflict(format!(
                    "{} isn't in {name} as the shared version expects. Ask the owner to publish again.",
                    o.path
                ))
            };
            let file = index.get(&o.path).filter(|f| f.sha256 == o.sha256);
            let file = file.ok_or_else(missing)?;
            let bytes = read_entry(&mut zip, &file.entry)?;
            if hex::encode(Sha256::digest(&bytes)) != o.sha256 {
                return Err(missing());
            }
            let target = fetch::blob_file(&paths, &o.sha256);
            std::fs::create_dir_all(target.parent().expect("has a parent"))?;
            let tmp = target.with_extension(format!("{}.part", new_id()));
            std::fs::File::create(&tmp)?.write_all(&bytes)?;
            std::fs::rename(&tmp, &target)?;
        }
        Ok(())
    })
    .await?
}

fn open(pack: &Path) -> AppResult<zip::ZipArchive<std::fs::File>> {
    zip::ZipArchive::new(std::fs::File::open(pack)?).map_err(damaged)
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> AppResult<Vec<u8>> {
    let mut file = zip.by_name(name).map_err(damaged)?;
    let mut bytes = Vec::new(); // not sized from the header: a damaged zip can claim anything
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn damaged(e: zip::result::ZipError) -> AppError {
    tracing::warn!(error = %e, "couldn't read a modpack");
    AppError::unknown("The modpack's download is damaged. Try again.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_zip(file: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(file).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn lists_a_curseforge_packs_own_files() {
        let dir = std::env::temp_dir().join(format!("atlas-pack-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pack = dir.join("ATM10SKY-2.0.4.zip");
        write_zip(
            &pack,
            &[
                ("manifest.json", br#"{"overrides":"overrides"}"#),
                ("overrides/config/a.toml", b"a = 1"),
                ("overrides/kubejs/assets/atm/logo.png", b"png"),
                ("overrides/mods/custom.jar", b"jar"),
                ("modlist.html", b"<ul></ul>"),
            ],
        );
        let index = read_index(&pack, ContentSource::Curseforge).unwrap();
        let mut keys: Vec<&str> = index.keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(
            keys,
            ["config/a.toml", "kubejs/assets/atm/logo.png"],
            "mods are content, not the pack's own files"
        );
        assert_eq!(
            index["config/a.toml"].sha256,
            hex::encode(Sha256::digest(b"a = 1"))
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Against a real CurseForge instance and its pack (downloaded):
    /// `ATLAS_CF_INSTANCE=<instance folder> cargo test real_curseforge_pack -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "downloads a modpack; needs ATLAS_CF_INSTANCE"]
    async fn real_curseforge_pack() {
        let dir = PathBuf::from(std::env::var("ATLAS_CF_INSTANCE").unwrap());
        let found = launchers::installed_pack(
            crate::dto::LauncherKind::Curseforge,
            dir.parent().unwrap(),
            "",
            &dir,
        )
        .await;
        let Some(InstalledPack::Known(base)) = found else {
            panic!("not installed from a CurseForge pack");
        };
        println!(
            "{} {:?}: {} bytes",
            base.name, base.version_label, base.size_bytes
        );
        let tmp = std::env::temp_dir().join(format!("atlas-real-{}", crate::util::ids::new_id()));
        let zip = tmp.join("pack.zip");
        fetch::download(
            &reqwest::Client::new(),
            &base.download_url,
            &zip,
            Some(&base.sha1),
            None,
            &mut |_| {},
        )
        .await
        .unwrap();
        let index = read_index(&zip, base.source).unwrap();
        let (mut as_is, mut changed, mut pack_copy, mut missing_here) = (0, 0, 0, 0);
        for (rel, file) in &index {
            let local = dir.join(rel);
            let Ok(bytes) = std::fs::read(&local) else {
                missing_here += 1;
                continue;
            };
            if hex::encode(Sha256::digest(&bytes)) == file.sha256 {
                as_is += 1;
            } else if super::super::manifest::upload_refusal(rel, bytes.len() as u64).is_none() {
                changed += 1;
            } else {
                pack_copy += 1;
            }
        }
        println!(
            "pack files: {} · as the pack has them: {as_is} · changed, uploaded: {changed} · changed, pack's copy: {pack_copy} · not here: {missing_here}",
            index.len()
        );
        assert!(as_is > 0);
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn client_overrides_win_in_a_modrinth_pack() {
        let dir = std::env::temp_dir().join(format!("atlas-pack-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pack = dir.join("pack.mrpack");
        write_zip(
            &pack,
            &[
                ("modrinth.index.json", b"{}"),
                ("overrides/options.txt", b"server"),
                ("client-overrides/options.txt", b"client"),
            ],
        );
        let index = read_index(&pack, ContentSource::Modrinth).unwrap();
        assert_eq!(
            index["options.txt"].sha256,
            hex::encode(Sha256::digest(b"client"))
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
