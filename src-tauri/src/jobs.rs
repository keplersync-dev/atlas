//! Long work on an instance folder (update, accept, restore, revert, optional download),
//! one job per instance at a time.
//!
//! A job can be cancelled while it waits or downloads, never once files are moving: jobs
//! wait through [`Progress::sleep`] and [`Progress::wait_for_game_exit`], and check
//! [`Progress::check`] before they change anything. A cancelled job ends without
//! `instance.jobFailed`.
//!
//! The job manager owns the runtime-only parts of `InstanceSummary`: `status: "syncing"`
//! and `progress`. Every job ends the same way, which the UI relies on (Review.svelte
//! waits for "syncing" and then "ready"; the failure must already be known by then):
//!
//!   instance.jobFailed { message }                           only if it failed
//!   instance.progress { status: "ready", progress: null }   (or "missing")
//!   instance.changed

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Runtime};
use tokio::sync::watch;

use crate::db;
use crate::dto::{InstanceStatus, ManualDownload, SyncProgress, SyncStage};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;

struct Slot {
    /// None until the first stage is reported.
    progress: Option<SyncProgress>,
    cancel: watch::Sender<bool>,
}

/// Instance id → the job running on it.
#[derive(Clone, Default)]
pub struct Jobs(Arc<Mutex<HashMap<String, Slot>>>);

impl Jobs {
    pub fn is_busy(&self, instance_id: &str) -> bool {
        self.0.lock().unwrap().contains_key(instance_id)
    }
    /// Some(progress) while a job runs.
    pub fn status(&self, instance_id: &str) -> Option<Option<SyncProgress>> {
        self.0
            .lock()
            .unwrap()
            .get(instance_id)
            .map(|s| s.progress.clone())
    }
    /// Ask the instance's job to stop. False if there's none.
    pub fn cancel(&self, instance_id: &str) -> bool {
        match self.0.lock().unwrap().get(instance_id) {
            Some(slot) => {
                slot.cancel.send_replace(true);
                true
            }
            None => false,
        }
    }
    fn claim(&self, instance_id: &str) -> Option<watch::Receiver<bool>> {
        let mut jobs = self.0.lock().unwrap();
        if jobs.contains_key(instance_id) {
            return None;
        }
        let (cancel, cancelled) = watch::channel(false);
        jobs.insert(
            instance_id.to_string(),
            Slot {
                progress: None,
                cancel,
            },
        );
        Some(cancelled)
    }
    fn set(&self, instance_id: &str, p: SyncProgress) {
        if let Some(slot) = self.0.lock().unwrap().get_mut(instance_id) {
            slot.progress = Some(p);
        }
    }
    /// Remove the job. True if it had been cancelled.
    fn release(&self, instance_id: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .remove(instance_id)
            .is_some_and(|s| *s.cancel.borrow())
    }
}

/// What a job returns when it stopped because it was cancelled.
pub fn cancelled() -> AppError {
    AppError::invalid_input("Cancelled")
}

/// Handed to a job to report what it's doing.
pub struct Progress<R: Runtime> {
    pub app: AppHandle<R>,
    jobs: Jobs,
    pub instance_id: String,
    current: Mutex<(SyncProgress, Instant)>,
    cancel: watch::Receiver<bool>,
}

impl<R: Runtime> Progress<R> {
    fn new(
        app: AppHandle<R>,
        jobs: Jobs,
        instance_id: String,
        cancel: watch::Receiver<bool>,
    ) -> Self {
        let p = SyncProgress {
            stage: SyncStage::Backup,
            percent: 0,
            bytes_done: 0,
            bytes_total: 0,
            manual_downloads: vec![],
        };
        Self {
            app,
            jobs,
            instance_id,
            current: Mutex::new((p, Instant::now() - Duration::from_secs(1))),
            cancel,
        }
    }

    /// Err once the job has been cancelled. Jobs check it before they change files.
    pub fn check(&self) -> AppResult<()> {
        if *self.cancel.borrow() {
            Err(cancelled())
        } else {
            Ok(())
        }
    }

    /// Resolves when the job is cancelled.
    pub async fn cancelled(&self) {
        let mut cancel = self.cancel.clone();
        let _ = cancel.wait_for(|c| *c).await;
    }

    /// Wait, unless the job is cancelled meanwhile.
    pub async fn sleep(&self, how_long: Duration) -> AppResult<()> {
        tokio::select! {
            _ = tokio::time::sleep(how_long) => Ok(()),
            _ = self.cancelled() => Err(cancelled()),
        }
    }

    /// Never write under a running game: wait until it's closed (or the job is cancelled).
    pub async fn wait_for_game_exit(&self, state: &AppState) -> AppResult<()> {
        if !state.is_running(&self.instance_id) {
            return self.check();
        }
        self.stage(SyncStage::WaitingForGameExit);
        while state.is_running(&self.instance_id) {
            self.sleep(Duration::from_secs(2)).await?;
        }
        self.check()
    }

