use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use tauri::Runtime;

use super::{file_rules, share_files};
use crate::db::file_hashes::FileHash;
use crate::db::{self, instances::InstanceRow};
use crate::dto::{FileRules, LeftOutFile, LeftOutReason, MemberRole, ShareFile, WorkStage};
use crate::error::AppResult;
use crate::services::work::Reporter;
use crate::state::AppState;
use crate::sync::manifest::{
    self, MAX_OVERRIDES, MAX_OVERRIDES_TOTAL, Override, PackBase, Refusal,
};
use crate::sync::pack;
use crate::util::time::iso_from_millis;

/// The files the owner shares besides mods and packs (unticked ones left out), not
/// hashed and with no limit: for exporting a pack.
pub async fn shared_config_files(
    state: &AppState,
    row: &InstanceRow,
) -> AppResult<Vec<(String, PathBuf)>> {
    let rules = file_rules(state, &row.id).await?;
    let game = PathBuf::from(&row.game_dir);
    let listed = tokio::task::spawn_blocking(move || share_files::list(&game)).await?;
    Ok(listed
        .into_iter()
        .filter(|l| {
            share_files::shared(&rules, &l.path, false)
                && manifest::upload_refusal(&l.path, l.size).is_none()
        })
        .map(|l| (l.path, l.file))
        .collect())
}

/// What sharing does with the files besides mods and packs.
pub(super) struct ConfigPlan {
    /// The modpack the instance is from, when its download could be read.
    pub(super) base: Option<PackBase>,
    /// Shared, with the local file: `in_pack` ones come from the pack, the rest upload.
    pub(super) shared: Vec<(Override, PathBuf)>,
    /// Ticked but not shared as they are here, and why. Shown before anything uploads.
    pub(super) left_out: Vec<LeftOutFile>,
    /// Every file in the tree.
    pub(super) files: Vec<ShareFile>,
    /// The modpack's own mods and packs, path to sha256: uploads that match come from it.
    pub(super) pack_content: HashMap<String, String>,
}

fn left_out_reason(r: Refusal) -> LeftOutReason {
    match r {
        Refusal::Folder => LeftOutReason::Folder,
        Refusal::Login => LeftOutReason::Login,
        Refusal::Program => LeftOutReason::Program,
        Refusal::FileType => LeftOutReason::FileType,
        Refusal::TooBig => LeftOutReason::TooBig,
    }
}

