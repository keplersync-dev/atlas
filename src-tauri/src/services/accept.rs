//! Adding a shared instance to this PC: into a new instance in a launcher Atlas can
//! create them in, or into an instance you already track (any launcher). Either way the
//! folder is linked to the shared instance and a job installs the latest version's files;
//! the user's own files are left alone. A pending invite is accepted on the way; members
//! and owners can add one they already have, after they stopped tracking it here.

use std::collections::HashSet;
use std::path::Path;

use reqwest::Method;
use tauri::{AppHandle, Runtime};

use super::shared::segment;
use crate::db;
use crate::dto::{
    BackupReason, Id, InstanceSummary, LauncherKind, Loader, MemberStatus, SharedInstance,
    TrackInstanceRef,
};
use crate::error::{AppError, AppResult};
use crate::jobs;
use crate::launchers;
use crate::state::AppState;
use crate::sync::apply;
use crate::sync::backup;
use crate::sync::source::Source;

/// Accept into a new instance in a launcher Atlas can create instances in. The instance
/// is made first, from the invite's versions, so an instance that can't be made (the
/// launcher is open, or the user says no in the Modrinth App) leaves the invite as it was.
pub async fn into_new<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    shared_id: &Id,
    launcher_id: &Id,
    install_optional: Vec<String>,
) -> AppResult<InstanceSummary> {
    let launcher = db::launchers::get(&state.db, launcher_id)
        .await?
        .ok_or_else(|| AppError::not_found("Launcher not found"))?;
    let kind = launcher
        .kind()
        .ok_or_else(|| AppError::not_found("Launcher not found"))?;
    let invite = addable(state, shared_id).await?;
    if !launchers::can_create_instance(kind, invite.loader) {
        return Err(AppError::invalid_input(format!(
            "Atlas can't set up {} in {} yet",
            versions_label(&invite),
            launchers::display_name(kind)
        )));
    }
    if let Some(reason) = launchers::create_blocker(kind) {
        return Err(AppError::conflict(reason));
    }

    let new = launchers::NewInstance {
        name: invite.name.clone(),
        minecraft: invite.minecraft_version.clone(),
        loader: invite.loader,
        loader_version: invite.loader_version.clone(),
        icon: super::shared::icon_bytes(state, &invite).await,
    };
    let curseforge_loader = match (kind, &invite.loader_version) {
        (LauncherKind::Curseforge, Some(version)) if invite.loader != Loader::Vanilla => {
            Some(curseforge_loader(state, &invite, version).await?)
        }
        _ => None,
    };
    let root = Path::new(&launcher.root_path);
    let folder = launchers::create_instance(kind, root, new, &state.http, curseforge_loader)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "couldn't create the instance");
            match e {
                launchers::ReadError::NotCreated => AppError::conflict(format!(
                    "{} didn't make the instance. Try again and confirm it there.",
                    launchers::display_name(kind)
                )),
                _ => AppError::unknown("Couldn't create the instance in the launcher"),
            }
        })?;

    let shared = join(state, invite).await?;
    let src = server_source(&shared);
    let manifest = src.manifest(state, shared.latest_revision).await?;
    let tracked = super::instances::track(
        state,
        app,
        &[TrackInstanceRef {
            launcher_kind: kind,
            launcher_instance_id: folder,
        }],
    )
    .await?;
    let instance = tracked
        .first()
        .ok_or_else(|| AppError::unknown("Couldn't find the new instance in the launcher"))?;
    install(
        state,
        app,
        &instance.id,
        shared,
        src,
        manifest,
        install_optional,
    )
    .await
}

