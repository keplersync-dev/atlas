//! Getting the bytes a plan needs into Atlas's cache, verified.
//!
//!   cache/<sha1[0..2]>/<sha1>                    mods and packs, reused across instances
//!   cache/blobs/<sha256[0..2]>/<sha256>          config files (only have a sha256)
//!
//! A file only enters the cache after its hashes match; nothing is ever written straight
//! to its final place in a game folder.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::manifest::{Entry, Override};
use super::source::Source;
use crate::dto::{ContentSource, ManualDownload};
use crate::error::{AppError, AppResult};
use crate::scan::hash::hash_file;
use crate::state::AppPaths;

pub fn cache_file(paths: &AppPaths, sha1: &str) -> PathBuf {
    paths.cache.join(&sha1[..2]).join(sha1)
}

pub fn blob_file(paths: &AppPaths, sha256: &str) -> PathBuf {
    paths.cache.join("blobs").join(&sha256[..2]).join(sha256)
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
    if cache_file(paths, &entry.sha1).is_file() {
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
        (ContentSource::Curseforge, Some(page)) => Route::Manual(ManualDownload {
            content_name: entry.name.clone(),
            file_name: entry
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&entry.path)
                .to_string(),
            page_url: page.clone(),
        }),
        (ContentSource::Upload, _) if source.server_id().is_some() => Route::Blob,
        (ContentSource::Upload, _) => Route::Unavailable(format!(
            "{} was uploaded to Atlas, but this instance isn't linked to the server",
            entry.name
        )),
        _ => Route::Unavailable(format!("{} has no download link", entry.name)),
    }
}

/// Copy `from` into the cache under `to` if its hashes match.
fn admit(from: &Path, to: &Path, sha1: Option<&str>, sha256: &str) -> AppResult<()> {
    let h = hash_file(from)?;
    if sha1.is_some_and(|s| s != h.sha1) || h.sha256 != sha256 {
        return Err(AppError::conflict(format!(
            "{} doesn't match the shared version (it may be corrupt or a different file)",
            from.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        )));
    }
    std::fs::create_dir_all(to.parent().expect("cache files have a parent"))?;
    let tmp = to.with_extension("part");
    std::fs::copy(from, &tmp)?;
    std::fs::rename(&tmp, to)?;
    Ok(())
}

/// Bring one content file into the cache by its route. Blocking for copies; downloads stream.
pub async fn content(
    paths: &AppPaths,
    http: &reqwest::Client,
    entry: &Entry,
    route: &Route,
    on_bytes: &mut (dyn FnMut(u64) + Send),
) -> AppResult<()> {
    let target = cache_file(paths, &entry.sha1);
    match route {
        Route::Cached => Ok(()),
        Route::Local(p) | Route::Source(p) => {
            let (p, t, sha1, sha256) =
                (p.clone(), target, entry.sha1.clone(), entry.sha256.clone());
            tokio::task::spawn_blocking(move || admit(&p, &t, Some(&sha1), &sha256))
                .await
                .map_err(|e| AppError::unknown(e.to_string()))??;
            on_bytes(entry.size_bytes);
            Ok(())
        }
        Route::Download(url) => {
            download(
                http,
                url,
                &target,
                Some(&entry.sha1),
                &entry.sha256,
                on_bytes,
            )
            .await
        }
        Route::Blob => Err(AppError::unknown(
            "Uploaded files need a download link first",
        )),
        Route::Manual(m) => Err(AppError::conflict(format!(
            "{} has to be downloaded by hand",
            m.content_name
        ))),
        Route::Unavailable(why) => Err(AppError::conflict(why.clone())),
    }
}

/// Download `url` to a temporary file, then admit it to `target` if its hashes match.
pub async fn download(
    http: &reqwest::Client,
    url: &str,
    target: &Path,
    sha1: Option<&str>,
    sha256: &str,
    on_bytes: &mut (dyn FnMut(u64) + Send),
) -> AppResult<()> {
    let offline = |e: reqwest::Error| {
        // The URL may be presigned: log the host only.
        let host = url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_default();
        tracing::warn!(error = %e, %host, "download failed");
        AppError::offline()
    };
    std::fs::create_dir_all(target.parent().expect("cache files have a parent"))?;
    let tmp = target.with_extension("download");
    let mut resp = http
        .get(url)
        .send()
        .await
        .map_err(offline)?
        .error_for_status()
        .map_err(offline)?;
    {
        let mut file = std::fs::File::create(&tmp)?;
        while let Some(chunk) = resp.chunk().await.map_err(offline)? {
            file.write_all(&chunk)?;
            on_bytes(chunk.len() as u64);
        }
        file.sync_all()?;
    }
    let (tmp2, t, sha1, sha256) = (
        tmp.clone(),
        target.to_path_buf(),
        sha1.map(str::to_owned),
        sha256.to_owned(),
    );
    let result = tokio::task::spawn_blocking(move || admit(&tmp2, &t, sha1.as_deref(), &sha256))
        .await
        .map_err(|e| AppError::unknown(e.to_string()))?;
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
        download(http, url, &target, None, &o.sha256, &mut |_| {}).await?;
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
    tokio::task::spawn_blocking(move || admit(&from, &t, None, &sha256))
        .await
        .map_err(|e| AppError::unknown(e.to_string()))??;
    Ok(target)
}

/// Look in the Downloads folder for files the user downloaded by hand since `since`,
/// and take any whose sha1 matches a wanted entry into the cache. Returns how many arrived.
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
        // Cheap check first: only hash files of a size we're waiting for.
        let Some(e) = wanted
            .iter()
            .find(|e| e.size_bytes == meta.len() && !cache_file(paths, &e.sha1).is_file())
        else {
            continue;
        };
        if admit(
            &file.path(),
            &cache_file(paths, &e.sha1),
            Some(&e.sha1),
            &e.sha256,
        )
        .is_ok()
        {
            arrived += 1;
        }
    }
    arrived
}
