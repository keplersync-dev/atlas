//! Atlas desktop backend. The contract with the UI:
//! src/lib/types.ts (shapes) and src/lib/api/api.ts (commands).

pub mod cloud;
pub mod commands;
pub mod db;
pub mod dto;
pub mod error;
pub mod events;
pub mod game;
pub mod jobs;
pub mod launchers;
pub mod links;
pub mod resolve;
pub mod scan;
pub mod services;
pub mod state;
pub mod sync;
pub mod util;

use tauri::{Manager, Runtime};
use tauri_plugin_deep_link::DeepLinkExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::state::{AppPaths, AppState};

/// Every command the UI can call. Public so tests can build a mock app with the same
/// handler. Keep it in the order of src/lib/api/api.ts.
pub fn invoke_handler<R: Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static
{
    tauri::generate_handler![
        // Session & account
        commands::session::get_session,
        commands::session::sign_in,
        commands::session::sign_out,
        commands::session::link_identity,
        commands::session::unlink_identity,
        commands::session::update_profile,
        commands::session::reset_friend_code,
        commands::session::delete_account,
        // Launchers
        commands::launchers::list_launchers,
        commands::launchers::detect_launchers,
        commands::launchers::add_launcher,
        commands::launchers::remove_launcher,
        commands::launchers::list_detected_instances,
        // Instances
        commands::instances::list_instances,
        commands::instances::get_instance,
        commands::instances::track_instances,
        commands::instances::track_folder,
        commands::instances::untrack_instance,
        commands::instances::update_instance,
        commands::instances::rescan_instance,
        commands::instances::open_in_launcher,
        commands::instances::close_launcher,
        commands::instances::skip_launcher_picture,
        commands::instances::open_instance_folder,
        commands::instances::set_content_enabled,
        commands::instances::restore_backup,
        commands::instances::set_share_mode,
        commands::instances::set_optional_installed,
        commands::instances::dev_link_manifest,
        commands::instances::dev_unlink_manifest,
        // Sharing
        commands::sharing::list_shared_instances,
        commands::sharing::get_shared_instance,
        commands::sharing::accept_invite,
        commands::sharing::accept_invite_into,
        commands::sharing::decline_invite,
        commands::sharing::prepare_publish,
        commands::sharing::share_instance,
        commands::sharing::publish_instance,
        commands::sharing::preview_sync,
        commands::sharing::sync_instance,
        commands::sharing::revert_local_changes,
        commands::sharing::update_shared_instance,
        commands::sharing::invite_to_shared_instance,
        commands::sharing::set_shared_member_role,
        commands::sharing::remove_shared_member,
        commands::sharing::unshare_from_group,
        commands::sharing::transfer_shared_instance,
        commands::sharing::leave_shared_instance,
        commands::sharing::archive_shared_instance,
        // Friends
        commands::friends::list_friends,
        commands::friends::list_friend_requests,
        commands::friends::preview_friend_code,
        commands::friends::send_friend_request,
        commands::friends::respond_to_friend_request,
        commands::friends::cancel_friend_request,
        commands::friends::remove_friend,
        commands::friends::block_user,
        commands::friends::take_opened_link,
        // Groups
        commands::groups::list_groups,
        commands::groups::get_group,
        commands::groups::create_group,
        commands::groups::update_group,
        commands::groups::delete_group,
        commands::groups::create_group_invite,
        commands::groups::revoke_group_invite,
        commands::groups::preview_group_invite,
        commands::groups::join_group,
        commands::groups::set_group_member_role,
        commands::groups::remove_group_member,
        commands::groups::transfer_group,
        // Activity
        commands::activity::list_activity,
        commands::activity::mark_activity_read,
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

/// Startup. Must not wait on the network.
fn setup<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let mut root = app.path().app_data_dir()?;
    let profile = state::dev_profile();
    if let Some(profile) = &profile {
        root = root.join("profiles").join(profile);
    }
    let paths = AppPaths::new(&root);
    paths.create_all()?;
    app.manage(init_logging(&paths));
    tracing::info!(root = %paths.root.display(), "starting Atlas");
    if let Some(profile) = &profile {
        // The asset protocol only serves $APPDATA/cache/icons; let this profile's in too.
        app.asset_protocol_scope()
            .allow_directory(&paths.icons, false)?;
        if let Some(window) = app.get_webview_window("main") {
            window.set_title(&format!("Atlas ({profile})"))?;
        }
    }

    let db = tauri::async_runtime::block_on(db::open(&paths.database()))?;
    let state = AppState::new(db, paths);
    app.manage(state.clone());

    // Friend and group links. A development build registers the scheme for this exe (an
    // installed Atlas registers it when installed); a profile copy leaves it to the main one.
    #[cfg(debug_assertions)]
    if profile.is_none()
        && let Err(e) = app.deep_link().register_all()
    {
        tracing::warn!(error = %e, "couldn't register atlas:// links");
    }
    {
        let handle = app.handle().clone();
        app.deep_link().on_open_url(move |event| {
            for url in event.urls() {
                links::open(&handle, url.as_str());
            }
        });
    }
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        for url in urls {
            links::open(app.handle(), url.as_str());
        }
    }

    let handle = app.handle().clone();

    // The page shows the window once it has drawn (it starts hidden). If the page never
    // loads, show it anyway rather than leave Atlas invisible.
    {
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            if let Some(window) = handle.get_webview_window("main")
                && !window.is_visible().unwrap_or(true)
            {
                tracing::warn!("the page didn't show the window; showing it");
                let _ = window.show();
            }
        });
    }

    // Development builds load the page from the dev server, which may still be starting
    // (a second copy opened while `pnpm tauri dev` starts up): load it again once it answers.
    #[cfg(debug_assertions)]
    if let Some(url) = app.config().build.dev_url.clone() {
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            let (Some(host), Some(port)) = (url.host_str().map(str::to_owned), url.port()) else {
                return;
            };
            let up = || tokio::net::TcpStream::connect((host.clone(), port));
            if up().await.is_ok() {
                return;
            }
            tracing::info!(%url, "waiting for the dev server");
            for _ in 0..300 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if up().await.is_ok() {
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.navigate(url);
                    }
                    return;
                }
            }
        });
    }

    // An update interrupted by a crash or power cut is undone before anything else.
    let recovered = tauri::async_runtime::block_on(sync::recovery::run(&state));
    if !recovered.is_empty() {
        tracing::warn!(count = recovered.len(), "undid interrupted updates");
    }

    // Rescan an instance whenever the watcher sees its content folders change.
    if let Some(mut changes) = state.watcher.take_receiver() {
        let (state, handle) = (state.clone(), handle.clone());
        tauri::async_runtime::spawn(async move {
            while let Some(id) = changes.recv().await {
                if let Err(e) = services::instances::rescan(&state, &handle, &id).await {
                    tracing::warn!(instance = %id, error = %e, "rescan after change failed");
                }
            }
        });
    }

    game::spawn(state.clone(), handle.clone());

    // Session, connection and friend events go to the UI; the saved session is checked
    // and the server socket opened in the background.
    {
        let handle = handle.clone();
        state.cloud.on_event(move |e| events::emit(&handle, e));
        let state = state.clone();
        tauri::async_runtime::spawn(async move {
            state.cloud.restore().await;
            services::presence::refresh(&state).await;
            state.cloud.start_realtime();
        });
    }

    // Background work that mustn't hold up the window: find launchers, then rescan
    // every tracked instance (metadata and files; this also starts watching them).
    tauri::async_runtime::spawn(async move {
        if let Err(e) = services::launchers::ensure_detected(&state).await {
            tracing::warn!(error = %e, "launcher detection failed");
        }
        services::instances::rescan_all(&state, &handle, std::time::Duration::ZERO).await;
    });
    Ok(())
}

/// Coming back to Atlas: catch changes the watcher can't see (launcher metadata),
/// for instances not scanned in the last 30 seconds.
fn on_window_event<R: Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::Focused(true) = event {
        let app = window.app_handle().clone();
        tauri::async_runtime::spawn(async move {
            let Some(state) = app.try_state::<AppState>().map(|s| s.inner().clone()) else {
                return;
            };
            services::instances::rescan_all(&state, &app, std::time::Duration::from_secs(30)).await;
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    // One Atlas at a time: opening Atlas or a link again goes to the copy that's running
    // (the deep-link feature passes links along). A development profile copy runs
    // alongside instead.
    if state::dev_profile().is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            links::show_window(app);
        }));
    }
    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .setup(setup)
        .on_window_event(on_window_event)
        .invoke_handler(invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
