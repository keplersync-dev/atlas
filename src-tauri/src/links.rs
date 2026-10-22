//! `atlas://friend/<code>` and `atlas://group/<code>` links, which the server's /f/ and
//! /g/ pages open. Atlas never acts on one by itself: the UI shows who or what it leads
//! to and the user decides.

use atlas_protocol::codes::{self, FRIEND_CODE_LEN, GROUP_CODE_LEN};
use tauri::{AppHandle, Manager, Runtime};

use crate::dto::{LinkKind, OpenedLink};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;

pub const SCHEME: &str = "atlas";

pub fn parse(url: &str) -> Option<OpenedLink> {
    let url = url::Url::parse(url).ok()?;
    if url.scheme() != SCHEME {
        return None;
    }
    let (kind, len) = match url.host_str()? {
        "friend" => (LinkKind::Friend, FRIEND_CODE_LEN),
        "group" => (LinkKind::Group, GROUP_CODE_LEN),
        _ => return None,
    };
    let code = codes::normalize(url.path().trim_matches('/'), len)?;
    Some(OpenedLink { kind, code })
}

/// Bring Atlas to the front and hand the link to the UI. It's kept until the UI takes it
/// (`take_opened_link`), so a link that started Atlas waits for the page to load.
pub fn open<R: Runtime>(app: &AppHandle<R>, url: &str) {
    let Some(link) = parse(url) else {
        tracing::info!("ignoring a link Atlas doesn't know");
        return;
    };
    tracing::info!(kind = ?link.kind, "opened with a link");
    show_window(app);
    if let Some(state) = app.try_state::<AppState>() {
        *state.opened_link.lock().unwrap() = Some(link);
    }
    emit(app, AtlasEvent::LinkOpened {});
}

/// Bring the main window to the front.
pub fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_friend_and_group_links() {
        assert_eq!(
            parse("atlas://friend/7kq2-9xmb"),
            Some(OpenedLink {
                kind: LinkKind::Friend,
                code: "7KQ29XMB".into()
            })
        );
        assert_eq!(
            parse("atlas://group/K3M9PQ2XRT/"),
            Some(OpenedLink {
                kind: LinkKind::Group,
                code: "K3M9PQ2XRT".into()
            })
        );
        assert_eq!(parse("atlas://group/7KQ29XMB"), None, "wrong length");
        assert_eq!(parse("atlas://settings/x"), None);
        assert_eq!(parse("https://friend/7KQ29XMB"), None);
    }
}
