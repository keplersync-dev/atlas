//! Installing new versions of shared instances by themselves, for packs with "Keep it up
//! to date automatically" on. Only when nothing needs the player's say: anything else
//! stays an update they start themselves.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Runtime};

use crate::db;
use crate::dto::{SyncOptions, SyncPreview, SyncState};
use crate::error::AppResult;
use crate::state::AppState;

/// How long a shared instance must stay quiet before its new version installs, so three
/// publishes in a row install once.
const QUIET: Duration = Duration::from_secs(120);

/// The latest change seen per shared instance: a wait ends early when a newer one came.
static LATEST: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(Default::default);

/// A shared instance has a new version: install it once things go quiet.
pub fn soon<R: Runtime>(state: &AppState, app: &AppHandle<R>, shared_id: &str) {
    let seen = {
        let mut latest = LATEST.lock().unwrap_or_else(|e| e.into_inner());
        let n = latest.entry(shared_id.to_owned()).or_default();
        *n += 1;
        *n
    };
    let (state, app, shared_id) = (state.clone(), app.clone(), shared_id.to_owned());
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(QUIET).await;
        let current = LATEST
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&shared_id)
            .copied();
        if current != Some(seen) {
            return;
        }
        let Ok(rows) = db::instances::list(&state.db).await else {
            return;
        };
        for row in rows
            .iter()
            .filter(|r| super::shared::server_id(r) == Some(shared_id.as_str()))
        {
            check(&state, &app, &row.id).await;
        }
    });
}

/// Every shared instance: for versions published while Atlas was closed.
pub async fn check_all<R: Runtime>(state: &AppState, app: &AppHandle<R>) {
    // Refreshes the cached list, so the latest versions are known.
    if let Err(e) = super::shared::list(state, app).await {
        tracing::info!(error = %e, "couldn't check shared instances for new versions");
        return;
    }
    let Ok(rows) = db::instances::list(&state.db).await else {
        return;
    };
    for row in rows
        .iter()
        .filter(|r| super::shared::server_id(r).is_some())
    {
        check(state, app, &row.id).await;
    }
}

/// Install this instance's new version if it's on and safe. Never fails loudly: an
/// update that can't start here is still there for the player to start.
pub async fn check<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) {
    if let Err(e) = try_check(state, app, id).await {
        tracing::info!(instance = %id, error = %e, "couldn't update by itself");
    }
}

async fn try_check<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) -> AppResult<()> {
    if state.jobs.is_busy(id) || !db::instances::auto_update(&state.db, id).await? {
        return Ok(());
    }
    let Some(row) = db::instances::get(&state.db, id).await? else {
        return Ok(());
    };
    let Some(info) = super::sync::info(state, &row).await? else {
        return Ok(());
    };
    if info.state != SyncState::UpdateAvailable {
        return Ok(());
    }
    let preview = super::sync::preview(state, id).await?;
    if !safe(&preview) {
        tracing::info!(instance = %row.name, "this update needs the player");
        return Ok(());
    }
    tracing::info!(instance = %row.name, to = preview.to_revision, "updating by itself");
    // The job waits for the game to close and backs up first. New optional mods aren't
    // installed: they're the player's choice.
    super::sync::sync(state, app, id, SyncOptions::default()).await
}

/// Nothing to approve, none of the player's own changes undone, and the launcher's
/// versions stay as they are.
fn safe(p: &SyncPreview) -> bool {
    p.unverified.is_empty()
        && p.config_conflicts.is_empty()
        && p.overwritten_local_changes.is_empty()
        && p.launcher_change.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ConfigConflict;

    fn preview() -> SyncPreview {
        SyncPreview {
            from_revision: 1,
            to_revision: 2,
            changes: Default::default(),
            download_bytes: 0,
            manual_downloads: vec![],
            overwritten_local_changes: vec![],
            config_conflicts: vec![],
            launcher_change: None,
            game_running: true,
            new_optional: vec![],
            unverified: vec![],
        }
    }

    #[test]
    fn installs_only_what_needs_no_say() {
        assert!(safe(&preview()), "a running game only delays it");
        let mut p = preview();
        p.unverified.push("custom-mod".into());
        assert!(!safe(&p));
        let mut p = preview();
        p.overwritten_local_changes.push("Sodium".into());
        assert!(!safe(&p));
        let mut p = preview();
        p.config_conflicts.push(ConfigConflict {
            path: "options.txt".into(),
            mine: "a".into(),
            theirs: "b".into(),
        });
        assert!(!safe(&p));
    }
}
