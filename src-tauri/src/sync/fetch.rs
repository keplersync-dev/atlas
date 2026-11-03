//! Getting the bytes a plan needs into Atlas's cache, verified.
//!
//!   cache/files/<sha1[0..2]>/<sha1>              mods, packs and modpacks from Modrinth and
//!                                                CurseForge, reused across instances
//!   cache/blobs/<sha256[0..2]>/<sha256>          uploads to Atlas, config files, modpack files
//!
//! Uploads stay out of `files/`: there they could stand in for a Modrinth or CurseForge
//! file with the same hashes, which installs without the approval an upload needs.
//!
//! A file only enters the cache after its hashes match; nothing is ever written straight
//! to its final place in a game folder.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::manifest::{Entry, Override};
use super::source::Source;
use crate::dto::{ContentSource, ManualDownload};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::scan::hash::hash_file;
use crate::state::AppPaths;
use crate::util::ids::new_id;

/// A download gives up when no bytes arrive for this long.
pub const STALL: Duration = Duration::from_secs(30);

pub fn cache_file(paths: &AppPaths, sha1: &str) -> PathBuf {
    paths.cache.join("files").join(&sha1[..2]).join(sha1)
}

pub fn blob_file(paths: &AppPaths, sha256: &str) -> PathBuf {
    paths.cache.join("blobs").join(&sha256[..2]).join(sha256)
}

/// Where a content file is cached: by where it comes from, never by its hashes alone.
pub fn content_file(paths: &AppPaths, entry: &Entry) -> PathBuf {
    if entry.source == ContentSource::Upload {
        blob_file(paths, &entry.sha256)
    } else {
        cache_file(paths, &entry.sha1)
    }
}

/// Older versions cached uploads by sha1 beside mod-site downloads (cache/<sha1[0..2]>/),
/// where one could stand in for a Modrinth or CurseForge file. That cache is dropped; its
/// files download again when needed.
pub fn drop_old_cache(paths: &AppPaths) {
    let Ok(dirs) = std::fs::read_dir(&paths.cache) else {
        return;
    };
    for dir in dirs.flatten() {
        let name = dir.file_name();
        let old = name.len() == 2
            && name
                .to_str()
                .is_some_and(|n| n.bytes().all(|b| b.is_ascii_hexdigit()));
        if old && let Err(e) = std::fs::remove_dir_all(dir.path()) {
            tracing::warn!(error = %e, "couldn't remove the old download cache");
        }
    }
}

/// How a file will be obtained.
#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    Cached,
    /// A file already on this PC with the same bytes (e.g. a disabled copy).
    Local(PathBuf),
    /// The shared instance's dev folder has it.
    Source(PathBuf),
    Download(String),
    /// Uploaded to the Atlas server: ask it for a download link first.
    Blob,
    /// CurseForge file whose author disabled third-party downloads.
    Manual(ManualDownload),
    Unavailable(String),
}

pub fn route(
    paths: &AppPaths,
    source: &Source,
    game_dir: &Path,
    entry: &Entry,
    local: Option<&String>,
) -> Route {
    if content_file(paths, entry).is_file() {
        return Route::Cached;
    }
    if let Some(l) = local {
        return Route::Local(game_dir.join(l));
    }
    if let Some(p) = source.local_copy(&entry.path) {
        return Route::Source(p);
    }
    if let Some(url) = &entry.download_url {
        return Route::Download(url.clone());
    }
    match (entry.source, &entry.page_url) {
        // Checked again here: a synced manifest saved before the check came in isn't re-validated.
        (ContentSource::Curseforge, Some(page)) if super::manifest::allowed_page(page) => {
            Route::Manual(ManualDownload {
                content_name: entry.name.clone(),
                file_name: entry
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&entry.path)
                    .to_string(),
                page_url: direct_download(page),
            })
        }
        (ContentSource::Upload, _) if source.server_id().is_some() => Route::Blob,
        (ContentSource::Upload, _) => Route::Unavailable(format!(
            "{} was uploaded to Atlas, but this instance isn't linked to the server",
            entry.name
        )),
        _ => Route::Unavailable(format!("{} has no download link", entry.name)),
    }
}

