//! Updates from the GitHub release page, through tauri-plugin-updater. The UI checks at
//! start-up and every few hours and offers a restart. When the server refuses this version
//! (`client_outdated`), the UI insists. Development builds never update themselves.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_updater::UpdaterExt;

use crate::dto::AvailableUpdate;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::{self, AtlasEvent};

/// The newest release, if it's newer than this one.
pub async fn check<R: Runtime>(app: &AppHandle<R>) -> AppResult<Option<AvailableUpdate>> {
    if cfg!(debug_assertions) {
        return Ok(None);
    }
    let update = find(app).await?;
    Ok(update.map(|u| AvailableUpdate {
        version: u.version,
        notes: u.body,
    }))
}

/// Download and install the newest release, then restart into it. On Windows the
/// installer runs on its own and Atlas quits while it does.
pub async fn install<R: Runtime>(app: &AppHandle<R>) -> AppResult<()> {
    if cfg!(debug_assertions) {
        return Err(AppError::unknown(
            "Development builds don't update themselves",
        ));
    }
    let update = find(app)
        .await?
        .ok_or_else(|| AppError::not_found("Atlas is already up to date"))?;
    let handle = app.clone();
    let mut downloaded = 0u64;
    let mut last = Instant::now();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                if last.elapsed() >= Duration::from_millis(100) {
                    last = Instant::now();
                    events::emit(&handle, AtlasEvent::UpdateProgress { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "couldn't install the update");
            AppError::unknown("The update didn't install. Try again.")
        })?;
    app.restart()
}

/// Where releases are published, for downloading one by hand.
const RELEASE_PAGE: &str = "https://github.com/keplersync-dev/atlas/releases/latest";

pub fn open_release_page<R: Runtime>(app: &AppHandle<R>) -> AppResult<()> {
    let opener = app
        .try_state::<tauri_plugin_opener::Opener<R>>()
        .ok_or_else(|| AppError::unknown("Couldn't open your browser"))?;
    opener.open_url(RELEASE_PAGE, None::<&str>).map_err(|e| {
        tracing::error!(error = %e, "couldn't open the browser");
        AppError::unknown("Couldn't open your browser")
    })
}

async fn find<R: Runtime>(app: &AppHandle<R>) -> AppResult<Option<tauri_plugin_updater::Update>> {
    let updater = app.updater().map_err(|e| {
        tracing::error!(error = %e, "the updater isn't set up");
        AppError::unknown("Couldn't check for updates")
    })?;
    updater.check().await.map_err(|e| {
        tracing::info!(error = %e, "couldn't check for updates");
        AppError::new(
            ErrorCode::Offline,
            "Can't check for updates right now. Try again in a little while.",
        )
    })
}
