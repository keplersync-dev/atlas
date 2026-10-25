//! Request and response bodies for the server's REST API. JSON in camelCase.
//! The user types are also what the app hands its UI (they mirror src/lib/types.ts).

use serde::{Deserialize, Serialize};

use crate::enums::{
    ActivityType, AuthProvider, AvatarSource, ContentKind, GroupMemberRole, GroupRole, InviteRole,
    Loader, MemberRole, MemberStatus, PresenceState, ReportReason, ReportTarget, RequestDirection,
    Side,
};
use crate::manifest::Manifest;
use crate::{Id, IsoDate};

// ── Users ──────────────────────────────────────────────────────────────

/// Any Atlas user as seen by others.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: Id,
    pub username: String,
    pub display_name: String,
    /// The picture to show, whichever they chose: their Minecraft skin (draw its face) or
    /// a picture (the Discord avatar). Both None = initials.
    pub skin_url: Option<String>,
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
    /// Which picture they chose, and both to choose from (None = not linked, or no
    /// custom skin / avatar). Defaults so a `Me` the app saved before this existed loads.
    #[serde(default = "discord")]
    pub avatar_source: AvatarSource,
    pub minecraft_skin_url: Option<String>,
    pub discord_avatar_url: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_source: Option<AvatarSource>,
}

fn discord() -> AvatarSource {
    AvatarSource::Discord
}

// ── Friends ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub state: PresenceState,
    /// Set when playing, unless the friend hid it.
    pub instance_name: Option<String>,
    pub minecraft_version: Option<String>,
    /// Set when playing a shared instance you can see too.
    pub shared_instance_id: Option<Id>,
    /// When offline, this is "last seen".
    pub updated_at: IsoDate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub user: PublicUser,
    pub friends_since: IsoDate,
    pub presence: Presence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequest {
    pub direction: RequestDirection,
    pub user: PublicUser,
    pub created_at: IsoDate,
}

/// `POST /friends/requests`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendFriendRequest {
    /// Normalized friend code ("7KQ29XMB").
    pub code: String,
}

// ── Shared instances ───────────────────────────────────────────────────

/// `{ id, name }`: how a group is named on a member row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRef {
    pub id: Id,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedMember {
    pub user: PublicUser,
    pub role: MemberRole,
    pub status: MemberStatus,
    pub via_group: Option<GroupRef>,
    pub last_synced_revision: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupLinkGroup {
    pub id: Id,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedGroupLink {
    pub group: GroupLinkGroup,
    pub role: InviteRole,
}

/// An optional file a friend can choose to download.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalItem {
    /// Provider project id, or the file's sha256 for uploads.
    pub key: String,
    pub name: String,
    pub kind: ContentKind,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedInstance {
    pub id: Id,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    /// None when frozen with no owner.
    pub owner: Option<PublicUser>,
    pub minecraft_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub latest_revision: u32,
    pub content_count: u32,
    /// Total download size of the latest revision.
    pub size_bytes: u64,
    pub updated_at: IsoDate,
    pub frozen: bool,
    pub my_role: MemberRole,
    pub my_status: MemberStatus,
    pub my_via_group: Option<GroupRef>,
    pub invited_by: Option<PublicUser>,
    pub invited_at: Option<IsoDate>,
    pub members: Vec<SharedMember>,
    pub groups: Vec<SharedGroupLink>,
    /// Filled in by the app from its own instances; the server always sends null.
    #[serde(default)]
    pub local_instance_id: Option<Id>,
    pub optional_items: Vec<OptionalItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedItem {
    pub name: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<UpdatedItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionSummary {
    pub revision: u32,
    pub changelog: Option<String>,
    pub created_by: Option<PublicUser>,
    pub created_at: IsoDate,
    pub changes: RevisionChanges,
}

/// `SharedInstanceDetail extends SharedInstance` in types.ts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedInstanceDetail {
    #[serde(flatten)]
    pub shared: SharedInstance,
    /// Newest first.
    pub revisions: Vec<RevisionSummary>,
}

/// `POST /shared-instances`: share an instance; its manifest becomes revision 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedInstance {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub manifest: Manifest,
    pub friend_ids: Vec<Id>,
    #[serde(default)]
    pub group_ids: Vec<Id>,
    pub role: InviteRole,
}

/// `POST /shared-instances/{id}/revisions`. Refused with 409 unless `base_revision` is
/// the latest, so nobody publishes over a version they haven't seen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRevision {
    pub base_revision: u32,
    pub manifest: Manifest,
    #[serde(default)]
    pub changelog: Option<String>,
}

/// `GET /shared-instances/{id}/revisions/{n}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub revision: u32,
    pub manifest: Manifest,
    pub manifest_hash: String,
}

/// `PATCH /shared-instances/{id}`. Missing fields stay; `description: null` clears it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSharedInstance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub description: Option<Option<String>>,
}

/// `POST /shared-instances/{id}/members`: invite friends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMembers {
    pub user_ids: Vec<Id>,
    pub role: InviteRole,
}

/// `PATCH /shared-instances/{id}/members/{userId}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMemberRole {
    pub role: InviteRole,
}

/// `POST /shared-instances/{id}/transfer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub user_id: Id,
}

/// `PUT /shared-instances/{id}/synced`: the revision this member's folder is on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Synced {
    pub revision: u32,
}

/// `POST /shared-instances/{id}/groups`: share with everyone in a group you're in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareToGroup {
    pub group_id: Id,
    pub role: InviteRole,
}