    /// Move to a stage (always sent right away).
    pub fn stage(&self, stage: SyncStage) {
        self.update(true, |p| {
            p.stage = stage;
            p.percent = 0;
            p.bytes_done = 0;
            p.bytes_total = 0;
            p.manual_downloads.clear();
        });
    }

    /// Byte progress within the current stage (sent at most 5 times a second).
    pub fn bytes(&self, done: u64, total: u64) {
        let force = done >= total;
        self.update(force, |p| {
            p.bytes_done = done;
            p.bytes_total = total;
            p.percent = (done * 100)
                .checked_div(total)
                .map_or(100, |pct| pct.min(100) as u8);
        });
    }

    /// Files the user has to download by hand (stage waiting_for_manual_downloads).
    pub fn manual(&self, list: Vec<ManualDownload>) {
        self.update(true, |p| {
            p.stage = SyncStage::WaitingForManualDownloads;
            p.manual_downloads = list;
        });
    }

    fn update(&self, force: bool, change: impl FnOnce(&mut SyncProgress)) {
        let mut cur = self.current.lock().unwrap();
        change(&mut cur.0);
        self.jobs.set(&self.instance_id, cur.0.clone());
        if force || cur.1.elapsed() >= Duration::from_millis(200) {
            cur.1 = Instant::now();
            emit(
                &self.app,
                AtlasEvent::InstanceProgress {
                    instance_id: self.instance_id.clone(),
                    status: InstanceStatus::Syncing,
                    progress: Some(cur.0.clone()),
                },
            );
        }
    }
}

/// Start `work` in the background as this instance's job. Returns right away;
/// Err(conflict) if the instance already has one.
pub fn spawn<R, F, Fut>(
    state: &AppState,
    app: &AppHandle<R>,
    instance_id: &str,
    work: F,
) -> AppResult<()>
where
    R: Runtime,
    F: FnOnce(Progress<R>) -> Fut + Send + 'static,
    Fut: Future<Output = AppResult<()>> + Send + 'static,
{
    let progress = begin(state, app, instance_id)?;
    let (state, app, id) = (state.clone(), app.clone(), instance_id.to_string());
    tauri::async_runtime::spawn(async move {
        let result = work(progress).await;
        finish(&state, &app, &id, &result, true).await;
    });
    Ok(())
}

/// Run `work` as this instance's job and wait for it (the caller gets the error itself).
pub async fn run<R, F, Fut>(
    state: &AppState,
    app: &AppHandle<R>,
    instance_id: &str,
    work: F,
) -> AppResult<()>
where
    R: Runtime,
    F: FnOnce(Progress<R>) -> Fut,
    Fut: Future<Output = AppResult<()>>,
{
    let progress = begin(state, app, instance_id)?;
    let result = work(progress).await;
    finish(state, app, instance_id, &result, false).await;
    result
}

fn begin<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    instance_id: &str,
) -> AppResult<Progress<R>> {
    let Some(cancel) = state.jobs.claim(instance_id) else {
        return Err(AppError::conflict("Wait for the current update to finish"));
    };
    emit(
        app,
        AtlasEvent::InstanceProgress {
            instance_id: instance_id.into(),
            status: InstanceStatus::Syncing,
            progress: None,
        },
    );
    Ok(Progress::new(
        app.clone(),
        state.jobs.clone(),
        instance_id.to_string(),
        cancel,
    ))
}

async fn finish<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    result: &AppResult<()>,
    report_failure: bool,
) {
    let was_cancelled = state.jobs.release(id);
    let status = match db::instances::get(&state.db, id).await {
        Ok(Some(row)) if !Path::new(&row.game_dir).is_dir() => InstanceStatus::Missing,
        _ => InstanceStatus::Ready,
    };
    if let Err(e) = result {
        tracing::warn!(instance = %id, error = %e.message, "job failed");
        // Cancelling isn't a failure to tell anyone about.
        if report_failure && !was_cancelled {
            emit(
                app,
                AtlasEvent::InstanceJobFailed {
                    instance_id: id.into(),
                    message: e.message.clone(),
                },
            );
        }
    }
    emit(
        app,
        AtlasEvent::InstanceProgress {
            instance_id: id.into(),
            status,
            progress: None,
        },
    );
    emit(
        app,
        AtlasEvent::InstanceChanged {
            instance_id: id.into(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cancelled_job_is_told_and_ends_quietly() {
        let jobs = Jobs::default();
        assert!(!jobs.cancel("i1"), "nothing running");
        let cancel = jobs.claim("i1").unwrap();
        assert!(jobs.claim("i1").is_none(), "one job per instance");
        assert!(!*cancel.borrow());
        assert!(jobs.cancel("i1"));
        assert!(*cancel.borrow(), "the job sees it");
        assert!(jobs.release("i1"), "ends as cancelled");
        assert!(!jobs.is_busy("i1"));

        jobs.claim("i1").unwrap();
        assert!(!jobs.release("i1"), "a job that just finished");
    }
}
