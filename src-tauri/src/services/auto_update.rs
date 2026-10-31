//! New versions of shared instances: installed by themselves for packs with "Keep it up
//! to date automatically" on, when nothing needs the player's say. When something does
//! (approving an upload, undoing their changes), the player gets a notice instead.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Runtime};

use crate::db;
use crate::dto::{Notice, NoticeTarget, SyncOptions, SyncPreview, SyncState};
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

/// A new version of this instance: install it if that's on and safe, or tell the player
/// when it needs them (whether auto-update is on or not). Never fails loudly: an update
/// that can't start here is still there for the player to start.
pub async fn check<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) {
    if let Err(e) = try_check(state, app, id).await {
        tracing::info!(instance = %id, error = %e, "couldn't update by itself");
    }
}

async fn try_check<R: Runtime>(state: &AppState, app: &AppHandle<R>, id: &str) -> AppResult<()> {
    if state.jobs.is_busy(id) {
        return Ok(());
    }
    let Some(row) = db::instances::get(&state.db, id).await? else {
        return Ok(());
    };
    let Some(info) = super::sync::info(state, &row).await? else {
        return Ok(());
    };
    let needs = match info.state {
        SyncState::Conflict => Some(Needs::TheirChanges),
        SyncState::UpdateAvailable => needs(&super::sync::preview(state, id).await?),
        _ => return Ok(()),
    };
    match needs {
        None if info.auto_update => {
            tracing::info!(instance = %row.name, to = info.latest_revision, "updating by itself");
            // The job waits for the game to close and backs up first. New optional mods
            // aren't installed: they're the player's choice.
            super::sync::sync(state, app, id, SyncOptions::default()).await
        }
        None => Ok(()),
        Some(needs) => {
            tracing::info!(instance = %row.name, ?needs, "this update needs the player");
            if let Some(body) = message(&needs) {
                tell(state, app, &row.id, &row.name, info.latest_revision, body).await;
            }
            Ok(())
        }
    }
}

/// Why an update can't install by itself.
#[derive(Debug, PartialEq)]
enum Needs {
    /// Uploaded mods or resource packs, to approve.
    Approval(Vec<String>),
    /// It would undo the player's own changes.
    TheirChanges,
    /// It changes the launcher's Minecraft or loader version: left for the player to
    /// start, without a notice.
    Versions,
}

fn needs(p: &SyncPreview) -> Option<Needs> {
    if !p.unverified.is_empty() {
        Some(Needs::Approval(p.unverified.clone()))
    } else if !p.config_conflicts.is_empty() || !p.overwritten_local_changes.is_empty() {
        Some(Needs::TheirChanges)
    } else if p.launcher_change.is_some() {
        Some(Needs::Versions)
    } else {
        None
    }
}

/// What the notice says, for the reasons that get one.
fn message(needs: &Needs) -> Option<String> {
    match needs {
        Needs::Approval(names) => Some(format!(
            "It adds {}, which {} from Modrinth or CurseForge. Review it before it installs.",
            names.join(", "),
            if names.len() == 1 { "isn't" } else { "aren't" }
        )),
        Needs::TheirChanges => {
            Some("It would undo your own changes. Choose yours or the new version.".to_owned())
        }
        Needs::Versions => None,
    }
}

/// Once per version, even across restarts.
async fn tell<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    name: &str,
    revision: u32,
    body: String,
) {
    let key = format!("update_noticed {id}");
    let seen: Option<u32> = db::settings::get(&state.db, &key).await.ok().flatten();
    if seen == Some(revision) {
        return;
    }
    if let Err(e) = db::settings::set(&state.db, &key, &revision).await {
        tracing::warn!(error = %e, "couldn't save that an update was noticed");
    }
    let notice = Notice {
        title: format!("{name} has an update that needs you"),
        body,
        target: NoticeTarget::Review {
            instance_id: id.to_owned(),
        },
    };
    crate::desktop::notice(app, id, notice);
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
        assert_eq!(needs(&preview()), None, "a running game only delays it");
        let mut p = preview();
        p.unverified.push("custom-mod".into());
        assert_eq!(needs(&p), Some(Needs::Approval(vec!["custom-mod".into()])));
        let mut p = preview();
        p.overwritten_local_changes.push("Sodium".into());
        assert_eq!(needs(&p), Some(Needs::TheirChanges));
        let mut p = preview();
        p.config_conflicts.push(ConfigConflict {
            path: "options.txt".into(),
            mine: "a".into(),
            theirs: "b".into(),
        });
        assert_eq!(needs(&p), Some(Needs::TheirChanges));
    }

    #[test]
    fn notices_say_what_needs_the_player() {
        assert_eq!(
            message(&Needs::Approval(vec!["custom-mod".into()])).unwrap(),
            "It adds custom-mod, which isn't from Modrinth or CurseForge. Review it before it installs."
        );
        assert!(
            message(&Needs::Approval(vec!["a".into(), "b".into()]))
                .unwrap()
                .contains("a, b, which aren't")
        );
        assert_eq!(
            message(&Needs::Versions),
            None,
            "no notice for a version change"
        );
    }
}
