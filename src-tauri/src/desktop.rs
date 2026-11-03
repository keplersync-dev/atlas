//! Atlas around its window on Windows: the tray icon, closing to it, notices when
//! something needs the player, and starting with Windows (in the tray, window closed).

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, UserAttentionType};
use tauri_plugin_autostart::AutoLaunchManager;
use tauri_plugin_notification::NotificationExt as _;

use crate::db;
use crate::dto::{ActivityItem, ActivityType, AppSettings, AppSettingsPatch, Notice, NoticeTarget};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;

/// What Windows passes Atlas when it starts it at sign-in: stay in the tray.
pub const STARTED_BY_WINDOWS: &str = "--hidden";

const NOTIFICATIONS: &str = "notifications";
const NOTIFICATION_SOUND: &str = "notification_sound";
const CLOSE_TO_TRAY: &str = "close_to_tray";
/// Set once the player has been told that closing the window leaves Atlas in the tray.
const TRAY_HINT_SHOWN: &str = "tray_hint_shown";
const AUTO_UPDATE: &str = "auto_update";
const START_WITH_WINDOWS_OFFERED: &str = "start_with_windows_offered";

/// At most one notice about the same pack or person in this long.
const NOTICE_GAP: Duration = Duration::from_secs(60 * 60);

pub struct Prefs {
    pub notifications: AtomicBool,
    notification_sound: AtomicBool,
    pub close_to_tray: AtomicBool,
    tray_hint_shown: AtomicBool,
    auto_update: AtomicBool,
    start_with_windows_offered: AtomicBool,
    /// Started with Windows: the window stays closed until the player opens it.
    pub started_hidden: bool,
    /// Notices that came while a game ran, shown when it closes.
    held: Mutex<Vec<Notice>>,
    /// When each pack or person last got a notice.
    last_notice: Mutex<HashMap<String, Instant>>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            notifications: AtomicBool::new(true),
            notification_sound: AtomicBool::new(true),
            close_to_tray: AtomicBool::new(true),
            tray_hint_shown: AtomicBool::new(false),
            auto_update: AtomicBool::new(true),
            start_with_windows_offered: AtomicBool::new(false),
            started_hidden: std::env::args().any(|a| a == STARTED_BY_WINDOWS),
            held: Mutex::default(),
            last_notice: Mutex::default(),
        }
    }
}

