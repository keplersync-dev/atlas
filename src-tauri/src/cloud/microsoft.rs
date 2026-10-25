//! Signing in to Minecraft on this PC, the way launchers do: Microsoft in the browser
//! (authorization code + PKCE, back to the loopback listener), then Xbox Live, XSTS and
//! Minecraft services. The Minecraft token is used once, to join the Atlas server's
//! challenge, and then dropped with the others. None of them are stored.

use atlas_protocol::pkce;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use super::loopback;
use crate::error::{AppError, AppResult, ErrorCode};

/// Atlas's Azure app (a public client, so not a secret). Release builds can swap it.
const CLIENT_ID: &str = match option_env!("ATLAS_MS_CLIENT_ID") {
    Some(id) => id,
    None => "a24b3417-16d7-417b-9591-38f04d255bb2",
};
const AUTHORIZE: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBOX: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
const JOIN: &str = "https://sessionserver.mojang.com/session/minecraft/join";

/// A signed-in Minecraft player, for as long as the sign-in takes.
pub struct Player {
    access_token: String,
    /// Profile UUID without dashes.
    pub id: String,
    pub name: String,
}

/// Sign in with Microsoft in the browser and get the Minecraft player. `open` opens a URL
/// there.
pub async fn sign_in(http: &Client, open: impl FnOnce(&str) -> AppResult<()>) -> AppResult<Player> {
    let verifier = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    let state = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 16]>());
    let listener = loopback::bind().await?;
    let redirect_uri = format!("http://localhost:{}", listener.port);
    let mut url = Url::parse(AUTHORIZE).expect("valid");
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", "XboxLive.signin")
        .append_pair("code_challenge", &pkce::challenge(&verifier))
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state)
        .append_pair("prompt", "select_account");
    open(url.as_str())?;
    let code = listener
        .wait_for_code(loopback::Expect {
            path: "/",
            state: Some(&state),
        })
        .await?;

    let microsoft = microsoft_token(http, &code, &verifier, &redirect_uri).await?;
    let (xbox, user_hash) = xbox_token(http, &microsoft).await?;
    let xsts = xsts_token(http, &xbox).await?;
    let access_token = minecraft_token(http, &user_hash, &xsts).await?;
    let (id, name) = profile(http, &access_token).await?;
    Ok(Player {
        access_token,
        id,
        name,
    })
}

/// Join `server_id` as this player, which is what lets the Atlas server check who it is.
pub async fn join(http: &Client, player: &Player, server_id: &str) -> AppResult<()> {
    let body = json!({
        "accessToken": player.access_token,
        "selectedProfile": player.id,
        "serverId": server_id,
    });
    let res = http
        .post(JOIN)
        .json(&body)
        .send()
        .await
        .map_err(unreachable)?;
    if res.status().is_success() {
        return Ok(());
    }
    let status = res.status();
    let body: Value = res.json().await.unwrap_or_default();
    tracing::warn!(%status, %body, "Minecraft join failed");
    Err(join_error(&body))
}

fn join_error(body: &Value) -> AppError {
    match body["error"].as_str() {
        Some("InsufficientPrivilegesException") => AppError::forbidden(
            "Your Xbox privacy settings block multiplayer, which Atlas uses to check your \
             Minecraft account. Allow \"You can join multiplayer games\" at xbox.com, then \
             try again.",
        ),
        Some("UserBannedException") => {
            AppError::forbidden("This Minecraft account is banned from online play.")
        }
        _ => AppError::unknown("Minecraft didn't accept the sign-in. Try again."),
    }
}

#[derive(Deserialize)]
struct AccessToken {
    access_token: String,
}

async fn microsoft_token(
    http: &Client,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> AppResult<String> {
    let res = http
        .post(TOKEN)
        .form(&[
            ("client_id", CLIENT_ID),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
            ("scope", "XboxLive.signin"),
        ])
        .send()
        .await
        .map_err(unreachable)?;
    Ok(read::<AccessToken>(res, "Microsoft token")
        .await?
        .access_token)
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxToken {
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<UserHash>,
}

#[derive(Deserialize)]
struct UserHash {
    uhs: String,
}

/// The Xbox Live user token and the user hash that goes with it.
async fn xbox_token(http: &Client, microsoft: &str) -> AppResult<(String, String)> {
    let body = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={microsoft}"),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT",
    });
    let res = http
        .post(XBOX)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(unreachable)?;
    let token: XboxToken = read(res, "Xbox Live token").await?;
    let hash = token
        .display_claims
        .xui
        .into_iter()
        .next()
        .ok_or_else(|| AppError::unknown("Xbox Live sent something unexpected"))?
        .uhs;
    Ok((token.token, hash))
}

