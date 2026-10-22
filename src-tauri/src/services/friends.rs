//! Friends and the activity feed. Lists come from the server and fall back to the
//! offline cache; changes need the server and are never queued.

use atlas_protocol::api::{MarkRead, Page, SendFriendRequest};
use atlas_protocol::codes::{self, FRIEND_CODE_LEN};
use reqwest::Method;
use tauri::{AppHandle, Runtime};

use crate::dto::{ActivityIds, ActivityItem, Friend, FriendRequest, Id, PublicUser};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;

pub async fn list(state: &AppState) -> AppResult<Vec<Friend>> {
    state.cloud.cached_get("friends", "/friends").await
}

pub async fn requests(state: &AppState) -> AppResult<Vec<FriendRequest>> {
    state
        .cloud
        .cached_get("friend_requests", "/friends/requests")
        .await
}

fn normalize(code: &str) -> AppResult<String> {
    codes::normalize(code, FRIEND_CODE_LEN).ok_or_else(|| {
        AppError::invalid_input("Friend codes are 8 letters and numbers, like 7KQ2-9XMB")
    })
}

pub async fn preview_code(state: &AppState, code: &str) -> AppResult<PublicUser> {
    let path = format!("/friend-codes/{}", normalize(code)?);
    state.cloud.call(Method::GET, &path, None::<&()>).await
}

pub async fn send_request<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    code: &str,
) -> AppResult<FriendRequest> {
    let body = SendFriendRequest {
        code: normalize(code)?,
    };
    let request = state
        .cloud
        .call(Method::POST, "/friends/requests", Some(&body))
        .await?;
    emit(app, AtlasEvent::FriendsChanged {});
    Ok(request)
}

pub async fn respond<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    user_id: &Id,
    accept: bool,
) -> AppResult<()> {
    let verb = if accept { "accept" } else { "decline" };
    let path = format!("/friends/requests/{}/{verb}", segment(user_id)?);
    state
        .cloud
        .call::<serde_json::Value>(Method::POST, &path, None::<&()>)
        .await?;
    emit(app, AtlasEvent::FriendsChanged {});
    Ok(())
}

/// Cancel a request, unfriend, or block: one call, then everyone refetches.
pub async fn change<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    method: Method,
    path: &str,
    user_id: &Id,
) -> AppResult<()> {
    let path = format!("{path}/{}", segment(user_id)?);
    state.cloud.call::<()>(method, &path, None::<&()>).await?;
    emit(app, AtlasEvent::FriendsChanged {});
    Ok(())
}

pub async fn activity(state: &AppState) -> AppResult<Vec<ActivityItem>> {
    let page: Page<ActivityItem> = state.cloud.cached_get("activity", "/notifications").await?;
    Ok(page.items)
}

pub async fn mark_read(state: &AppState, ids: ActivityIds) -> AppResult<()> {
    let body = match ids {
        ActivityIds::All(_) => MarkRead {
            all: true,
            ..Default::default()
        },
        ActivityIds::Some(ids) => MarkRead {
            ids,
            ..Default::default()
        },
    };
    state
        .cloud
        .call::<()>(Method::POST, "/notifications/read", Some(&body))
        .await
}

/// A user id as a path segment. Ids are UUIDs; anything else is refused before it can
/// change the path.
fn segment(id: &str) -> AppResult<&str> {
    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        Ok(id)
    } else {
        Err(AppError::invalid_input("Unknown person"))
    }
}