/// Copy `from` into the cache under `to` if its hashes match (at least one is given).
fn admit(from: &Path, to: &Path, sha1: Option<&str>, sha256: Option<&str>) -> AppResult<()> {
    debug_assert!(sha1.is_some() || sha256.is_some());
    let h = hash_file(from)?;
    if sha1.is_some_and(|s| s != h.sha1) || sha256.is_some_and(|s| s != h.sha256) {
        return Err(AppError::conflict(format!(
            "{} doesn't match the shared version (it may be corrupt or a different file)",
            from.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        )));
    }
    std::fs::create_dir_all(to.parent().expect("cache files have a parent"))?;
    let tmp = to.with_extension(format!("{}.part", new_id()));
    std::fs::copy(from, &tmp)?;
    std::fs::rename(&tmp, to)?;
    Ok(())
}

/// Bring one content file into the cache by its route. Blocking for copies; downloads stream.
/// Uploads (`Route::Blob`) need a link from the server first: apply.rs fetches those.
pub async fn content(
    paths: &AppPaths,
    http: &reqwest::Client,
    entry: &Entry,
    route: &Route,
    on_bytes: &mut (dyn FnMut(u64) + Send),
) -> AppResult<()> {
    let target = content_file(paths, entry);
    match route {
        Route::Cached => Ok(()),
        Route::Local(p) | Route::Source(p) => {
            let (p, t, sha1, sha256) =
                (p.clone(), target, entry.sha1.clone(), entry.sha256.clone());
            tokio::task::spawn_blocking(move || admit(&p, &t, Some(&sha1), Some(&sha256)))
                .await
                .map_err(AppError::from)??;
            on_bytes(entry.size_bytes);
            Ok(())
        }
        Route::Download(url) => {
            download(
                http,
                url,
                &target,
                Some(&entry.sha1),
                Some(&entry.sha256),
                entry.size_bytes,
                on_bytes,
            )
            .await
        }
        Route::Blob => unreachable!("apply.rs downloads uploads itself"),
        Route::Manual(m) => Err(AppError::conflict(format!(
            "{} has to be downloaded by hand",
            m.content_name
        ))),
        Route::Unavailable(why) => Err(AppError::conflict(why.clone())),
    }
}

/// Downloads come from Modrinth, CurseForge or Atlas's file storage, so this names none.
fn download_failed() -> AppError {
    AppError::new(
        ErrorCode::Offline,
        "A download didn't finish. Check your internet connection and try again.",
    )
}

/// Download `url` to a temporary file, then admit it to `target` if its hashes match.
/// Gives up once it's over `size_bytes`, the size it should be.
pub async fn download(
    http: &reqwest::Client,
    url: &str,
    target: &Path,
    sha1: Option<&str>,
    sha256: Option<&str>,
    size_bytes: u64,
    on_bytes: &mut (dyn FnMut(u64) + Send),
) -> AppResult<()> {
    let offline = |e: reqwest::Error| {
        // The URL may be presigned: log the host only.
        let host = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_default();
        tracing::warn!(error = %e.without_url(), %host, "download failed");
        download_failed()
    };
    std::fs::create_dir_all(target.parent().expect("cache files have a parent"))?;
    // Unique: another instance's job may be fetching the same file right now.
    let tmp = target.with_extension(format!("{}.download", new_id()));
    let stalled = || {
        tracing::warn!("download stalled");
        download_failed()
    };
    let mut resp = tokio::time::timeout(STALL, http.get(url).send())
        .await
        .map_err(|_| stalled())?
        .map_err(offline)?
        .error_for_status()
        .map_err(offline)?;
    {
        let mut file = std::fs::File::create(&tmp)?;
        let mut written = 0u64;
        while let Some(chunk) = tokio::time::timeout(STALL, resp.chunk())
            .await
            .map_err(|_| stalled())?
            .map_err(offline)?
        {
            written += chunk.len() as u64;
            if written > size_bytes {
                drop(file);
                let _ = std::fs::remove_file(&tmp);
                return Err(AppError::conflict(
                    "A download is bigger than the shared version (it may be corrupt or a different file)",
                ));
            }
            file.write_all(&chunk)?;
            on_bytes(chunk.len() as u64);
        }
        file.sync_all()?;
    }
    let (tmp2, t, sha1, sha256) = (
        tmp.clone(),
        target.to_path_buf(),
        sha1.map(str::to_owned),
        sha256.map(str::to_owned),
    );
    let result =
        tokio::task::spawn_blocking(move || admit(&tmp2, &t, sha1.as_deref(), sha256.as_deref()))
            .await
            .map_err(AppError::from)?;
    let _ = std::fs::remove_file(&tmp);
    result
}

