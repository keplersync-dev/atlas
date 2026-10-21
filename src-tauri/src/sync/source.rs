//! Where a shared instance's versions come from.
//!
//! T4 has one kind: a **dev source**, a folder standing in for the server so the sync
//! engine can be built and tried before any server exists:
//!
//!   <folder>/revisions/1.json, 2.json, ...   one manifest per revision
//!   <folder>/files/<relative path>            the bytes: files/mods/sodium.jar, files/config/a.json
//!
//! An instance is linked to one with the debug-only `dev_link_manifest` command. T7 adds
//! the Atlas server (GET /shared-instances/{id}/revisions/{n}, downloads by URL).

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use super::manifest::{self, Manifest};
use crate::db;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub enum Source {
    Dev(PathBuf),
}

fn setting_key(shared_instance_id: &str) -> String {
    format!("dev_source:{shared_instance_id}")
}

/// The source behind a shared instance id, if Atlas knows one.
pub async fn for_shared(db: &SqlitePool, shared_instance_id: &str) -> AppResult<Option<Source>> {
    let folder: Option<String> = db::settings::get(db, &setting_key(shared_instance_id)).await?;
    Ok(folder.map(|f| Source::Dev(PathBuf::from(f))))
}

pub async fn save_dev(db: &SqlitePool, shared_instance_id: &str, folder: &Path) -> AppResult<()> {
    db::settings::set(db, &setting_key(shared_instance_id), &folder.to_string_lossy()).await?;
    Ok(())
}

pub async fn forget(db: &SqlitePool, shared_instance_id: &str) -> AppResult<()> {
    db::settings::delete(db, &setting_key(shared_instance_id)).await?;
    Ok(())
}

impl Source {
    /// The newest revision number, or 0 if there is none.
    pub fn latest_revision(&self) -> u32 {
        match self {
            Source::Dev(folder) => std::fs::read_dir(folder.join("revisions"))
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| e.path().file_stem()?.to_str()?.parse::<u32>().ok())
                .max()
                .unwrap_or(0),
        }
    }

    /// A revision's manifest, validated (hostile-input rules, backend-plan §6.2).
    pub fn manifest(&self, revision: u32) -> AppResult<Manifest> {
        match self {
            Source::Dev(folder) => {
                let file = folder.join("revisions").join(format!("{revision}.json"));
                let bytes = std::fs::read(&file)
                    .map_err(|_| AppError::not_found(format!("Revision {revision} isn't available")))?;
                manifest::parse(&bytes).map_err(|e| {
                    tracing::warn!(revision, error = %e, "refused manifest");
                    AppError::invalid_input("This shared instance contains files Atlas won't install")
                })
            }
        }
    }

    /// A local file holding the bytes at `path` (a content or override path) for this
    /// source, if it has them. Hashes are checked by the caller.
    pub fn local_copy(&self, path: &str) -> Option<PathBuf> {
        match self {
            Source::Dev(folder) => Some(folder.join("files").join(path)).filter(|p| p.is_file()),
        }
    }

    pub fn is_frozen(&self) -> bool {
        false
    }
}
