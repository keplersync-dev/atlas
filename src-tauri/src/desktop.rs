//! Atlas around its window on Windows: the tray icon, closing to it, notifications for
//! new activity, and starting with Windows (in the tray, window closed).

use std::sync::atomic::{AtomicBool, Ordering};

use sqlx::SqlitePool;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::AutoLaunchManager;
use tauri_plugin_notification::NotificationExt as _;

use crate::db;
use crate::dto::{ActivityItem, ActivityType, AppSettings, AppSettingsPatch};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// What Windows passes Atlas when it starts it at sign-in: stay in the tray.
pub const STARTED_BY_WINDOWS: &str = "--hidden";

const NOTIFICATIONS: &str = "notifications";
const CLOSE_TO_TRAY: &str = "close_to_tray";
/// Set once the player has been told that closing the window leaves Atlas in the tray.
const TRAY_HINT_SHOWN: &str = "tray_hint_shown";

pub struct Prefs {
    pub notifications: AtomicBool,
    pub close_to_tray: AtomicBool,
    tray_hint_shown: AtomicBool,
    /// Started with Windows: the window stays closed until the player opens it.
    pub started_hidden: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            notifications: AtomicBool::new(true),
            close_to_tray: AtomicBool::new(true),
            tray_hint_shown: AtomicBool::new(false),
            started_hidden: std::env::args().any(|a| a == STARTED_BY_WINDOWS),
        }
    }
}

impl Prefs {
    /// The saved settings, read once at startup.
    pub async fn load(&self, db: &SqlitePool) -> AppResult<()> {
        let notifications: bool = db::settings::get_or(db, NOTIFICATIONS, true).await?;
        let close_to_tray: bool = db::settings::get_or(db, CLOSE_TO_TRAY, true).await?;
        let hint_shown: bool = db::settings::get_or(db, TRAY_HINT_SHOWN, false).await?;
        self.notifications.store(notifications, Ordering::Relaxed);
        self.close_to_tray.store(close_to_tray, Ordering::Relaxed);
        self.tray_hint_shown.store(hint_shown, Ordering::Relaxed);
        Ok(())
    }
}

/// The page has drawn: show the window, unless Atlas started with Windows.
pub fn window_ready<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    if !state.prefs.started_hidden {
        crate::links::show_window(app);
    }
}

/// The tray icon: clicking it opens Atlas; its menu opens or quits it.
pub fn add_tray<R: Runtime>(app: &AppHandle<R>, tooltip: &str) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Atlas", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Atlas", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip(tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => crate::links::show_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::links::show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// The window's close button: with close-to-tray on, hide instead, so Atlas keeps
/// updating instances and showing presence. The first time, say where it went.
/// True when the close became a hide.
pub fn close_to_tray<R: Runtime>(window: &tauri::Window<R>) -> bool {
    let Some(state) = window.app_handle().try_state::<AppState>() else {
        return false;
    };
    if !state.prefs.close_to_tray.load(Ordering::Relaxed) {
        return false;
    }
    let _ = window.hide();
    if !state.prefs.tray_hint_shown.swap(true, Ordering::Relaxed) {
        notify(
            window.app_handle(),
            "Atlas is still running",
            "It keeps your shared instances up to date. Quit it from the tray icon.",
        );
        let db = state.db.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = db::settings::set(&db, TRAY_HINT_SHOWN, &true).await {
                tracing::warn!(error = %e, "couldn't save that the tray hint was shown");
            }
        });
    }
    true
}

/// New activity from a friend: a Windows notification, when they're on and Atlas isn't
/// in front (its activity feed shows it then).
pub fn notify_activity<R: Runtime>(app: &AppHandle<R>, item: &ActivityItem) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if !state.prefs.notifications.load(Ordering::Relaxed) {
        return;
    }
    let in_front = app
        .get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if !in_front {
        // Worded for the signed-in user ("You now own …"), whose id is in the saved session.
        let (app, item) = (app.clone(), item.clone());
        tauri::async_runtime::spawn(async move {
            let me = match app.try_state::<AppState>() {
                Some(state) => state.cloud.session().await.ok().and_then(|s| s.user),
                None => None,
            };
            let (title, body) = describe(&item, me.as_ref().map(|m| m.user.id.as_str()));
            notify(&app, &title, &body);
        });
    }
}

fn notify<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!(error = %e, "couldn't show a notification");
    }
}