async fn xsts_token(http: &Client, xbox: &str) -> AppResult<String> {
    let body = json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbox] },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT",
    });
    let res = http
        .post(XSTS)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(unreachable)?;
    if res.status() == StatusCode::UNAUTHORIZED {
        let body: Value = res.json().await.unwrap_or_default();
        tracing::warn!(%body, "XSTS refused the sign-in");
        return Err(xsts_error(body["XErr"].as_u64()));
    }
    Ok(read::<XboxToken>(res, "XSTS token").await?.token)
}

/// XSTS's reasons for refusing an account.
fn xsts_error(xerr: Option<u64>) -> AppError {
    let message = match xerr {
        Some(2148916233) => {
            "This Microsoft account has no Xbox profile. Sign in once at xbox.com to create \
             one, then try again."
        }
        Some(2148916235) => "Xbox Live isn't available in your country or region.",
        Some(2148916236 | 2148916237) => {
            "This account needs adult verification on xbox.com before it can sign in."
        }
        Some(2148916238) => {
            "This is a child account. An adult needs to add it to a Microsoft family before \
             it can sign in."
        }
        Some(2148916227) => "This Xbox account is banned.",
        Some(2148916229) => {
            "This account's parental controls block online play. An adult can change that in \
             the Microsoft family settings."
        }
        Some(2148916234) => "Accept the Xbox terms at xbox.com, then try again.",
        _ => "Xbox Live didn't accept this account. Try again.",
    };
    AppError::forbidden(message)
}

async fn minecraft_token(http: &Client, user_hash: &str, xsts: &str) -> AppResult<String> {
    let body = json!({ "identityToken": format!("XBL3.0 x={user_hash};{xsts}") });
    let res = http
        .post(LOGIN)
        .json(&body)
        .send()
        .await
        .map_err(unreachable)?;
    if res.status() == StatusCode::FORBIDDEN {
        let body = res.text().await.unwrap_or_default();
        tracing::error!(%body, "Minecraft services refused Atlas's app registration");
        return Err(AppError::unknown(
            "Minecraft sign-in isn't working for Atlas right now. Use Discord for now.",
        ));
    }
    Ok(read::<AccessToken>(res, "Minecraft token")
        .await?
        .access_token)
}

#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}

async fn profile(http: &Client, minecraft: &str) -> AppResult<(String, String)> {
    let res = http
        .get(PROFILE)
        .bearer_auth(minecraft)
        .send()
        .await
        .map_err(unreachable)?;
    if res.status() == StatusCode::NOT_FOUND {
        return Err(AppError::forbidden(
            "This Microsoft account doesn't have Minecraft: Java Edition. If you just got it, \
             start the game once from the Minecraft Launcher, then try again.",
        ));
    }
    let profile: Profile = read(res, "Minecraft profile").await?;
    Ok((profile.id, profile.name))
}

/// A successful JSON answer, or a logged failure.
async fn read<T: serde::de::DeserializeOwned>(res: reqwest::Response, what: &str) -> AppResult<T> {
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        tracing::warn!(%status, %body, "{what} request failed");
        return Err(AppError::unknown(
            "Signing in to Minecraft didn't work. Try again.",
        ));
    }
    res.json().await.map_err(|e| {
        tracing::error!(error = %e, "unexpected {what}");
        AppError::unknown("Signing in to Minecraft didn't work. Try again.")
    })
}

fn unreachable(e: reqwest::Error) -> AppError {
    tracing::warn!(error = %e, "couldn't reach Microsoft or Minecraft");
    AppError::new(
        ErrorCode::Offline,
        "Can't reach Microsoft or Minecraft right now. Try again in a little while.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xsts_refusals_say_what_to_do() {
        assert!(
            xsts_error(Some(2148916233))
                .message
                .contains("no Xbox profile")
        );
        assert!(
            xsts_error(Some(2148916238))
                .message
                .contains("child account")
        );
        assert!(xsts_error(None).message.contains("didn't accept"));
    }

    #[test]
    fn join_refusals_say_what_to_do() {
        let blocked = json!({ "error": "InsufficientPrivilegesException", "errorMessage": "x" });
        assert!(join_error(&blocked).message.contains("privacy settings"));
        assert!(join_error(&Value::Null).message.contains("Try again"));
    }
}