/// Sort the ticked files: exactly as the modpack has them (referenced, not uploaded),
/// uploadable, or left out. A file the owner changed that can't be uploaded falls back
/// to the pack's own copy, so friends at least get that.
pub(super) async fn plan_configs<R: Runtime>(
    state: &AppState,
    row: &InstanceRow,
    work: &Reporter<R>,
) -> AppResult<ConfigPlan> {
    let rules = file_rules(state, &row.id).await?;
    let mut base = pack::base_for(state, row).await;
    let mut index = None;
    if let Some(b) = &base {
        let total = if pack::needs_download(&state.paths, b) {
            b.size_bytes
        } else {
            0
        };
        let mut got = 0;
        let mut progress = |n: u64| {
            got += n;
            work.report(WorkStage::Downloading, got.min(total), total);
        };
        match pack::index(state, b, &mut progress).await {
            Ok(i) => index = Some(i),
            Err(e) => {
                tracing::info!(pack = %b.name, error = %e, "couldn't read the modpack; uploading its files");
                base = None;
            }
        }
    }
    let in_pack = |rel: &str| index.as_ref().is_some_and(|i| i.contains_key(rel));
    let progress = |done, total| work.report(WorkStage::Reading, done, total);
    let (listed, hashed) = read_files(state, row, &rules, index.as_ref(), &progress, false).await?;
    let files: Vec<ShareFile> = listed
        .iter()
        .map(|l| ShareFile {
            path: l.path.clone(),
            size_bytes: l.size,
            modified: l.modified.and_then(iso_from_millis),
            from_pack: in_pack(&l.path),
            changed: false,
            refusal: manifest::upload_refusal(&l.path, l.size).map(left_out_reason),
            personal: share_files::personal(&l.path),
        })
        .collect();
    let game = PathBuf::from(&row.game_dir);

    let mut plan = ConfigPlan {
        base: base.clone(),
        shared: Vec::new(),
        left_out: Vec::new(),
        files,
        pack_content: index
            .iter()
            .flatten()
            .filter(|(path, _)| manifest::can_be_pack_content(path))
            .map(|(path, f)| (path.clone(), f.sha256.clone()))
            .collect(),
    };
    let mut uploaded = 0u64;
    let mut changed = HashSet::new();
    let full =
        |shared: usize, uploaded: u64| shared > MAX_OVERRIDES || uploaded > MAX_OVERRIDES_TOTAL;
    for FileHash {
        path: rel,
        size_bytes: size,
        sha256,
        settings: mine,
        ..
    } in hashed
    {
        let file = game.join(&rel);
        let from_pack = index.as_ref().and_then(|i| i.get(&rel)).cloned();
        // Set the same as the pack's copy: friends get that copy, nothing uploads.
        let as_is = from_pack
            .as_ref()
            .is_some_and(|p| p.sha256 == sha256 || (mine.is_some() && p.settings == mine));
        let reason = if as_is {
            None
        } else {
            manifest::upload_refusal(&rel, size)
        };
        if from_pack.is_some() && !as_is {
            changed.insert(rel.clone());
        }
        let shared = match (as_is, reason, from_pack) {
            (true, _, p) => p.map(|p| (p.sha256, p.size_bytes, true, None)),
            // Changed here, but not a file that can be uploaded: the pack's own copy.
            (false, Some(_), Some(p)) => {
                Some((p.sha256, p.size_bytes, true, Some(LeftOutReason::PackCopy)))
            }
            (false, Some(reason), None) => {
                plan.left_out.push(LeftOutFile {
                    path: rel.clone(),
                    reason: left_out_reason(reason),
                });
                None
            }
            (false, None, _) => Some((sha256, size, false, None)),
        };
        let Some((sha256, size_bytes, in_pack, note)) = shared else {
            continue;
        };
        let upload = if in_pack { 0 } else { size_bytes };
        if full(plan.shared.len() + 1, uploaded + upload) {
            plan.left_out.push(LeftOutFile {
                path: rel,
                reason: LeftOutReason::OverLimit,
            });
            continue;
        }
        uploaded += upload;
        if let Some(reason) = note {
            plan.left_out.push(LeftOutFile {
                path: rel.clone(),
                reason,
            });
        }
        plan.shared.push((
            Override {
                keep: share_files::keep(&rules, &rel),
                path: rel,
                sha256,
                size_bytes,
                in_pack,
            },
            file,
        ));
    }
    for f in &mut plan.files {
        f.changed = changed.contains(&f.path);
    }
    if !plan.left_out.is_empty() {
        tracing::info!(
            files = plan.left_out.len(),
            "ticked files not shared as they are"
        );
    }
    Ok(plan)
}

/// List the instance's files and hash the ticked ones, reusing the hashes saved last
/// time for files whose size and date haven't changed, then save them for next time.
/// `progress` gets (files read, files to read).
async fn read_files(
    state: &AppState,
    row: &InstanceRow,
    rules: &FileRules,
    index: Option<&pack::PackIndex>,
    progress: &(dyn Fn(u64, u64) + Sync),
    background: bool,
) -> AppResult<(Vec<share_files::Listed>, Vec<FileHash>)> {
    let in_pack = |rel: &str| index.is_some_and(|i| i.contains_key(rel));
    let game = PathBuf::from(&row.game_dir);
    let listed = tokio::task::spawn_blocking(move || share_files::list(&game)).await?;
    let wanted: Vec<share_files::Listed> = listed
        .iter()
        .filter(|l| share_files::shared(rules, &l.path, in_pack(&l.path)))
        .cloned()
        .collect();
    // Share reads now: a read ahead of this instance stops (what it read is in
    // Windows' file cache, so reading it again is quick). A read ahead registers itself.
    let stop = {
        let mut reading = READING.lock().unwrap_or_else(|e| e.into_inner());
        if background {
            let stop = Arc::new(AtomicBool::new(false));
            reading.insert(row.id.clone(), Arc::clone(&stop));
            Some(stop)
        } else {
            if let Some(ahead) = reading.get(&row.id) {
                ahead.store(true, Ordering::Relaxed);
            }
            None
        }
    };
    let saved = Arc::new(db::file_hashes::list(&state.db, &row.id).await?);
    let count = wanted.len() as u64;
    let done = Arc::new(AtomicU64::new(0));
    let (counter, known) = (Arc::clone(&done), Arc::clone(&saved));
    let hashing = tokio::task::spawn_blocking(move || {
        let _quiet = background.then(BackgroundPriority::start);
        share_files::hashes(wanted, &known, &counter, stop.as_deref())
    });
    tokio::pin!(hashing);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(200));
    let hashed = loop {
        tokio::select! {
            hashed = &mut hashing => break hashed?,
            _ = tick.tick() => progress(done.load(Ordering::Relaxed), count),
        }
    };
    if background {
        READING
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&row.id);
    }
    let hashed = hashed?;
    let same = hashed.len() == saved.len() && hashed.iter().all(|h| saved.get(&h.path) == Some(h));
    if !same {
        db::file_hashes::replace(&state.db, &row.id, &hashed).await?;
    }
    Ok((listed, hashed))
}

