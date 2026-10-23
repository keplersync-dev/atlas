//! `instance.work` events: what Atlas is doing to an instance outside an update, so a long
//! scan or share doesn't look like Atlas froze.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Runtime};

use crate::dto::{Work, WorkStage};
use crate::events::{AtlasEvent, emit};

/// Updates go out at most this often; the last one of a stage always does.
const EVERY: Duration = Duration::from_millis(100);

/// Reports one instance's work. When dropped (finished, failed or cancelled) it tells
/// the UI the work is over, if it reported any.
pub struct Reporter<R: Runtime> {
    app: AppHandle<R>,
    instance_id: String,
    last: Mutex<Option<Instant>>,
    reported: AtomicBool,
}

impl<R: Runtime> Reporter<R> {
    pub fn new(app: &AppHandle<R>, instance_id: &str) -> Self {
        Self {
            app: app.clone(),
            instance_id: instance_id.to_owned(),
            last: Mutex::new(None),
            reported: AtomicBool::new(false),
        }
    }

    pub fn report(&self, stage: WorkStage, done: u64, total: u64) {
        {
            let mut last = self.last.lock().unwrap();
            let due = last.is_none_or(|t| t.elapsed() >= EVERY) || done >= total;
            if !due {
                return;
            }
            *last = Some(Instant::now());
        }
        self.reported.store(true, Ordering::Relaxed);
        self.send(Some(Work { stage, done, total }));
    }

    fn send(&self, work: Option<Work>) {
        emit(
            &self.app,
            AtlasEvent::InstanceWork {
                instance_id: self.instance_id.clone(),
                work,
            },
        );
    }
}

impl<R: Runtime> Drop for Reporter<R> {
    fn drop(&mut self) {
        if self.reported.load(Ordering::Relaxed) {
            self.send(None);
        }
    }
}