impl Prefs {
    /// The saved settings, read once at startup.
    pub async fn load(&self, db: &SqlitePool) -> AppResult<()> {
        let notifications: bool = db::settings::get_or(db, NOTIFICATIONS, true).await?;
        let sound: bool = db::settings::get_or(db, NOTIFICATION_SOUND, true).await?;
        self.notification_sound.store(sound, Ordering::Relaxed);
        let close_to_tray: bool = db::settings::get_or(db, CLOSE_TO_TRAY, true).await?;
        let hint_shown: bool = db::settings::get_or(db, TRAY_HINT_SHOWN, false).await?;
        let auto_update: bool = db::settings::get_or(db, AUTO_UPDATE, true).await?;
        let offered: bool = db::settings::get_or(db, START_WITH_WINDOWS_OFFERED, false).await?;
        self.notifications.store(notifications, Ordering::Relaxed);
        self.close_to_tray.store(close_to_tray, Ordering::Relaxed);
        self.tray_hint_shown.store(hint_shown, Ordering::Relaxed);
        self.auto_update.store(auto_update, Ordering::Relaxed);
        self.start_with_windows_offered
            .store(offered, Ordering::Relaxed);
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
    // The Mac's menu bar wants a black shape it can tint for light and dark menu bars, not the app icon.
    if cfg!(target_os = "macos") {
        tray = tray
            .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray-mac.png"))?)
            .icon_as_template(true);
    } else if let Some(icon) = app.default_window_icon() {
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
            if cfg!(target_os = "macos") {
                "It keeps your packs up to date. Quit it from the menu bar icon."
            } else {
                "It keeps your packs up to date. Quit it from the tray icon."
            },
            false,
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

/// A pack shared with the player, or a friend request: a notice. Other activity stays in
/// the feed.
pub fn notify_activity<R: Runtime>(app: &AppHandle<R>, item: &ActivityItem) {
    let (key, target) = match (item.kind, &item.shared_instance_id) {
        (ActivityType::FriendRequest, _) => (
            format!("friend {}", item.actor.as_ref().map_or(&item.id, |u| &u.id)),
            NoticeTarget::Friends,
        ),
        (ActivityType::InstanceInvite | ActivityType::InstanceSharedToGroup, Some(id)) => (
            id.clone(),
            NoticeTarget::Invite {
                shared_instance_id: id.clone(),
            },
        ),
        _ => return,
    };
    // Worded for the signed-in user, whose id is in the saved session.
    let (app, item) = (app.clone(), item.clone());
    tauri::async_runtime::spawn(async move {
        let me = match app.try_state::<AppState>() {
            Some(state) => state.cloud.session().await.ok().and_then(|s| s.user),
            None => None,
        };
        let (title, body) = describe(&item, me.as_ref().map(|m| m.user.id.as_str()));
        notice(
            &app,
            &key,
            Notice {
                title,
                body,
                target,
            },
        );
    });
}

/// An update is waiting on files the player has to download by hand: a notice, and the
/// taskbar button flashes, so they don't think it's done.
pub fn notify_manual_downloads<R: Runtime>(app: &AppHandle<R>, instance_id: &str) {
    if let Some(window) = app.get_webview_window("main")
        && !window.is_focused().unwrap_or(false)
        && let Err(e) = window.request_user_attention(Some(UserAttentionType::Informational))
    {
        tracing::debug!(error = %e, "couldn't flash the taskbar button");
    }
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let (app, db, id) = (app.clone(), state.db.clone(), instance_id.to_owned());
    tauri::async_runtime::spawn(async move {
        let name = match db::instances::get(&db, &id).await {
            Ok(Some(row)) => row.name,
            _ => "A pack".to_owned(),
        };
        let notice_ = Notice {
            title: "Some files need you".to_owned(),
            body: format!("{name} has files to download yourself before it can finish updating."),
            target: NoticeTarget::Pack {
                instance_id: id.clone(),
            },
        };
        notice(&app, &id, notice_);
    });
}

/// A background job failed. Its pack's page shows it, so this is only for when Atlas
/// isn't on screen.
pub fn notify_failed<R: Runtime>(app: &AppHandle<R>, instance_id: &str, name: &str, why: &str) {
    if window_visible(app) {
        return;
    }
    let notice_ = Notice {
        title: name.to_owned(),
        body: why.to_owned(),
        target: NoticeTarget::Pack {
            instance_id: instance_id.to_owned(),
        },
    };
    notice(app, instance_id, notice_);
}

/// Every notice goes out here: held while a game runs, at most one about the same pack
/// or person (`key`) an hour, inside Atlas when its window is visible and as a Windows
/// notification when it isn't, with the notice sound.
pub fn notice<R: Runtime>(app: &AppHandle<R>, key: &str, notice: Notice) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let prefs = &state.prefs;
    if !prefs.notifications.load(Ordering::Relaxed) {
        return;
    }
    {
        let mut last = prefs.last_notice.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if last
            .get(key)
            .is_some_and(|t| now.duration_since(*t) < NOTICE_GAP)
        {
            return;
        }
        last.insert(key.to_owned(), now);
    }
    if state.running.any() {
        prefs
            .held
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(notice);
        return;
    }
    show(app, &state, notice);
}

/// The game closed: what waited for it, as one notice.
pub fn release_held<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let held = std::mem::take(&mut *state.prefs.held.lock().unwrap_or_else(|e| e.into_inner()));
    if let Some(notice) = summary(held) {
        show(app, &state, notice);
    }
}

fn summary(mut held: Vec<Notice>) -> Option<Notice> {
    match held.len() {
        0 => None,
        1 => held.pop(),
        n => Some(Notice {
            title: format!("{n} things need you"),
            body: held
                .iter()
                .map(|h| h.body.as_str())
                .collect::<Vec<_>>()
                .join(" · "),
            target: NoticeTarget::Home,
        }),
    }
}

fn show<R: Runtime>(app: &AppHandle<R>, state: &AppState, notice: Notice) {
    let sound = state.prefs.notification_sound.load(Ordering::Relaxed);
    if window_visible(app) {
        emit(app, AtlasEvent::Notice { notice });
    } else {
        notify(app, &notice.title, &notice.body, sound);
    }
    if sound {
        play_sound();
    }
}

/// On screen: shown and not minimised (in front or not).
fn window_visible<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
}

