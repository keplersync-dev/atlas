use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use reqwest::Method;
use tauri::{AppHandle, Runtime};

use super::configs::plan_configs;
use super::{file_rules, from_pack, is_upload, live, mode_of, not_checked};
use crate::db::{self, content::ContentRow, instances::InstanceRow};
use crate::dto::{
    ContentSource, ManualDownload, MemberRole, PackInfo, PublishItem, PublishPlan, ShareMode,
    Storage, WorkStage, from_wire,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::services::work::Reporter;
use crate::state::AppState;
use crate::sync::manifest::{Keep, Override};
use crate::sync::pack;

/// Files on no mod site that the modpack names by CurseForge id: CurseForge can't find
/// some files by fingerprint, but the pack lists them, so they aren't uploads.
async fn look_up_in_pack<R: Runtime>(state: &AppState, app: &AppHandle<R>, row: &InstanceRow) {
    let has_local = db::content::list(&state.db, &row.id)
        .await
        .is_ok_and(|rows| live(&rows).any(|r| r.source == "local"));
    if !has_local {
        return;
    }
    let Some(base) = pack::base_for(state, row).await else {
        return;
    };
    let work = Reporter::new(app, &row.id);
    let total = if pack::needs_download(&state.paths, &base) {
        base.size_bytes
    } else {
        0
    };
    let mut got = 0;
    let mut progress = |n: u64| {
        got += n;
        work.report(WorkStage::Downloading, got.min(total), total);
    };
    let found = match pack::curseforge_file_ids(state, &base, &mut progress).await {
        Ok(ids) => crate::resolve::resolve_from_pack(&state.db, &state.cloud, &row.id, &ids).await,
        Err(e) => Err(e),
    };
    match found {
        Ok(0) => {}
        Ok(n) => {
            tracing::info!(pack = %base.name, files = n, "found files by the modpack's list");
            emit(
                app,
                AtlasEvent::InstanceChanged {
                    instance_id: row.id.clone(),
                },
            );
        }
        Err(e) => {
            tracing::info!(pack = %base.name, error = %e, "couldn't look files up in the modpack's list")
        }
    }
}

fn manual_download(r: &ContentRow) -> Option<ManualDownload> {
    let page = r.page_url.clone()?;
    (from_wire(&r.source) == Some(ContentSource::Curseforge) && r.download_url.is_none()).then(
        || ManualDownload {
            content_name: r.name.clone(),
            file_name: r.path.rsplit('/').next().unwrap_or(&r.path).to_owned(),
            page_url: page,
        },
    )
}

pub async fn prepare<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
) -> AppResult<PublishPlan> {
    let row = db::instances::require(&state.db, id).await?;
    if row.shared_instance_id.is_some() && row.role() == Some(MemberRole::Viewer) {
        return Err(AppError::forbidden(
            "Only the owner and editors can send updates",
        ));
    }
    // Read the folder now, after any scan already running: a just-tracked instance may
    // not have been scanned yet, and the plan must match what's on disk.
    let started = std::time::Instant::now();
    let mut took = Vec::new();
    let mut lap = |step: &'static str| took.push((step, started.elapsed().as_millis() as u64));
    crate::services::instances::rescan(state, app, id).await?;
    lap("rescan");
    // Files not looked up yet would all count as uploads.
    crate::services::instances::look_up_now(state, app, id).await;
    lap("look up");
    look_up_in_pack(state, app, &row).await;
    lap("modpack list");
    let mut rows = db::content::list(&state.db, id).await?;
    if live(&rows).any(not_checked) {
        // Once more before telling the owner: the sites may only have hiccuped.
        crate::services::instances::look_up_now(state, app, id).await;
        rows = db::content::list(&state.db, id).await?;
    }
    let to_check: Vec<String> = live(&rows)
        .filter(|r| is_upload(r))
        .map(|r| r.sha256.clone())
        .collect();
    lap("look up again");
    // The server's answers and the file read don't depend on each other: at once.
    let stored = async {
        match state.cloud.stored(to_check).await {
            Ok(found) => found.into_iter().collect::<HashSet<String>>(),
            Err(e) => {
                tracing::info!(error = %e, "couldn't check which files are uploaded");
                HashSet::new()
            }
        }
    };
    let storage = async {
        match state
            .cloud
            .call(Method::GET, "/me/storage", None::<&()>)
            .await
        {
            Ok(storage) => Ok(storage),
            Err(_) => state.cloud.session().await.map(|s| {
                s.user.map(|me| me.storage).unwrap_or(Storage {
                    used_bytes: 0,
                    quota_bytes: 0,
                    max_file_bytes: 0,
                })
            }),
        }
    };
    let work = Reporter::new(app, id);
    let (stored, storage, configs) =
        tokio::join!(stored, storage, plan_configs(state, &row, &work));
    let (storage, configs) = (storage?, configs?);
    lap("server and other files");
    tracing::info!(instance = %row.name, ?took, "prepared a share (ms since start)");
    let synced = row.synced_manifest();
    let replaced: HashSet<&str> =
        crate::sync::state::local_changes(MemberRole::Owner, &rows, synced.as_ref(), false)
            .changed
            .iter()
            .map(|r| r.id.as_str())
            .collect();
    let files_changed = synced
        .as_ref()
        .map(|m| configs_changed(&m.overrides, &configs.shared))
        .unwrap_or_default();
    let items = live(&rows)
        .map(|r| {
            let dto = r.to_dto();
            PublishItem {
                content_id: r.id.clone(),
                name: r.name.clone(),
                icon_url: dto.icon_url,
                kind: dto.kind,
                source: dto.source,
                path: r.path.clone(),
                size_bytes: r.size_bytes.max(0) as u64,
                is_new: r.published_share_mode.is_none(),
                share_mode: mode_of(r),
                published_share_mode: r.published_share_mode.as_deref().and_then(from_wire),
                needs_upload: is_upload(r) && !from_pack(r, &configs.pack_content),
                already_uploaded: stored.contains(&r.sha256),
                unchecked: not_checked(r),
                contents_changed: replaced.contains(r.id.as_str()),
            }
        })
        .collect();
    let mut removed: Vec<String> = rows
        .iter()
        .filter(|r| r.is_managed && r.installed && !r.enabled)
        .map(|r| r.name.clone())
        .collect();
    if let Some(synced) = &synced {
        for e in &synced.content {
            if e.required && !rows.iter().any(|r| r.path.eq_ignore_ascii_case(&e.path)) {
                removed.push(e.name.clone());
            }
        }
    }
    Ok(PublishPlan {
        base_revision: row.synced_revision.map(|r| r.max(0) as u32),
        items,
        removed,
        manual_downloads: live(&rows)
            .filter(|r| mode_of(r) != ShareMode::Private)
            .filter_map(manual_download)
            .collect(),
        storage,
        pack: configs.base.as_ref().map(|b| PackInfo {
            name: b.name.clone(),
            version_label: b.version_label.clone(),
            source: b.source,
            files: (configs.shared.iter().filter(|(o, _)| o.in_pack).count()
                + live(&rows)
                    .filter(|r| from_pack(r, &configs.pack_content))
                    .count()) as u32,
        }),
        left_out: configs.left_out,
        files: configs.files,
        rules: file_rules(state, id).await?,
        versions: synced
            .as_ref()
            .and_then(|m| crate::sync::state::version_change(&row, m)),
        files_changed,
    })
}

