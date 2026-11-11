//! Sharing an instance and publishing new versions of it: upload the files that aren't
//! on a mod site and the shared config files, build a manifest from the folder's files
//! and their share settings, send it, then mark the files as published. Nothing changes
//! locally until the server has accepted it.
//!
//! - plan.rs: what the share dialog shows (`prepare`)
//! - configs.rs: which files besides mods and packs are shared, read ahead of the dialog
//! - uploads.rs: sending what the server doesn't have yet
//! - here: the manifest, and sending it

mod configs;
mod plan;
mod uploads;

use std::collections::HashMap;

use reqwest::Method;
use tauri::{AppHandle, Runtime};

pub use configs::{read_ahead_soon, shared_config_files};
pub use plan::prepare;

use super::share_files;
use super::work::Reporter;
use crate::db::{self, content::ContentRow, instances::InstanceRow};
use crate::dto::{
    ContentKind, ContentSource, CreateRevision, CreateSharedInstance, FileRules, InstanceSummary,
    MemberRole, PublishInput, RevisionSummary, ShareInstanceInput, ShareMode, SharedInstance, Side,
    WorkStage, from_wire, to_wire,
};
use crate::error::{AppError, AppResult};
use crate::events::{AtlasEvent, emit};
use crate::state::AppState;
use crate::sync::adopt;
use crate::sync::manifest::{
    self, Entry, Manifest, MinecraftSpec, Override, PackBase, SCHEMA_VERSION,
};
use uploads::{send_file_icons, send_files, send_icon};

fn mode_of(r: &ContentRow) -> ShareMode {
    from_wire(&r.share_mode).unwrap_or(ShareMode::Required)
}

/// Files that are there, on or off: the ones a publish looks at. Off only changes the
/// owner's own game; what friends get is each file's share mode.
fn live(rows: &[ContentRow]) -> impl Iterator<Item = &ContentRow> {
    rows.iter().filter(|r| r.installed)
}

/// On no mod site: shared means uploaded to Atlas.
fn is_upload(r: &ContentRow) -> bool {
    matches!(
        from_wire(&r.source),
        Some(ContentSource::Local | ContentSource::Upload) | None
    )
}

/// An upload exactly as the modpack has it (`ConfigPlan::pack_content`): members take it
/// from the pack's download, so it isn't uploaded.
fn from_pack(r: &ContentRow, pack: &HashMap<String, String>) -> bool {
    is_upload(r) && pack.get(&r.path).is_some_and(|sha| *sha == r.sha256)
}

/// Modrinth and CurseForge couldn't be asked about it (offline, or they failed), so
/// whether it has to be uploaded isn't known yet.
fn not_checked(r: &ContentRow) -> bool {
    r.source == "local" && r.resolved_at.is_none()
}

/// The settings that will be published: each file's own, overridden by the dialog's.
fn modes(rows: &[ContentRow], chosen: &HashMap<String, ShareMode>) -> HashMap<String, ShareMode> {
    rows.iter()
        .map(|r| {
            (
                r.id.clone(),
                chosen.get(&r.id).copied().unwrap_or(mode_of(r)),
            )
        })
        .collect()
}

