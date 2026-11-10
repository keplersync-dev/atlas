//! Friends and the activity feed. Lists come from the server and fall back to the
//! offline cache; changes need the server and are never queued.

use atlas_protocol::api::{MarkRead, SendFriendRequest};
use atlas_protocol::codes::{self, FRIEND_CODE_LEN};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::cloud::Lenient;
use crate::dto::{ActivityIds, ActivityItem, Friend, FriendRequest, Id, PresenceState, PublicUser};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;

/// Offline, the cached list comes back with everyone shown offline: their last known
/// presence would be out of date.
pub async fn list(state: &AppState) -> AppResult<Vec<Friend>> {
    let mut friends = state
        .cloud
        .cached_get::<Lenient<Friend>>("friends", "/friends")
        .await?
        .items;
    if !state.cloud.is_online() {
        for friend in &mut friends {
            let p = &mut friend.presence;
            p.state = PresenceState::Offline;
            p.instance_name = None;
            p.minecraft_version = None;
            p.shared_instance_id = None;
        }
    }
    Ok(friends)
}

pub async fn requests(state: &AppState) -> AppResult<Vec<FriendRequest>> {
    let requests: Lenient<FriendRequest> = state
        .cloud
        .cached_get("friend_requests", "/friends/requests")
        .await?;
    Ok(requests.items)
}

fn normalize(code: &str) -> AppResult<String> {
    codes::normalize(code, FRIEND_CODE_LEN).ok_or_else(|| {
        AppError::invalid_input("Friend codes are 8 letters and numbers, like 7KQ2-9XMB")
    })
}

/// Minecraft names are letters, digits and underscores, so they need no escaping.
fn name(name: &str) -> AppResult<String> {
    let name = name.trim().trim_start_matches('@');
    if name.is_empty()
        || name.len() > 16
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(AppError::invalid_input(format!(
            "No one called {name} is on KeplerSync. Check the spelling."
        )));
    }
    Ok(name.to_owned())
}

pub async fn preview(state: &AppState, who: &SendFriendRequest) -> AppResult<PublicUser> {
    let path = match (&who.name, &who.code) {
        (Some(n), _) => format!("/friend-names/{}", name(n)?),
        (None, Some(code)) => format!("/friend-codes/{}", normalize(code)?),
        (None, None) => return Err(AppError::invalid_input("Type their Minecraft name")),
    };
    state.cloud.call(Method::GET, &path, None::<&()>).await
}

pub async fn send_request<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    who: SendFriendRequest,
) -> AppResult<FriendRequest> {
    let body = SendFriendRequest {
        name: who.name.as_deref().map(name).transpose()?,
        code: match who.name {
            Some(_) => None,
            None => who.code.as_deref().map(normalize).transpose()?,
        },
    };
    let request = state
        .cloud
        .call(Method::POST, "/friends/requests", Some(&body))
        .await?;
    emit(app, AtlasEvent::FriendsChanged {});
    Ok(request)
}

/// Ask someone you share a group with, no code needed.
pub async fn send_request_to<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    user_id: &Id,
) -> AppResult<FriendRequest> {
    let path = format!("/friends/requests/{}", segment(user_id)?);
    let request = state.cloud.call(Method::POST, &path, None::<&()>).await?;
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

/// The people you blocked, so a block can be undone.
pub async fn blocked(state: &AppState) -> AppResult<Vec<crate::dto::PublicUser>> {
    state.cloud.call(Method::GET, "/blocks", None::<&()>).await
}

/// `Page<ActivityItem>`, read leniently.
#[derive(Serialize, Deserialize)]
struct Feed {
    items: Lenient<ActivityItem>,
}

pub async fn activity(state: &AppState) -> AppResult<Vec<ActivityItem>> {
    let feed: Feed = state.cloud.cached_get("activity", "/notifications").await?;
    Ok(feed.items.items)
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

/// A user id as a path segment (`ids::segment`).
fn segment(id: &str) -> AppResult<&str> {
    crate::util::ids::segment(id, || {
        AppError::invalid_input("That person isn't on Atlas anymore")
    })
}
