use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use reqwest::Method;
use sha2::{Digest, Sha256};
use tauri::Runtime;

use super::configs::{ConfigPlan, plan_configs};
use super::{from_pack, is_upload, live, not_checked};
use crate::cloud::files::UploadFile;
use crate::db::{content::ContentRow, instances::InstanceRow};
use crate::dto::{ShareMode, Storage, WorkStage};
use crate::error::{AppError, AppResult};
use crate::services::work::Reporter;
use crate::state::AppState;
use crate::sync::manifest::{MAX_ICON_BYTES, Override, PackBase};

/// Upload what the version needs that the server doesn't have yet: files on no mod site
/// that are shared, and the config files that aren't the modpack's own. Returns the
/// configs as overrides, the modpack they're based on, and its own mods (`from_pack`).
pub(super) async fn send_files<R: Runtime>(
    state: &AppState,
    row: &InstanceRow,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
    confirmed_rights: bool,
    work: &Reporter<R>,
) -> AppResult<(Vec<Override>, Option<PackBase>, HashMap<String, String>)> {
    let mut uploads: Vec<&ContentRow> = live(rows)
        .filter(|r| modes.get(&r.id) != Some(&ShareMode::Private) && is_upload(r))
        .collect();
    let storage: Storage = state
        .cloud
        .call(Method::GET, "/me/storage", None::<&()>)
        .await?;
    let ConfigPlan {
        base,
        shared: configs,
        pack_content,
        ..
    } = plan_configs(state, row, work).await?;
    // The modpack's own mods come out of its download: nothing to upload.
    uploads.retain(|r| !from_pack(r, &pack_content));
    // Not known to be on no mod site: uploading it could share a mod friends could
    // have downloaded, under a permission the owner never needed to give.
    if let Some(r) = uploads.iter().find(|r| not_checked(r)) {
        return Err(AppError::conflict(format!(
            "Couldn't check {} with Modrinth and CurseForge. Try again in a moment, or set it to Only me.",
            r.name
        )));
    }

    let game = PathBuf::from(&row.game_dir);
    // (file, sha256, size, name, rights confirmed): the checkbox covers mods and packs;
    // config files are the owner's own settings.
    let mut wanted: Vec<(PathBuf, String, u64, String, bool)> = uploads
        .iter()
        .map(|r| {
            (
                game.join(&r.path),
                r.sha256.clone(),
                r.size_bytes.max(0) as u64,
                r.name.clone(),
                confirmed_rights,
            )
        })
        .chain(configs.iter().filter(|(o, _)| !o.in_pack).map(|(o, file)| {
            (
                file.clone(),
                o.sha256.clone(),
                o.size_bytes,
                o.path.clone(),
                false,
            )
        }))
        .collect();
    let mut seen = HashSet::new();
    wanted.retain(|w| seen.insert(w.1.clone()));
    let stored: HashSet<String> = state
        .cloud
        .stored(wanted.iter().map(|w| w.1.clone()).collect())
        .await?
        .into_iter()
        .collect();
    // Files uploaded for an earlier version were confirmed then; only new ones need it,
    // the same rule the dialog uses.
    let new_uploads: Vec<&&ContentRow> = uploads
        .iter()
        .filter(|r| !stored.contains(&r.sha256))
        .collect();
    if !new_uploads.is_empty() && !confirmed_rights {
        return Err(AppError::invalid_input(
            "Confirm you have the right to share the files being uploaded",
        ));
    }
    if let Some(big) = new_uploads
        .iter()
        .find(|r| r.size_bytes.max(0) as u64 > storage.max_file_bytes)
    {
        return Err(AppError::new(
            crate::error::ErrorCode::QuotaExceeded,
            format!(
                "{} is larger than the per-file limit. Set it to Only me.",
                big.name
            ),
        ));
    }
    wanted.retain(|w| !stored.contains(&w.1));
    let new_bytes: u64 = wanted.iter().map(|w| w.2).sum();
    if storage.used_bytes + new_bytes > storage.quota_bytes {
        return Err(AppError::new(
            crate::error::ErrorCode::QuotaExceeded,
            "Not enough storage left. Set some files to Only me.",
        ));
    }
    // Files read when the dialog opened may have changed since (the launcher updated a mod).
    if let Some((_, _, _, name, _)) = wanted
        .iter()
        .find(|(file, _, size, ..)| std::fs::metadata(file).map(|m| m.len()).ok() != Some(*size))
    {
        return Err(AppError::conflict(format!(
            "{name} changed since you opened this. Close it and publish again."
        )));
    }
    // Progress in bytes, so it moves within a big file too.
    let total = new_bytes;
    let sent = Arc::new(AtomicU64::new(0));
    work.report(WorkStage::Uploading, 0, total);
    let files: Vec<UploadFile> = wanted
        .iter()
        .map(|(path, sha256, size, name, rights)| UploadFile {
            path: path.clone(),
            sha256: sha256.clone(),
            size: *size,
            name: name.clone(),
            rights_confirmed: *rights,
        })
        .collect();
    let counter = Arc::clone(&sent);
    let uploads = state.cloud.upload_all(&files, move |n| {
        counter.fetch_add(n, Ordering::Relaxed);
    });
    tokio::pin!(uploads);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(200));
    loop {
        tokio::select! {
            uploaded = &mut uploads => break uploaded?,
            _ = tick.tick() => {
                work.report(WorkStage::Uploading, sent.load(Ordering::Relaxed).min(total), total)
            }
        }
    }
    work.report(WorkStage::Uploading, total, total);
    if !wanted.is_empty() {
        tracing::info!(files = wanted.len(), bytes = new_bytes, "uploaded files");
    }
    Ok((
        configs.into_iter().map(|(o, _)| o).collect(),
        base,
        pack_content,
    ))
}