/// Windows plays a custom sound with a toast only for packaged apps, so Atlas's toasts
/// are silent (no sound name) and Atlas plays this itself.
#[cfg(windows)]
static NOTICE_SOUND: &[u8] = include_bytes!("../sounds/notice.wav");

fn play_sound() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
        use windows_sys::Win32::UI::Shell::{
            QUNS_ACCEPTS_NOTIFICATIONS, SHQueryUserNotificationState,
        };
        let mut quiet = 0;
        // SAFETY: writes one value into `quiet`.
        let asked = unsafe { SHQueryUserNotificationState(&mut quiet) } == 0;
        // Do not disturb, a full-screen game or a presentation: stay silent.
        if !asked || quiet != QUNS_ACCEPTS_NOTIFICATIONS {
            return;
        }
        // SAFETY: the sound is static, so it outlives the asynchronous playback.
        unsafe {
            PlaySoundW(
                NOTICE_SOUND.as_ptr().cast(),
                std::ptr::null_mut(),
                SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
            )
        };
    }
}

/// `sound`: macOS plays its own notification sound (and keeps quiet during a Focus);
/// Windows' comes from `play_sound`.
fn notify<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str, sound: bool) {
    let mut builder = app.notification().builder().title(title).body(body);
    if sound && cfg!(target_os = "macos") {
        builder = builder.sound("NSUserNotificationDefaultSoundName");
    }
    if let Err(e) = builder.show() {
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
            format!("{who} sent an update to {what}"),
        ),
        ActivityType::InstanceSharedToGroup => (
            "Shared with a group",
            format!("{who} shared {what} with {group}"),
        ),
        ActivityType::MemberJoined => ("Pack added", format!("{who} added {what}")),
        ActivityType::MemberLeft => ("Pack left", format!("{who} left {what}")),
        ActivityType::OwnershipTransferred => (
            "New owner",
            if a.actor.as_ref().map(|u| u.id.as_str()) == me && me.is_some() {
                format!("You now own {what}")
            } else {
                format!("{who} now owns {what}")
            },
        ),
        ActivityType::InstanceFrozen => (
            "No longer updated",
            format!("{what} is no longer updated: the owner left"),
        ),
        ActivityType::GroupJoined => ("New group member", format!("{who} joined {group}")),
        ActivityType::RoleChanged => (
            "New role",
            format!("{who} made you {} of {what}", role_phrase(a.role)),
        ),
    };
    (title.to_owned(), body)
}

/// "an editor", "a viewer": what someone was made in `role_changed`.
fn role_phrase(role: Option<crate::dto::MemberRole>) -> &'static str {
    match role {
        Some(crate::dto::MemberRole::Editor) => "an editor",
        Some(crate::dto::MemberRole::Owner) => "the owner",
        _ => "a viewer",
    }
}