/// The tree's ticks, as saved with the last share or publish.
async fn file_rules(state: &AppState, id: &str) -> AppResult<FileRules> {
    let saved: Option<(String, String, String)> = sqlx::query_as(
        "SELECT sync_include, sync_exclude, sync_first_install FROM instance WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let (include, exclude, first) = saved.unwrap_or_default();
    let list = |json: &str| serde_json::from_str::<Vec<String>>(json).unwrap_or_default();
    Ok(share_files::from_saved(
        list(&include),
        list(&exclude),
        list(&first),
    ))
}

async fn save_file_rules(state: &AppState, id: &str, rules: &FileRules) -> AppResult<()> {
    sqlx::query(
        "UPDATE instance SET sync_include = ?, sync_exclude = ?, sync_first_install = ? WHERE id = ?",
    )
    .bind(serde_json::to_string(&rules.include)?)
    .bind(serde_json::to_string(&rules.exclude)?)
    .bind(serde_json::to_string(&share_files::saved_first_install(rules))?)
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// The tree's ticks, saved from the instance's Sharing tab: they go out with the next update.
pub async fn set_file_rules(state: &AppState, id: &str, rules: &FileRules) -> AppResult<()> {
    let row = db::instances::require(&state.db, id).await?;
    if !matches!(row.role(), Some(MemberRole::Owner | MemberRole::Editor)) {
        return Err(AppError::forbidden(
            "Only the owner and editors choose which files are shared",
        ));
    }
    save_file_rules(state, id, rules).await
}

/// The manifest for these files and settings, with these config files.
fn build(
    row: &InstanceRow,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
    description: Option<String>,
    (overrides, base, pack): (Vec<Override>, Option<PackBase>, HashMap<String, String>),
    icon: Option<String>,
    file_icons: &HashMap<String, String>,
) -> AppResult<Manifest> {
    let (Some(version), Some(loader)) = (row.minecraft_version.clone(), row.loader()) else {
        return Err(AppError::invalid_input(
            "Atlas couldn't tell which Minecraft version and loader this folder uses",
        ));
    };
    let mut content = Vec::new();
    for r in live(rows) {
        let mode = modes.get(&r.id).copied().unwrap_or(ShareMode::Required);
        if mode == ShareMode::Private {
            continue;
        }
        let upload = is_upload(r);
        content.push(Entry {
            kind: from_wire(&r.kind).unwrap_or(ContentKind::Mod),
            source: if upload {
                ContentSource::Upload
            } else {
                from_wire(&r.source).unwrap_or(ContentSource::Modrinth)
            },
            // Uploads are known by their hash, not a project.
            project_id: r.project_id.clone().filter(|_| !upload),
            version_id: r.version_id.clone().filter(|_| !upload),
            name: r.name.clone(),
            version_label: r.version_label.clone(),
            path: r.path.clone(),
            sha1: r.sha1.clone(),
            sha256: r.sha256.clone(),
            size_bytes: r.size_bytes.max(0) as u64,
            download_url: r.download_url.clone().filter(|_| !upload),
            page_url: r
                .page_url
                .clone()
                .filter(|u| !upload && manifest::allowed_page(u)),
            side: from_wire(&r.side).unwrap_or(Side::Both),
            required: mode == ShareMode::Required,
            icon: file_icons.get(&r.id).cloned(),
            in_pack: from_pack(r, &pack),
        });
    }
    let mods_from_pack = content.iter().any(|e| e.in_pack);
    let files_from_pack = mods_from_pack || overrides.iter().any(|o| o.in_pack);
    let m = Manifest {
        // Apps that know only 2 can still take everything else.
        schema_version: if mods_from_pack { SCHEMA_VERSION } else { 2 },
        name: row.name.clone(),
        description,
        icon,
        minecraft: MinecraftSpec {
            version,
            loader,
            loader_version: row.loader_version.clone(),
        },
        recommended_memory_mb: None,
        content,
        // Only named when some files come from it.
        base: base.filter(|_| files_from_pack),
        overrides,
    };
    manifest::validate(&m).map_err(|e| {
        tracing::warn!(error = %e, "built a manifest that doesn't validate");
        AppError::invalid_input(format!("Atlas can't share this instance. {e}"))
    })?;
    Ok(m)
}

/// After the server took a version: save the settings, mark what's published, link it.
async fn apply_published(
    state: &AppState,
    row: &InstanceRow,
    rows: &[ContentRow],
    modes: &HashMap<String, ShareMode>,
    revision: u32,
    m: &Manifest,
) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    for r in rows {
        if let Some(mode) = modes.get(&r.id) {
            sqlx::query("UPDATE instance_content SET share_mode = ? WHERE id = ?")
                .bind(to_wire(*mode))
                .bind(&r.id)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    adopt::adopt(&state.db, &row.id, revision, m).await?;
    // "Only me" files are recorded as published that way, so they don't count as new
    // files to publish next time.
    for r in live(rows) {
        if modes.get(&r.id) == Some(&ShareMode::Private) {
            sqlx::query(
                "UPDATE instance_content SET is_managed = 0, published_share_mode = 'private'
                 WHERE id = ?",
            )
            .bind(&r.id)
            .execute(&state.db)
            .await?;
        }
    }
    Ok(())
}

/// Share it. Runs as the instance's job, so an update or restore can't run on it meanwhile.
pub async fn share<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: ShareInstanceInput,
) -> AppResult<SharedInstance> {
    let mut shared = None;
    let slot = &mut shared;
    crate::jobs::run(state, app, id, move |_| async move {
        *slot = Some(share_now(state, app, id, input).await?);
        Ok(())
    })
    .await?;
    Ok(shared.expect("set when the job succeeds"))
}

async fn share_now<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: ShareInstanceInput,
) -> AppResult<SharedInstance> {
    let row = db::instances::require(&state.db, id).await?;
    if row.shared_instance_id.is_some() {
        return Err(AppError::conflict("This instance is already shared"));
    }
    // A scan under way (a just-added instance's first) finishes first: its files are what's shared.
    drop(state.scan_lock(id).await);
    if let Some(files) = &input.files {
        save_file_rules(state, id, files).await?;
    }
    let rows = db::content::list(&state.db, id).await?;
    let modes = modes(&rows, &input.share_modes);
    let description = input.description;
    // Check the manifest builds before uploading anything.
    build(
        &row,
        &rows,
        &modes,
        description.clone(),
        (Vec::new(), None, HashMap::new()),
        None,
        &HashMap::new(),
    )?;
    let work = Reporter::new(app, id);
    let configs = send_files(
        state,
        &row,
        &rows,
        &modes,
        input.confirmed_upload_rights,
        &work,
    )
    .await?;
    let icon = send_icon(state, &row).await;
    let file_icons = send_file_icons(state, &rows, &modes).await;
    let m = build(
        &row,
        &rows,
        &modes,
        description.clone(),
        configs,
        icon,
        &file_icons,
    )?;
    work.report(WorkStage::Saving, 0, 0);
    let body = CreateSharedInstance {
        name: row.name.clone(),
        description,
        manifest: m.clone(),
        friend_ids: input.friend_ids,
        group_ids: input.group_ids,
        role: input.role,
    };
    let shared: SharedInstance = state
        .cloud
        .call(Method::POST, "/shared-instances", Some(&body))
        .await?;

    super::shared::segment(&shared.id)?;
    db::instances::set_link(&state.db, id, Some((&shared.id, MemberRole::Owner))).await?;
    apply_published(state, &row, &rows, &modes, shared.latest_revision, &m).await?;
    sqlx::query("UPDATE instance SET sync_report_due = 0 WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    super::shared::changed(state, app, &shared.id).await;
    let mut out = shared;
    out.local_instance_id = Some(id.to_owned());
    Ok(out)
}

/// Publish a new version. Runs as the instance's job, like [`share`].
pub async fn publish<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: PublishInput,
) -> AppResult<InstanceSummary> {
    crate::jobs::run(state, app, id, |_| publish_now(state, app, id, input)).await?;
    super::instances::summary_by_id(state, id).await
}

async fn publish_now<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    id: &str,
    input: PublishInput,
) -> AppResult<()> {
    let row = db::instances::require(&state.db, id).await?;
    let shared = super::shared::server_id(&row)
        .ok_or_else(|| AppError::conflict("This instance isn't shared"))?
        .to_owned();
    if !matches!(row.role(), Some(MemberRole::Owner | MemberRole::Editor)) {
        return Err(AppError::forbidden(
            "Only the owner and editors can send updates",
        ));
    }
    let base = row
        .synced_revision
        .ok_or_else(|| AppError::conflict("Update to the latest version first"))?
        .max(0) as u32;
    if let Some(files) = &input.files {
        save_file_rules(state, id, files).await?;
    }
    let rows = db::content::list(&state.db, id).await?;
    let modes = modes(&rows, &input.share_modes);
    // The shared instance's own description (what editing it changes), not the copy in
    // the last version, so publishing never brings back an old one.
    let description = match super::shared::cached(state, &shared).await? {
        Some(s) => s.description,
        None => row.synced_manifest().and_then(|m| m.description),
    };
    build(
        &row,
        &rows,
        &modes,
        description.clone(),
        (Vec::new(), None, HashMap::new()),
        None,
        &HashMap::new(),
    )?;
    let work = Reporter::new(app, id);
    let configs = send_files(
        state,
        &row,
        &rows,
        &modes,
        input.confirmed_upload_rights,
        &work,
    )
    .await?;
    // The owner's picture is the shared one; editors keep it.
    let icon = if row.role() == Some(MemberRole::Owner) {
        send_icon(state, &row).await
    } else {
        row.synced_manifest().and_then(|m| m.icon)
    };
    let file_icons = send_file_icons(state, &rows, &modes).await;
    let m = build(&row, &rows, &modes, description, configs, icon, &file_icons)?;
    if row.synced_manifest_hash.as_deref() == Some(manifest::hash(&m).as_str()) {
        return Err(AppError::invalid_input("Nothing new to send"));
    }
    work.report(WorkStage::Saving, 0, 0);
    let body = CreateRevision {
        base_revision: base,
        manifest: m.clone(),
        changelog: input.changelog,
    };
    let path = format!(
        "/shared-instances/{}/revisions",
        super::shared::segment(&shared)?
    );
    let published: RevisionSummary = state.cloud.call(Method::POST, &path, Some(&body)).await?;

    apply_published(state, &row, &rows, &modes, published.revision, &m).await?;
    sqlx::query("UPDATE instance SET sync_report_due = 0 WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    super::shared::changed(state, app, &shared).await;
    emit(
        app,
        AtlasEvent::InstanceChanged {
            instance_id: id.to_owned(),
        },
    );
    Ok(())
}
