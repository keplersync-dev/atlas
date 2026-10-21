//! Atlas desktop backend. Plan: docs/tauri-plan.md. The contract with the UI:
//! src/lib/types.ts (shapes) and src/lib/api/api.ts (commands).

pub mod commands;
pub mod db;
pub mod dto;
pub mod error;
pub mod events;
pub mod state;
pub mod util;

use tauri::{Manager, Runtime};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::state::{AppPaths, AppState};

/// Every command the UI can call. Public so tests can build a mock app with the same
/// handler. Keep it in the order of src/lib/api/api.ts.
pub fn invoke_handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static
{
    use commands::*;
    tauri::generate_handler![
        // Session & account
        session::get_session,
        session::sign_in,
        session::sign_out,
        session::link_identity,
        session::unlink_identity,
        session::update_profile,
        session::reset_friend_code,
        session::delete_account,
        // Launchers
        launchers::list_launchers,
        launchers::detect_launchers,
        launchers::add_launcher,
        launchers::remove_launcher,
        launchers::list_detected_instances,
        // Instances
        instances::list_instances,
        instances::get_instance,
        instances::track_instances,
        instances::track_folder,
        instances::untrack_instance,
        instances::update_instance,
        instances::rescan_instance,
        instances::open_in_launcher,
        instances::open_instance_folder,
        instances::set_content_enabled,
        instances::restore_backup,
        instances::set_share_mode,
        instances::set_optional_installed,
        // Sharing
        sharing::list_shared_instances,
        sharing::get_shared_instance,
        sharing::accept_invite,
        sharing::decline_invite,
        sharing::prepare_publish,
        sharing::share_instance,
        sharing::publish_instance,
        sharing::preview_sync,
        sharing::sync_instance,
        sharing::revert_local_changes,
        sharing::update_shared_instance,
        sharing::invite_to_shared_instance,
        sharing::set_shared_member_role,
        sharing::remove_shared_member,
        sharing::unshare_from_group,
        sharing::transfer_shared_instance,
        sharing::leave_shared_instance,
        sharing::archive_shared_instance,
        // Friends
        friends::list_friends,
        friends::list_friend_requests,
        friends::preview_friend_code,
        friends::send_friend_request,
        friends::respond_to_friend_request,
        friends::cancel_friend_request,
        friends::remove_friend,
        friends::block_user,
        // Groups
        groups::list_groups,
        groups::get_group,
        groups::create_group,
        groups::update_group,
        groups::delete_group,
        groups::create_group_invite,
        groups::revoke_group_invite,
        groups::preview_group_invite,
        groups::join_group,
        groups::set_group_member_role,
        groups::remove_group_member,
        groups::transfer_group,
        // Activity
        activity::list_activity,
        activity::mark_activity_read,
    ]
}

/// Keeps the log file writer flushing until the app exits.
struct LogGuard(#[allow(dead_code)] tracing_appender::non_blocking::WorkerGuard);

/// Logs go to <app data>/logs/atlas.log.<date> (kept by date), and to the terminal in dev.
fn init_logging(paths: &AppPaths) -> LogGuard {
    let file = tracing_appender::rolling::daily(&paths.logs, "atlas.log");
    let (file, guard) = tracing_appender::non_blocking(file);
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,atlas_lib=debug,sqlx=warn"));
    let terminal = cfg!(debug_assertions).then(fmt::layer);
    // try_init: a second call (tests) is harmless.
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_ansi(false).with_writer(file))
        .with(terminal)
        .try_init();
    LogGuard(guard)
}

/// Startup (docs/tauri-plan.md §4.3). Must not wait on the network.
fn setup<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let paths = AppPaths::new(&app.path().app_data_dir()?);
    paths.create_all()?;
    app.manage(init_logging(&paths));
    tracing::info!(root = %paths.root.display(), "starting Atlas");

    let db = tauri::async_runtime::block_on(db::open(&paths.database()))?;
    app.manage(AppState { db, paths });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(setup)
        .invoke_handler(invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
