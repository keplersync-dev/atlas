//! Accepting an invite: into a new Prism/MultiMC instance, or into an instance you already
//! track (any launcher). Either way the folder is linked to the shared instance and a
//! job installs the latest version's files; the user's own files are left alone.

use std::collections::HashSet;
use std::path::Path;

use reqwest::Method;
use tauri::{AppHandle, Runtime};

use super::shared::segment;
use crate::db;
use crate::dto::{BackupReason, Id, InstanceSummary, SharedInstance, TrackInstanceRef};
use crate::error::{AppError, AppResult};
use crate::jobs;
use crate::launchers;
use crate::state::AppState;
use crate::sync::apply;
use crate::sync::backup;
use crate::sync::source::Source;

/// Accept into a new instance in a launcher Atlas can create instances in.
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
    if !launchers::can_create_instances(kind) {
        return Err(AppError::invalid_input(format!(
            "Atlas can't create instances in {} yet",
            launchers::display_name(kind)
        )));
    }
    let invite = pending(state, shared_id).await?;
    let shared = accept(state, shared_id).await?;
    let src = server_source(&shared);
    let manifest = src.manifest(state, shared.latest_revision).await?;

    let folder = launchers::create_instance(
        kind,
        Path::new(&launcher.root_path),
        &invite.name,
        &manifest.minecraft.version,
        manifest.minecraft.loader,
        manifest.minecraft.loader_version.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::warn!(error = %e, "couldn't create the instance");
        AppError::unknown("Couldn't create the instance in the launcher")
    })?;
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
    pending(state, shared_id).await?;
    let shared = accept(state, shared_id).await?;
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

/// The invite, which must still be waiting for an answer.
async fn pending(state: &AppState, shared_id: &str) -> AppResult<SharedInstance> {
    let invite = super::shared::cached(state, shared_id).await?;
    match invite {
        Some(s) if s.my_status == crate::dto::MemberStatus::Invited => Ok(s),
        Some(_) => Err(AppError::conflict("No pending invite")),
        // Not fetched yet: let the server decide.
        None => {
            let path = format!("/shared-instances/{}", segment(shared_id)?);
            state.cloud.call(Method::GET, &path, None::<&()>).await
        }
    }
}

async fn accept(state: &AppState, shared_id: &str) -> AppResult<SharedInstance> {
    let path = format!("/shared-instances/{}/accept", segment(shared_id)?);
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
