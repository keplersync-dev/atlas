//! Request and response bodies for the server's REST API. JSON in camelCase.
//! The user types are also what the app hands its UI (they mirror src/lib/types.ts).

use serde::{Deserialize, Serialize};

use crate::enums::AuthProvider;
use crate::{Id, IsoDate};

// ── Users ──────────────────────────────────────────────────────────────

/// Any Atlas user as seen by others.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: Id,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub minecraft_uuid: Option<String>,
    pub minecraft_username: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedIdentity {
    pub provider: AuthProvider,
    /// Minecraft name or Discord username at the last sign-in.
    pub username: String,
    pub linked_at: IsoDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub used_bytes: u64,
    pub quota_bytes: u64,
    pub max_file_bytes: u64,
}

/// The signed-in user, with private fields. `extends PublicUser` in types.ts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    #[serde(flatten)]
    pub user: PublicUser,
    /// "7KQ29XMB"
    pub friend_code: String,
    /// e.g. "https://atlas.example/f/7KQ29XMB"
    pub friend_link: String,
    pub identities: Vec<LinkedIdentity>,
    pub appear_offline: bool,
    /// When the username can next be changed (None = now).
    pub username_change_available_at: Option<IsoDate>,
    pub storage: Storage,
}

/// `PATCH /me`. Missing fields stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appear_offline: Option<bool>,
}

// ── Auth ───────────────────────────────────────────────────────────────

/// Query of `GET /auth/discord/start`, opened in the browser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordStart {
    /// The app's loopback port: the browser comes back to `http://127.0.0.1:{port}/callback`.
    pub port: u16,
    /// PKCE S256 challenge of the verifier the app keeps.
    pub challenge: String,
}

/// The one-time code from the loopback redirect, with the PKCE verifier it was started with.
/// `POST /auth/exchange` trades it for tokens; `POST /me/identities` links that identity to
/// the signed-in account instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRequest {
    pub code: String,
    pub verifier: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub access_token: String,
    /// Seconds until the access token expires.
    pub expires_in: u64,
    /// Replaces the previous refresh token, which stops working.
    pub refresh_token: String,
}

/// Answer to a sign-in. Linking answers with `Me` alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResult {
    #[serde(flatten)]
    pub tokens: Tokens,
    pub user: Me,
    /// The account was created by this sign-in.
    pub is_new: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    pub refresh_token: String,
}

// ── Errors ─────────────────────────────────────────────────────────────

/// `{ "error": { "code": "conflict", "message": "…" } }` on every non-2xx answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetail {
    pub code: ErrorCode,
    /// Shown to players as it is.
    pub message: String,
    /// With `client_outdated`: the oldest app version the server accepts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidInput,
    NotSignedIn,
    Forbidden,
    NotFound,
    Conflict,
    QuotaExceeded,
    ClientOutdated,
    RateLimited,
    /// Also any code this build doesn't know yet.
    #[serde(other)]
    Unknown,
}

impl ErrorCode {
    pub fn http_status(self) -> u16 {
        match self {
            Self::InvalidInput => 400,
            Self::NotSignedIn => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::QuotaExceeded => 413,
            Self::ClientOutdated => 426,
            Self::RateLimited => 429,
            Self::Unknown => 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn me_flattens_the_public_fields() {
        let me = Me {
            user: PublicUser {
                id: "u1".into(),
                username: "alex".into(),
                display_name: "Alex".into(),
                avatar_url: None,
                minecraft_uuid: None,
                minecraft_username: None,
            },
            friend_code: "7KQ29XMB".into(),
            friend_link: "https://atlas.example/f/7KQ29XMB".into(),
            identities: vec![LinkedIdentity {
                provider: AuthProvider::Discord,
                username: "alex".into(),
                linked_at: "2026-10-22T00:00:00.000Z".into(),
            }],
            appear_offline: false,
            username_change_available_at: None,
            storage: Storage {
                used_bytes: 0,
                quota_bytes: 524_288_000,
                max_file_bytes: 104_857_600,
            },
        };
        let v = serde_json::to_value(&me).unwrap();
        assert_eq!(v["displayName"], "Alex");
        assert_eq!(v["identities"][0]["provider"], "discord");
        assert_eq!(v["storage"]["quotaBytes"], 524_288_000);
        assert_eq!(serde_json::from_value::<Me>(v).unwrap(), me);
    }

    #[test]
    fn unknown_error_codes_still_parse() {
        let body: ErrorBody = serde_json::from_value(json!({
            "error": { "code": "brand_new_code", "message": "Hm" }
        }))
        .unwrap();
        assert_eq!(body.error.code, ErrorCode::Unknown);
        assert_eq!(ErrorCode::ClientOutdated.http_status(), 426);
    }

    #[test]
    fn profile_patch_leaves_out_missing_fields() {
        let patch = UpdateProfileInput {
            display_name: Some("A".into()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&patch).unwrap(),
            json!({ "displayName": "A" })
        );
    }
}