// ── Groups ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMember {
    pub user: PublicUser,
    pub role: GroupRole,
    pub joined_at: IsoDate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInvite {
    pub code: String,
    /// A web page that opens the invite in Atlas.
    pub link: String,
    /// None if they deleted their account.
    pub created_by: Option<PublicUser>,
    pub max_uses: Option<u32>,
    pub uses: u32,
    pub expires_at: Option<IsoDate>,
    pub created_at: IsoDate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: Id,
    pub name: String,
    pub icon: Option<String>,
    pub my_role: GroupRole,
    pub members: Vec<GroupMember>,
    /// Shared instances available to the group.
    pub shared_instance_ids: Vec<Id>,
    pub created_at: IsoDate,
}

/// `GET /groups/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDetail {
    #[serde(flatten)]
    pub group: Group,
    /// Only filled for the owner and admins.
    pub invites: Vec<GroupInvite>,
}

/// `GET /group-invites/{code}`: what an invite leads to, before joining.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInvitePreview {
    pub code: String,
    pub group_id: Id,
    pub group_name: String,
    pub group_icon: Option<String>,
    pub member_count: u32,
    pub invited_by: Option<PublicUser>,
    pub already_member: bool,
}

/// `POST /groups`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGroup {
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
}

/// `PATCH /groups/{id}`. Missing fields stay; `icon: null` clears it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "double_option"
    )]
    pub icon: Option<Option<String>>,
}

/// `POST /groups/{id}/invites`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGroupInvite {
    /// None = any number of people.
    #[serde(default)]
    pub max_uses: Option<u32>,
    /// None = never expires.
    #[serde(default)]
    pub expires_in_hours: Option<u32>,
}

/// `PATCH /groups/{id}/members/{userId}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetGroupRole {
    pub role: GroupMemberRole,
}

/// Missing = `None`, null = `Some(None)`, value = `Some(Some(v))`.
mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, T: Serialize>(
        value: &Option<Option<T>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(inner) => inner.serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
        d: D,
    ) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}

// ── Files ──────────────────────────────────────────────────────────────

/// `POST /blobs/check`: which of these files the server already stores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobCheck {
    pub sha256: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobCheckResult {
    pub existing: Vec<String>,
}

/// `POST /uploads`: ask to upload one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewUpload {
    pub sha256: String,
    pub size_bytes: u64,
    /// For display only.
    pub file_name: String,
}

/// Either the server has the file already, or `url` is a presigned PUT for its bytes,
/// valid until `expires_at`. Then `POST /uploads/{upload_id}/complete`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadTicket {
    #[serde(default)]
    pub already_exists: bool,
    #[serde(default)]
    pub upload_id: Option<Id>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub expires_at: Option<IsoDate>,
}

/// `POST /uploads/{id}/complete`: the server checked the stored bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadDone {
    pub sha256: String,
}

// ── CurseForge ─────────────────────────────────────────────────────────

/// `POST /curseforge/fingerprints`: look files up by CurseForge fingerprint (murmur2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeLookup {
    pub fingerprints: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeLookupResult {
    pub files: Vec<CurseForgeFile>,
}

/// A file CurseForge knows by its fingerprint. Fingerprints are 32-bit, so check `sha1`
/// before trusting a match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFile {
    pub fingerprint: u32,
    pub project_id: String,
    pub file_id: String,
    /// The project's name: "Just Enough Items (JEI)"
    pub name: String,
    /// The file's display name, often its file name: "jei-1.21.1-neoforge-19.21.0.247.jar"
    pub version_label: String,
    pub sha1: Option<String>,
    pub author: Option<String>,
    pub icon_url: Option<String>,
    /// None when the author turned off downloads outside CurseForge: members download it
    /// from `page_url` themselves.
    pub download_url: Option<String>,
    /// The file's page on curseforge.com.
    pub page_url: String,
    pub side: Side,
}

// ── Reports ────────────────────────────────────────────────────────────

/// `POST /reports`: tell Atlas about something that breaks the rules. Only things the
/// reporter can see: a member's shared instance, a group they're in, any user, or an
/// uploaded file (`target_id` is its sha256) in `shared_instance_id`, which they're in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewReport {
    pub target_type: ReportTarget,
    pub target_id: String,
    pub reason: ReportReason,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub shared_instance_id: Option<Id>,
}

// ── Activity ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: Id,
    #[serde(rename = "type")]
    pub kind: ActivityType,
    /// None if the actor deleted their account.
    pub actor: Option<PublicUser>,
    pub created_at: IsoDate,
    pub read_at: Option<IsoDate>,
    pub shared_instance_id: Option<Id>,
    pub shared_instance_name: Option<String>,
    pub group_id: Option<Id>,
    pub group_name: Option<String>,
    pub revision: Option<u32>,
}

/// `POST /notifications/read`: these ids, or everything.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkRead {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ids: Vec<Id>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub all: bool,
}

/// One page of a list, newest first. Pass `nextCursor` as `?before=` for the next page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
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

/// Answer to `POST /auth/minecraft/challenge`. The app joins this one-time server id with
/// its Minecraft session (Mojang's `session/minecraft/join`), the way the game joins a
/// server, then sends it back for the server to check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftChallenge {
    pub server_id: String,
}

/// After joining: `POST /auth/minecraft/verify` signs in with it, and
/// `POST /me/identities/minecraft` links it to the signed-in account instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVerify {
    pub server_id: String,
    /// The Minecraft name the app joined with.
    pub username: String,
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
                skin_url: None,
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
            avatar_source: AvatarSource::Discord,
            minecraft_skin_url: None,
            discord_avatar_url: None,
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
