//! Where a shared instance's versions come from.
//!
//! - **Server:** the Atlas server. Manifests come from `GET /shared-instances/{id}/revisions/{n}`
//!   and are kept in `manifests/<id>/<n>.json` (revisions never change); the latest revision
//!   and frozen flag come from the cached shared-instance list, so summaries need no network.
//! - **Dev:** a folder standing in for the server (debug builds, `dev_link_manifest`):
//!
//!   <folder>/revisions/1.json, 2.json, ...   one manifest per revision
//!   <folder>/files/<relative path>            the bytes: files/mods/sodium.jar, files/config/a.json

use std::path::{Path, PathBuf};

use reqwest::Method;
use sqlx::SqlitePool;

use super::manifest::{self, Manifest};
use crate::db;
use crate::dto::Revision;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Clone)]
pub enum Source {
    Server {
        shared_instance_id: String,
        latest: u32,
        frozen: bool,
    },
    Dev(PathBuf),
}

fn setting_key(shared_instance_id: &str) -> String {
    format!("dev_source:{shared_instance_id}")
}

/// The source behind a shared instance id. `synced` is the instance's revision, used as
/// the latest when the server's list hasn't been fetched yet.
pub async fn for_shared(
    state: &AppState,
    shared_instance_id: &str,
    synced: u32,
) -> AppResult<Source> {
    let folder: Option<String> =
        db::settings::get(&state.db, &setting_key(shared_instance_id)).await?;
    if let Some(folder) = folder {
        return Ok(Source::Dev(PathBuf::from(folder)));
    }
    let cached = crate::services::shared::cached(state, shared_instance_id).await?;
    Ok(Source::Server {
        shared_instance_id: shared_instance_id.to_owned(),
        latest: cached
            .as_ref()
            .map_or(synced, |s| s.latest_revision)
            .max(synced),
        frozen: cached.is_some_and(|s| s.frozen),
    })
}

pub async fn save_dev(db: &SqlitePool, shared_instance_id: &str, folder: &Path) -> AppResult<()> {
    db::settings::set(
        db,
        &setting_key(shared_instance_id),
        &folder.to_string_lossy(),
    )
    .await?;
    Ok(())
}

pub async fn forget(db: &SqlitePool, shared_instance_id: &str) -> AppResult<()> {
    db::settings::delete(db, &setting_key(shared_instance_id)).await?;
    Ok(())
}

fn refused(revision: u32, e: manifest::Invalid) -> AppError {
    tracing::warn!(revision, error = %e, "refused manifest");
    AppError::invalid_input("This shared instance contains files Atlas won't install")
}

impl Source {
    /// The newest revision number, or 0 if there is none.
    pub fn latest_revision(&self) -> u32 {
        match self {
            Source::Server { latest, .. } => *latest,
            Source::Dev(folder) => std::fs::read_dir(folder.join("revisions"))
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| e.path().file_stem()?.to_str()?.parse::<u32>().ok())
                .max()
                .unwrap_or(0),
        }
    }

    /// A revision's manifest, validated (hostile-input rules).
    pub async fn manifest(&self, state: &AppState, revision: u32) -> AppResult<Manifest> {
        match self {
            Source::Server {
                shared_instance_id, ..
            } => {
                let dir = state.paths.manifests.join(shared_instance_id);
                let file = dir.join(format!("{revision}.json"));
                if let Ok(bytes) = std::fs::read(&file) {
                    return manifest::parse(&bytes).map_err(|e| refused(revision, e));
                }
                let path = format!("/shared-instances/{shared_instance_id}/revisions/{revision}");
                let r: Revision = state.cloud.call(Method::GET, &path, None::<&()>).await?;
                manifest::validate(&r.manifest).map_err(|e| refused(revision, e))?;
                std::fs::create_dir_all(&dir)?;
                std::fs::write(&file, serde_json::to_vec(&r.manifest)?)?;
                Ok(r.manifest)
            }
            Source::Dev(folder) => {
                let file = folder.join("revisions").join(format!("{revision}.json"));
                let bytes = std::fs::read(&file).map_err(|_| {
                    AppError::not_found(format!("Revision {revision} isn't available"))
                })?;
                manifest::parse(&bytes).map_err(|e| refused(revision, e))
            }
        }
    }

    /// A local file holding the bytes at `path` (a content or override path) for this
    /// source, if it has them. Hashes are checked by the caller.
    pub fn local_copy(&self, path: &str) -> Option<PathBuf> {
        match self {
            Source::Server { .. } => None,
            Source::Dev(folder) => Some(folder.join("files").join(path)).filter(|p| p.is_file()),
        }
    }

    pub fn is_frozen(&self) -> bool {
        match self {
            Source::Server { frozen, .. } => *frozen,
            Source::Dev(_) => false,
        }
    }

    /// The server's id for it, when it's on the server.
    pub fn server_id(&self) -> Option<&str> {
        match self {
            Source::Server {
                shared_instance_id, ..
            } => Some(shared_instance_id),
            Source::Dev(_) => None,
        }
    }
}