/// Windows' background mode for this thread while it lives: lower CPU and disk priority,
/// so reading ahead doesn't slow the game or the PC down.
struct BackgroundPriority;

impl BackgroundPriority {
    fn start() -> Self {
        #[cfg(windows)]
        // SAFETY: changes only the calling thread's own priority; undone in drop.
        unsafe {
            use windows_sys::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
            };
            SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
        }
        BackgroundPriority
    }
}

impl Drop for BackgroundPriority {
    fn drop(&mut self) {
        #[cfg(windows)]
        // SAFETY: as in start; the thread returns to the blocking pool at normal priority.
        unsafe {
            use windows_sys::Win32::System::Threading::{
                GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_END,
            };
            SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
        }
    }
}

/// The latest rescan per instance: a read ahead waits for rescans to stop.
static READ_AHEAD: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(Default::default);
static ONE_READ_AHEAD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Read aheads running, by instance: setting the flag stops one.
static READING: LazyLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = LazyLock::new(Default::default);

/// `read_ahead` once the instance has been quiet for a while: rescans come in bursts,
/// and Share's own rescan shouldn't race Share's own read.
pub fn read_ahead_soon(state: &AppState, id: &str) {
    let seen = {
        let mut latest = READ_AHEAD.lock().unwrap_or_else(|e| e.into_inner());
        let n = latest.entry(id.to_owned()).or_default();
        *n += 1;
        *n
    };
    let (state, id) = (state.clone(), id.to_owned());
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        let current = READ_AHEAD
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .copied();
        if current == Some(seen) {
            // One at a time: after startup every instance asks at once.
            let _one = ONE_READ_AHEAD.lock().await;
            read_ahead(&state, &id).await;
        }
    });
}

/// Read an instance's shared files ahead of Share, so the dialog opens on saved hashes.
/// Skipped while a game runs (it's busy writing them), for instances the user can't
/// publish, and when the modpack isn't read yet (Share downloads it first).
pub async fn read_ahead(state: &AppState, id: &str) {
    if state.running.any() {
        return;
    }
    let Ok(row) = db::instances::require(&state.db, id).await else {
        return;
    };
    if row.shared_instance_id.is_some() && row.role() != Some(MemberRole::Owner) {
        return;
    }
    let index = match pack::base_for(state, &row).await {
        Some(base) => match pack::saved_index(&state.paths, &base) {
            Some(index) => Some(index),
            None => return,
        },
        None => None,
    };
    let started = std::time::Instant::now();
    let rules = match file_rules(state, id).await {
        Ok(rules) => rules,
        Err(e) => return tracing::debug!(error = %e, "couldn't read share rules"),
    };
    match read_files(state, &row, &rules, index.as_ref(), &|_, _| {}, true).await {
        Ok((_, hashed)) => tracing::debug!(
            instance = %row.name,
            files = hashed.len(),
            ms = started.elapsed().as_millis() as u64,
            "read shared files ahead"
        ),
        Err(e) => {
            tracing::debug!(instance = %row.name, error = %e, "couldn't read shared files ahead")
        }
    }
}