/// Bring a config file uploaded to the Atlas server into the cache, from a download link.
pub async fn config_from(
    paths: &AppPaths,
    http: &reqwest::Client,
    o: &Override,
    url: &str,
) -> AppResult<PathBuf> {
    let target = blob_file(paths, &o.sha256);
    if !target.is_file() {
        download(
            http,
            url,
            &target,
            None,
            Some(&o.sha256),
            o.size_bytes,
            &mut |_| {},
        )
        .await?;
    }
    Ok(target)
}

/// Bring a config file into the cache from a dev source folder.
pub async fn config(paths: &AppPaths, source: &Source, o: &Override) -> AppResult<PathBuf> {
    let target = blob_file(paths, &o.sha256);
    if target.is_file() {
        return Ok(target);
    }
    let from = source
        .local_copy(&o.path)
        .ok_or_else(|| AppError::conflict(format!("{} isn't available", o.path)))?;
    let (t, sha256) = (target.clone(), o.sha256.clone());
    tokio::task::spawn_blocking(move || admit(&from, &t, None, Some(&sha256)))
        .await
        .map_err(AppError::from)??;
    Ok(target)
}

/// CurseForge's page that downloads this exact file after a few seconds, from its file
/// page (`…/files/<id>`). The file page's big Download button gets the mod's newest file,
/// which often isn't the one the pack needs.
fn direct_download(page: &str) -> String {
    match page.rsplit_once("/files/") {
        Some((project, id)) if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) => {
            format!("{project}/download/{id}")
        }
        _ => page.to_owned(),
    }
}