/// The tray and startup settings. Starting with Windows is Windows' own list of apps to
/// run at sign-in, so it's read from there rather than saved by Atlas.
pub fn settings<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> AppSettings {
    AppSettings {
        notifications: state.prefs.notifications.load(Ordering::Relaxed),
        notification_sound: state.prefs.notification_sound.load(Ordering::Relaxed),
        close_to_tray: state.prefs.close_to_tray.load(Ordering::Relaxed),
        start_with_windows: app
            .try_state::<AutoLaunchManager>()
            .is_some_and(|m| m.is_enabled().unwrap_or(false)),
        auto_update: state.prefs.auto_update.load(Ordering::Relaxed),
        start_with_windows_offered: state
            .prefs
            .start_with_windows_offered
            .load(Ordering::Relaxed),
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
    if let Some(on) = patch.notification_sound {
        db::settings::set(&state.db, NOTIFICATION_SOUND, &on).await?;
        state.prefs.notification_sound.store(on, Ordering::Relaxed);
    }
    if let Some(on) = patch.close_to_tray {
        db::settings::set(&state.db, CLOSE_TO_TRAY, &on).await?;
        state.prefs.close_to_tray.store(on, Ordering::Relaxed);
    }
    if let Some(on) = patch.auto_update {
        db::settings::set(&state.db, AUTO_UPDATE, &on).await?;
        state.prefs.auto_update.store(on, Ordering::Relaxed);
    }
    if let Some(offered) = patch.start_with_windows_offered {
        db::settings::set(&state.db, START_WITH_WINDOWS_OFFERED, &offered).await?;
        state
            .prefs
            .start_with_windows_offered
            .store(offered, Ordering::Relaxed);
    }
    if let Some(on) = patch.start_with_windows {
        let manager = app
            .try_state::<AutoLaunchManager>()
            .ok_or_else(|| AppError::unknown("Starting Atlas at sign-in isn't available here"))?;
        let done = if on {
            manager.enable()
        } else {
            manager.disable()
        };
        done.map_err(|e| {
            tracing::warn!(error = %e, "couldn't change starting with Windows");
            AppError::unknown("Atlas couldn't change that. Try again.")
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
            role: None,
        }
    }

    #[test]
    fn notifications_read_like_the_feed() {
        assert_eq!(
            describe(&item(ActivityType::InstanceUpdated), None),
            (
                "Update available".to_owned(),
                "dakep sent an update to SkyFactory 4 fixed".to_owned()
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
            "SkyFactory 4 fixed is no longer updated: the owner left"
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
        let mut role = item(ActivityType::RoleChanged);
        role.role = Some(crate::dto::MemberRole::Editor);
        assert_eq!(
            describe(&role, None).1,
            "dakep made you an editor of SkyFactory 4 fixed"
        );
    }

    fn shared_with_you(body: &str) -> Notice {
        Notice {
            title: "Shared with you".into(),
            body: body.into(),
            target: NoticeTarget::Invite {
                shared_instance_id: "s1".into(),
            },
        }
    }

    #[test]
    fn what_waited_for_the_game_is_one_notice() {
        assert_eq!(summary(vec![]), None);
        let one = shared_with_you("dakep shared Create SMP with you");
        assert_eq!(summary(vec![one.clone()]), Some(one));
        let two = summary(vec![
            shared_with_you("dakep shared Create SMP with you"),
            shared_with_you("Sam wants to be friends"),
        ])
        .unwrap();
        assert_eq!(two.title, "2 things need you");
        assert_eq!(
            two.body,
            "dakep shared Create SMP with you · Sam wants to be friends"
        );
        assert_eq!(two.target, NoticeTarget::Home);
    }

    #[test]
    fn notice_targets_read_like_types_ts() {
        assert_eq!(
            serde_json::to_value(shared_with_you("x").target).unwrap(),
            serde_json::json!({ "kind": "invite", "sharedInstanceId": "s1" })
        );
        assert_eq!(
            serde_json::to_value(NoticeTarget::Friends).unwrap(),
            serde_json::json!({ "kind": "friends" })
        );
    }
}