/// Upload the instance's picture so friends see it too; returns the manifest's `icon`.
/// A picture that can't be read or sent is left out rather than stopping the share.
pub(super) async fn send_icon(state: &AppState, row: &InstanceRow) -> Option<String> {
    send_picture(state, row.icon.as_deref()?, "The instance picture").await
}

/// Each shared file's picture, by content row id: Modrinth's and CurseForge's links as
/// they are, pictures Atlas took out of a file uploaded.
pub(super) async fn send_file_icons(
    state: &AppState,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
) -> HashMap<String, String> {
    let shared = live(rows).filter(|r| modes.get(&r.id) != Some(&ShareMode::Private));
    let found = futures_util::future::join_all(shared.filter_map(|r| {
        let icon = r.icon_url.as_deref()?;
        Some(async move {
            let out = if crate::sync::manifest::allowed_icon(icon) {
                Some(icon.to_owned())
            } else {
                send_picture(state, icon, &format!("{}'s picture", r.name)).await
            };
            Some((r.id.clone(), out?))
        })
    }))
    .await;
    found.into_iter().flatten().collect()
}

/// Upload a picture ("file:<path>", or an https link) unless the server has it; returns
/// "sha256:<hex>". None when it can't be read, isn't a picture, is too big, or can't be sent.
async fn send_picture(state: &AppState, icon: &str, name: &str) -> Option<String> {
    let bytes = if let Some(path) = icon.strip_prefix("file:") {
        tokio::fs::read(path).await.ok()?
    } else if icon.starts_with("https://") {
        let res = state.downloads.get(icon).send().await.ok()?;
        crate::cloud::files::body_up_to(res.error_for_status().ok()?, MAX_ICON_BYTES)
            .await
            .ok()??
    } else {
        return None;
    };
    if bytes.len() as u64 > MAX_ICON_BYTES || !crate::scan::icons::is_image(&bytes) {
        tracing::info!(size = bytes.len(), name, "picture left out of the share");
        return None;
    }
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let file = crate::scan::icons::shared_icon_file(&state.paths.icons, &sha256);
    if !file.is_file() && tokio::fs::write(&file, &bytes).await.is_err() {
        return None;
    }
    match state
        .cloud
        .upload(UploadFile {
            path: file,
            sha256: sha256.clone(),
            size: bytes.len() as u64,
            name: name.into(),
            rights_confirmed: false,
        })
        .await
    {
        Ok(()) => Some(format!("sha256:{sha256}")),
        Err(e) => {
            tracing::warn!(error = %e, name, "couldn't upload a picture");
            None
        }
    }
}