/// A notification's title and text, worded like the activity feed. `me` is the signed-in
/// user's id.
fn describe(a: &ActivityItem, me: Option<&str>) -> (String, String) {
    let who = a
        .actor
        .as_ref()
        .map(|u| u.display_name.as_str())
        .unwrap_or("Someone");
    let what = a.shared_instance_name.as_deref().unwrap_or("an instance");
    let group = a.group_name.as_deref().unwrap_or("a group");
    let (title, body) = match a.kind {
        ActivityType::FriendRequest => ("Friend request", format!("{who} wants to be friends")),
        ActivityType::FriendAccepted => {
            ("New friend", format!("{who} accepted your friend request"))
        }
        ActivityType::InstanceInvite => {
            ("Shared with you", format!("{who} shared {what} with you"))
        }
        ActivityType::InstanceUpdated => (
            "Update available",
            match a.revision {
                Some(n) => format!("{who} published v{n} of {what}"),
                None => format!("{who} updated {what}"),
            },
        ),
        ActivityType::InstanceSharedToGroup => (
            "Shared with a group",
            format!("{who} shared {what} with {group}"),
        ),
        ActivityType::MemberJoined => ("Instance added", format!("{who} added {what}")),
        ActivityType::MemberLeft => ("Instance left", format!("{who} left {what}")),
        ActivityType::OwnershipTransferred => (
            "New owner",
            if a.actor.as_ref().map(|u| u.id.as_str()) == me && me.is_some() {
                format!("You now own {what}")
            } else {
                format!("{who} now owns {what}")
            },
        ),
        ActivityType::InstanceFrozen => (
            "Instance frozen",
            format!("{what} was frozen: the owner left"),
        ),
        ActivityType::GroupJoined => ("New group member", format!("{who} joined {group}")),
    };
    (title.to_owned(), body)
}

/// The tray and startup settings. Starting with Windows is Windows' own list of apps to
/// run at sign-in, so it's read from there rather than saved by Atlas.
pub fn settings<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> AppSettings {
    AppSettings {
        notifications: state.prefs.notifications.load(Ordering::Relaxed),
        close_to_tray: state.prefs.close_to_tray.load(Ordering::Relaxed),
        start_with_windows: app
            .try_state::<AutoLaunchManager>()
            .is_some_and(|m| m.is_enabled().unwrap_or(false)),
    }
}

pub async fn update_settings<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    patch: AppSettingsPatch,
) -> AppResult<AppSettings> {
    if let Some(on) = patch.notifications {
        db::settings::set(&state.db, NOTIFICATIONS, &on).await?;
        state.prefs.notifications.store(on, Ordering::Relaxed);
    }
    if let Some(on) = patch.close_to_tray {
        db::settings::set(&state.db, CLOSE_TO_TRAY, &on).await?;
        state.prefs.close_to_tray.store(on, Ordering::Relaxed);
    }
    if let Some(on) = patch.start_with_windows {
        let manager = app
            .try_state::<AutoLaunchManager>()
            .ok_or_else(|| AppError::unknown("Starting with Windows isn't available here"))?;
        let done = if on {
            manager.enable()
        } else {
            manager.disable()
        };
        done.map_err(|e| {
            tracing::warn!(error = %e, "couldn't change starting with Windows");
            AppError::unknown("Windows didn't let Atlas change that. Try again.")
        })?;
    }
    Ok(settings(app, state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::PublicUser;

    fn item(kind: ActivityType) -> ActivityItem {
        ActivityItem {
            id: "a".into(),
            kind,
            actor: Some(PublicUser {
                id: "u".into(),
                username: "dakep".into(),
                display_name: "dakep".into(),
                skin_url: None,
                avatar_url: None,
                minecraft_uuid: None,
                minecraft_username: None,
            }),
            created_at: "2026-10-24T00:00:00Z".into(),
            read_at: None,
            shared_instance_id: Some("s".into()),
            shared_instance_name: Some("SkyFactory 4 fixed".into()),
            group_id: None,
            group_name: None,
            revision: Some(4),
        }
    }

    #[test]
    fn notifications_read_like_the_feed() {
        assert_eq!(
            describe(&item(ActivityType::InstanceUpdated), None),
            (
                "Update available".to_owned(),
                "dakep published v4 of SkyFactory 4 fixed".to_owned()
            )
        );
        assert_eq!(
            describe(&item(ActivityType::InstanceInvite), None).1,
            "dakep shared SkyFactory 4 fixed with you"
        );
        let mut frozen = item(ActivityType::InstanceFrozen);
        frozen.actor = None;
        assert_eq!(
            describe(&frozen, None).1,
            "SkyFactory 4 fixed was frozen: the owner left"
        );
        let owner = item(ActivityType::OwnershipTransferred);
        assert_eq!(
            describe(&owner, Some("x")).1,
            "dakep now owns SkyFactory 4 fixed"
        );
        assert_eq!(
            describe(&owner, Some("u")).1,
            "You now own SkyFactory 4 fixed",
            "to the new owner themselves"
        );
    }
}