/// Look in the Downloads folder for files the user downloaded by hand since `since`,
/// and take any whose sha1 matches a wanted entry into the cache. Returns how many arrived.
/// A file taken in leaves Downloads: it was only downloaded for Atlas, which has it now.
pub fn pick_up_manual_downloads(paths: &AppPaths, wanted: &[&Entry], since: SystemTime) -> usize {
    let Ok(dir) = std::fs::read_dir(&paths.downloads) else {
        return 0;
    };
    let mut arrived = 0;
    for file in dir.flatten() {
        let Ok(meta) = file.metadata() else { continue };
        let recent = meta
            .modified()
            .is_ok_and(|t| t + Duration::from_secs(5) >= since);
        if !meta.is_file() || !recent {
            continue;
        }
        // Cheap check first: only hash files of a size we're waiting for. Several wanted
        // files can share a size, so the hash picks which one this is.
        let same_size = || {
            wanted
                .iter()
                .filter(|e| e.size_bytes == meta.len() && !content_file(paths, e).is_file())
        };
        if same_size().next().is_none() {
            continue;
        }
        let Ok(h) = hash_file(&file.path()) else {
            continue;
        };
        let Some(e) = same_size().find(|e| e.sha1 == h.sha1) else {
            continue;
        };
        if admit(
            &file.path(),
            &content_file(paths, e),
            Some(&e.sha1),
            Some(&e.sha256),
        )
        .is_ok()
        {
            arrived += 1;
            if let Err(e) = std::fs::remove_file(file.path()) {
                tracing::debug!(error = %e, "couldn't remove a manual download from Downloads");
            }
        }
    }
    arrived
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curseforge_files_download_directly() {
        assert_eq!(
            direct_download(
                "https://www.curseforge.com/minecraft/mc-mods/silent-fullbright/files/2702370"
            ),
            "https://www.curseforge.com/minecraft/mc-mods/silent-fullbright/download/2702370"
        );
        let other = "https://www.curseforge.com/minecraft/mc-mods/silent-fullbright";
        assert_eq!(direct_download(other), other);
    }

    #[test]
    fn manual_downloads_are_taken_out_of_downloads() {
        let root =
            std::env::temp_dir().join(format!("atlas-fetch-test-{}", crate::util::ids::new_id()));
        let mut paths = AppPaths::new(&root);
        paths.downloads = root.join("Downloads");
        std::fs::create_dir_all(&paths.downloads).unwrap();
        let wanted_file = paths.downloads.join("essentialtweaks-1.1.0.jar");
        std::fs::write(&wanted_file, b"the mod").unwrap();
        let other = paths.downloads.join("holiday.png");
        std::fs::write(&other, b"not a mod").unwrap();

        let h = hash_file(&wanted_file).unwrap();
        let entry: Entry = serde_json::from_value(serde_json::json!({
            "kind": "mod", "source": "curseforge", "name": "Essential Tweaks",
            "path": "mods/essentialtweaks-1.1.0.jar", "sha1": h.sha1, "sha256": h.sha256,
            "sizeBytes": 7, "required": true
        }))
        .unwrap();
        let since = SystemTime::now() - Duration::from_secs(60);

        assert_eq!(pick_up_manual_downloads(&paths, &[&entry], since), 1);
        assert!(cache_file(&paths, &entry.sha1).is_file(), "Atlas has it");
        assert!(!wanted_file.exists(), "and it left Downloads");
        assert!(other.exists(), "other files stay");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_manual_download_is_matched_by_hash_not_just_size() {
        let root =
            std::env::temp_dir().join(format!("atlas-fetch-test-{}", crate::util::ids::new_id()));
        let mut paths = AppPaths::new(&root);
        paths.downloads = root.join("Downloads");
        std::fs::create_dir_all(&paths.downloads).unwrap();
        let entry = |name: &str, bytes: &[u8]| -> Entry {
            let file = root.join(name);
            std::fs::write(&file, bytes).unwrap();
            let h = hash_file(&file).unwrap();
            serde_json::from_value(serde_json::json!({
                "kind": "mod", "source": "curseforge", "name": name,
                "path": format!("mods/{name}"), "sha1": h.sha1, "sha256": h.sha256,
                "sizeBytes": bytes.len(), "required": true
            }))
            .unwrap()
        };
        let first = entry("first.jar", b"aaaa");
        let second = entry("second.jar", b"bbbb");
        std::fs::write(paths.downloads.join("second.jar"), b"bbbb").unwrap();
        let since = SystemTime::now() - Duration::from_secs(60);

        assert_eq!(
            pick_up_manual_downloads(&paths, &[&first, &second], since),
            1
        );
        assert!(cache_file(&paths, &second.sha1).is_file());
        assert!(!cache_file(&paths, &first.sha1).is_file());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_upload_never_stands_in_for_a_mod_site_file() {
        let root =
            std::env::temp_dir().join(format!("atlas-fetch-test-{}", crate::util::ids::new_id()));
        let paths = AppPaths::new(&root);
        let url = "https://cdn.modrinth.com/data/x/versions/y/x.jar";
        let entry = |source: &str| -> Entry {
            serde_json::from_value(serde_json::json!({
                "kind": "mod", "source": source, "name": "X", "path": "mods/x.jar",
                "sha1": "a".repeat(40), "sha256": "b".repeat(64), "sizeBytes": 4,
                "downloadUrl": url, "required": true
            }))
            .unwrap()
        };
        let (upload, modrinth) = (entry("upload"), entry("modrinth"));
        // The upload as an update caches it now, and as older versions did, by its sha1.
        let old = paths.cache.join("aa").join("a".repeat(40));
        for file in [content_file(&paths, &upload), old.clone()] {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, b"jar!").unwrap();
        }
        let source = Source::Dev(root.join("no source"));
        assert_eq!(route(&paths, &source, &root, &upload, None), Route::Cached);
        assert_eq!(
            route(&paths, &source, &root, &modrinth, None),
            Route::Download(url.into()),
            "the same hashes from Modrinth come from Modrinth"
        );
        drop_old_cache(&paths);
        assert!(!old.exists(), "the old cache is dropped");
        assert!(content_file(&paths, &upload).is_file());
        std::fs::remove_dir_all(&root).ok();
    }

    #[tokio::test]
    async fn a_download_bigger_than_it_should_be_stops() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/x.jar", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let _ = std::io::Read::read(&mut s, &mut [0; 4096]);
            let body = vec![b'x'; 256 * 1024];
            let head = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n", body.len());
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&body);
        });
        let root =
            std::env::temp_dir().join(format!("atlas-fetch-test-{}", crate::util::ids::new_id()));
        let target = root.join("x.jar");
        let sha256 = "0".repeat(64);
        let err = download(
            &reqwest::Client::new(),
            &url,
            &target,
            None,
            Some(&sha256),
            1024,
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::Conflict);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0, "nothing left");
        std::fs::remove_dir_all(&root).ok();
    }
}