/// Config and other files whose shared copy would differ from the shared version's.
fn configs_changed(before: &[Override], now: &[(Override, PathBuf)]) -> Vec<String> {
    let old: HashMap<&str, (&str, Option<Keep>)> = before
        .iter()
        .map(|o| (o.path.as_str(), (o.sha256.as_str(), o.keep)))
        .collect();
    let new: HashMap<&str, (&str, Option<Keep>)> = now
        .iter()
        .map(|(o, _)| (o.path.as_str(), (o.sha256.as_str(), o.keep)))
        .collect();
    let mut out: Vec<String> = new
        .iter()
        .filter(|(path, file)| old.get(*path) != Some(file))
        .map(|(path, _)| path.to_string())
        .chain(
            old.keys()
                .filter(|path| !new.contains_key(*path))
                .map(|path| path.to_string()),
        )
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, sha: &str, keep: Option<Keep>) -> Override {
        Override {
            path: path.into(),
            sha256: sha.into(),
            size_bytes: 1,
            in_pack: false,
            keep,
        }
    }

    #[test]
    fn config_changes_against_the_shared_version() {
        let before = [
            file("config/a.json", "1", None),
            file("config/b.json", "2", None),
            file("options.txt", "3", Some(Keep::FirstInstall)),
            file("config/gone.json", "4", None),
        ];
        let now = [
            file("config/a.json", "1", None),
            file("config/b.json", "9", None),
            file("options.txt", "3", None),
            file("config/new.json", "5", None),
        ]
        .map(|o| (o, PathBuf::new()));
        assert_eq!(
            configs_changed(&before, &now),
            [
                "config/b.json",
                "config/gone.json",
                "config/new.json",
                "options.txt"
            ]
        );
        let same: Vec<(Override, PathBuf)> = before
            .iter()
            .cloned()
            .map(|o| (o, PathBuf::new()))
            .collect();
        assert!(configs_changed(&before, &same).is_empty());
    }
}