/// Accept into an instance already tracked here. It must not be shared already.
pub async fn into_existing<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    shared_id: &Id,
    instance_id: &Id,
    install_optional: Vec<String>,
) -> AppResult<InstanceSummary> {
    let row = db::instances::get(&state.db, instance_id)
        .await?
        .ok_or_else(|| AppError::not_found("Instance not found"))?;
    if row.shared_instance_id.is_some() {
        return Err(AppError::conflict(
            "That instance is already shared. Pick another one.",
        ));
    }
    if !Path::new(&row.game_dir).is_dir() {
        return Err(AppError::conflict("The folder is missing"));
    }
    let invite = addable(state, shared_id).await?;
    let shared = join(state, invite).await?;
    let src = server_source(&shared);
    let manifest = src.manifest(state, shared.latest_revision).await?;
    install(
        state,
        app,
        instance_id,
        shared,
        src,
        manifest,
        install_optional,
    )
    .await
}

/// CurseForge's install data for the invite's loader. Its API needs a key, which the
/// Atlas server keeps, so the server asks.
async fn curseforge_loader(
    state: &AppState,
    invite: &SharedInstance,
    version: &str,
) -> AppResult<serde_json::Value> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("minecraft", &invite.minecraft_version)
        .append_pair("loader", &crate::dto::to_wire(invite.loader))
        .append_pair("version", version)
        .finish();
    state
        .cloud
        .call(
            Method::GET,
            &format!("/curseforge/modloader?{query}"),
            None::<&()>,
        )
        .await
}

/// "NeoForge 1.21.1", for messages.
fn versions_label(s: &SharedInstance) -> String {
    let loader = match s.loader {
        Loader::Vanilla => "Minecraft",
        Loader::Fabric => "Fabric",
        Loader::Quilt => "Quilt",
        Loader::Forge => "Forge",
        Loader::Neoforge => "NeoForge",
    };
    format!("{loader} {}", s.minecraft_version)
}

/// The server's copy (the cached one may be on older versions), if it can be added here:
/// you're invited or already in it, and no instance on this PC has it yet.
async fn addable(state: &AppState, shared_id: &str) -> AppResult<SharedInstance> {
    let path = format!("/shared-instances/{}", segment(shared_id)?);
    let shared: SharedInstance = state.cloud.call(Method::GET, &path, None::<&()>).await?;
    if shared.my_status == MemberStatus::Declined {
        return Err(AppError::conflict("No pending invite"));
    }
    let rows = db::instances::list(&state.db).await?;
    if rows
        .iter()
        .any(|r| super::shared::server_id(r) == Some(shared_id))
    {
        return Err(AppError::conflict("It's already on this PC"));
    }
    Ok(shared)
}

/// Accept the invite if it's still pending; members and owners are in already.
async fn join(state: &AppState, shared: SharedInstance) -> AppResult<SharedInstance> {
    if shared.my_status != MemberStatus::Invited {
        return Ok(shared);
    }
    let path = format!("/shared-instances/{}/accept", segment(&shared.id)?);
    state.cloud.call(Method::POST, &path, None::<&()>).await
}

fn server_source(shared: &SharedInstance) -> Source {
    Source::Server {
        shared_instance_id: shared.id.clone(),
        latest: shared.latest_revision,
        frozen: shared.frozen,
    }
}

/// Link the folder and start installing the shared files. Returns once the job started.
async fn install<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    instance_id: &str,
    shared: SharedInstance,
    src: Source,
    manifest: crate::sync::manifest::Manifest,
    install_optional: Vec<String>,
) -> AppResult<InstanceSummary> {
    db::instances::set_link(&state.db, instance_id, Some((&shared.id, shared.my_role))).await?;
    let revision = shared.latest_revision;
    let state2 = state.clone();
    jobs::spawn(state, app, instance_id, move |progress| async move {
        let req = apply::Request {
            revision,
            manifest,
            reason: BackupReason::BeforeUpdate,
            install_optional: install_optional.into_iter().collect::<HashSet<_>>(),
            config_choices: Default::default(),
        };
        apply::run(&state2, &progress, &src, req).await?;
        super::sync::report_if_shared(&state2, &progress.instance_id, &src).await;
        backup::prune(&state2.db, &state2.paths, &progress.instance_id).await
    })?;
    super::shared::changed(state, app, &shared.id).await;
    super::instances::summary_by_id(state, instance_id).await
}
