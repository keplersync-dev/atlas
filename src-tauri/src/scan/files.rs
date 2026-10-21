//! Scan an instance's content folders and bring `instance_content` up to date.
//!
//! - Files in mods/ (.jar), resourcepacks/, shaderpacks/, datapacks/ (.zip), top level only.
//!   Datapacks inside worlds are personal; pack folders can't be hashed as one file.
//! - "x.jar.disabled" is stored as "mods/x.jar" with enabled = false (every launcher does this).
//! - A file whose path, size and modified time haven't changed keeps its hashes.
//! - Rows keep their ids across scans (the UI keys on them) and their share settings.
//! - A missing managed optional file means "not installed", not deleted (tauri-plan §9.1).
//! - Jars are read for their name, version and mod id (metadata.rs). A mod updated in the
//!   launcher (old jar gone, new jar with the same mod id, in one scan) keeps its share setting.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sqlx::SqlitePool;

use super::hash::{Hashes, hash_file};
use super::metadata::{ModInfo, read_jar};
use crate::db::content::{self, ContentRow, FileFacts, NewFile};
use crate::dto::{ContentKind, ShareMode, from_wire, to_wire};
use crate::error::{AppError, AppResult};

const FOLDERS: [(&str, ContentKind, &str); 4] = [
    ("mods", ContentKind::Mod, "jar"),
    ("resourcepacks", ContentKind::Resourcepack, "zip"),
    ("shaderpacks", ContentKind::Shaderpack, "zip"),
    ("datapacks", ContentKind::Datapack, "zip"),
];

/// Folders whose changes matter to a scan (the watcher uses this too).
pub fn is_content_folder(name: &str) -> bool {
    FOLDERS.iter().any(|(f, _, _)| f.eq_ignore_ascii_case(name))
}

/// A content file on disk.
#[derive(Debug, Clone)]
pub struct DiskFile {
    /// Relative, forward slashes, without ".disabled": "mods/sodium.jar"
    pub path: String,
    pub absolute: PathBuf,
    pub kind: ContentKind,
    pub enabled: bool,
    pub size: u64,
    pub mtime_ms: i64,
}

/// List the content files of a game dir. Blocking.
pub fn list_files(game_dir: &Path) -> std::io::Result<Vec<DiskFile>> {
    let mut by_path: HashMap<String, DiskFile> = HashMap::new();
    for (folder, kind, ext) in FOLDERS {
        let dir = game_dir.join(folder);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let (name, enabled) = match strip_suffix_ignore_case(&file_name, ".disabled") {
                Some(n) => (n.to_string(), false),
                None => (file_name.clone(), true),
            };
            if !has_extension(&name, ext) {
                continue;
            }
            let file = DiskFile {
                path: format!("{folder}/{name}"),
                absolute: entry.path(),
                kind,
                enabled,
                size: meta.len(),
                mtime_ms: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis() as i64),
            };
            // "x.jar" and "x.jar.disabled" side by side: the enabled one is what the game loads.
            match by_path.get(&file.path.to_lowercase()) {
                Some(existing) if existing.enabled => {
                    tracing::warn!(path = %file.path, "both enabled and disabled copies exist; using the enabled one");
                }
                _ => {
                    by_path.insert(file.path.to_lowercase(), file);
                }
            }
        }
    }
    Ok(by_path.into_values().collect())
}

fn strip_suffix_ignore_case<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    let cut = s.len().checked_sub(suffix.len())?;
    (s.is_char_boundary(cut) && s[cut..].eq_ignore_ascii_case(suffix)).then(|| &s[..cut])
}

fn has_extension(name: &str, ext: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// What a file is called until it's identified (T3 gives it a real name).
fn display_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// A hashed file plus what it says about itself (jars only).
struct Read {
    hashes: Hashes,
    info: ModInfo,
}

/// Hash (and, for jars, read) files on up to 4 threads. Blocking. Results match `files`.
fn read_all(files: &[PathBuf]) -> Vec<std::io::Result<Read>> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<std::io::Result<Read>>>> =
        Mutex::new((0..files.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..4.min(files.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = files.get(i) else { break };
                    let r = hash_file(path).map(|hashes| Read {
                        info: if has_extension(&path.to_string_lossy(), "jar") {
                            read_jar(path)
                        } else {
                            ModInfo::default()
                        },
                        hashes,
                    });
                    results.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|r| r.expect("every file hashed"))
        .collect()
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScanResult {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}

impl ScanResult {
    pub fn changed(&self) -> bool {
        self.added + self.updated + self.removed > 0
    }
}

/// Bring the instance's rows in line with its folder. Does nothing if the folder is missing.
pub async fn scan(db: &SqlitePool, instance_id: &str, game_dir: &Path) -> AppResult<ScanResult> {
    if !game_dir.is_dir() {
        return Ok(ScanResult::default());
    }
    let dir = game_dir.to_path_buf();
    let files = blocking(move || list_files(&dir)).await??;
    let rows = content::list(db, instance_id).await?;
    let by_path: HashMap<String, &ContentRow> =
        rows.iter().map(|r| (r.path.to_lowercase(), r)).collect();

    // Hash only what's new or changed since the last scan.
    let unchanged = |f: &DiskFile, r: &ContentRow| {
        r.installed && r.size_bytes == f.size as i64 && r.mtime_ms == Some(f.mtime_ms)
    };
    let to_hash: Vec<&DiskFile> = files
        .iter()
        .filter(|f| {
            by_path
                .get(&f.path.to_lowercase())
                .is_none_or(|r| !unchanged(f, r))
        })
        .collect();
    let paths: Vec<PathBuf> = to_hash.iter().map(|f| f.absolute.clone()).collect();
    let hashes = blocking(move || read_all(&paths)).await?;
    let hashed: HashMap<String, Read> = to_hash
        .iter()
        .zip(hashes)
        .filter_map(|(f, h)| match h {
            Ok(h) => Some((f.path.to_lowercase(), h)),
            Err(e) => {
                // In use or deleted mid-scan: try again next scan.
                tracing::debug!(path = %f.path, error = %e, "couldn't hash file");
                None
            }
        })
        .collect();

    // Rows whose file is gone this scan, by mod id: an updated mod's new jar takes over
    // the old one's share setting.
    let on_disk: std::collections::HashSet<String> =
        files.iter().map(|f| f.path.to_lowercase()).collect();
    let gone: Vec<&ContentRow> = rows
        .iter()
        .filter(|r| r.installed && !on_disk.contains(&r.path.to_lowercase()))
        .collect();
    let inherited: HashMap<(&str, &str), &str> = gone
        .iter()
        .filter_map(|r| {
            Some((
                (r.kind.as_str(), r.mod_id.as_deref()?),
                r.share_mode.as_str(),
            ))
        })
        .collect();

    let mut result = ScanResult::default();
    let mut tx = db.begin().await?;
    for f in &files {
        let key = f.path.to_lowercase();
        match (by_path.get(&key), hashed.get(&key)) {
            (Some(r), _) if unchanged(f, r) => {
                if r.enabled != f.enabled {
                    content::touch_file(&mut tx, &r.id, f.enabled, f.size as i64, f.mtime_ms)
                        .await?;
                    result.updated += 1;
                }
            }
            (Some(r), Some(read)) => {
                let h = &read.hashes;
                if r.sha1 == h.sha1 {
                    // Same bytes, new timestamp (copied, touched) or re-enabled.
                    content::touch_file(&mut tx, &r.id, f.enabled, f.size as i64, f.mtime_ms)
                        .await?;
                    if r.enabled != f.enabled || !r.installed {
                        result.updated += 1;
                    }
                } else {
                    let name = read
                        .info
                        .name
                        .clone()
                        .unwrap_or_else(|| display_name(&f.path));
                    content::replace_file(&mut tx, &r.id, facts(f, read, &name)).await?;
                    result.updated += 1;
                }
            }
            (None, Some(read)) => {
                let kind = to_wire(f.kind);
                let name = read
                    .info
                    .name
                    .clone()
                    .unwrap_or_else(|| display_name(&f.path));
                let share_mode = read
                    .info
                    .mod_id
                    .as_deref()
                    .and_then(|id| inherited.get(&(kind.as_str(), id)).copied())
                    .unwrap_or("required");
                content::insert_file(
                    &mut tx,
                    NewFile {
                        instance_id,
                        kind: &kind,
                        path: &f.path,
                        facts: facts(f, read, &name),
                        share_mode,
                    },
                )
                .await?;
                result.added += 1;
            }
            (_, None) => {} // couldn't hash this time
        }
    }

    // Rows whose file is gone.
    for r in gone {
        let optional = from_wire::<ShareMode>(&r.share_mode) == Some(ShareMode::Optional);
        if r.is_managed && optional {
            content::mark_not_installed(&mut tx, &r.id).await?;
        } else {
            content::delete(&mut tx, &r.id).await?;
        }
        result.removed += 1;
    }
    tx.commit().await?;
    Ok(result)
}

fn facts<'a>(f: &DiskFile, read: &'a Read, name: &'a str) -> FileFacts<'a> {
    FileFacts {
        enabled: f.enabled,
        size_bytes: f.size as i64,
        mtime_ms: f.mtime_ms,
        sha1: &read.hashes.sha1,
        sha256: &read.hashes.sha256,
        murmur2: read.hashes.murmur2 as i64,
        name,
        version_label: read.info.version.as_deref(),
        author: read.info.authors.as_deref(),
        mod_id: read.info.mod_id.as_deref(),
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> AppResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::unknown(format!("Scan failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    struct Fixture {
        dir: PathBuf,
        pool: SqlitePool,
        id: String,
    }

    impl Fixture {
        async fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("atlas-scan-{}", crate::util::ids::new_id()));
            std::fs::create_dir_all(dir.join("mods")).unwrap();
            std::fs::create_dir_all(dir.join("resourcepacks")).unwrap();
            let pool = db::open_in_memory().await.unwrap();
            let row = db::instances::insert(
                &pool,
                db::instances::NewInstance {
                    launcher_id: None,
                    kind: crate::dto::LauncherKind::Folder,
                    launcher_instance_id: None,
                    game_dir: dir.to_string_lossy().into_owned(),
                    meta: db::instances::Metadata {
                        name: "t".into(),
                        icon: None,
                        minecraft_version: None,
                        loader: None,
                        loader_version: None,
                        last_played_at: None,
                        total_playtime_seconds: None,
                    },
                },
            )
            .await
            .unwrap();
            Self {
                dir,
                pool,
                id: row.id,
            }
        }
        fn write(&self, rel: &str, bytes: &[u8]) {
            std::fs::write(self.dir.join(rel), bytes).unwrap();
        }
        async fn scan(&self) -> ScanResult {
            scan(&self.pool, &self.id, &self.dir).await.unwrap()
        }
        async fn rows(&self) -> Vec<ContentRow> {
            let mut rows = content::list(&self.pool, &self.id).await.unwrap();
            rows.sort_by(|a, b| a.path.cmp(&b.path));
            rows
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[tokio::test]
    async fn finds_content_and_ignores_everything_else() {
        let f = Fixture::new().await;
        f.write("mods/sodium-0.6.0.jar", b"sodium");
        f.write("mods/iris-1.8.jar.disabled", b"iris");
        f.write("mods/readme.txt", b"not a mod");
        f.write("resourcepacks/Fresh Animations.zip", b"fa");
        f.write("options.txt", b"personal");
        std::fs::create_dir_all(f.dir.join("resourcepacks/Unzipped pack")).unwrap();

        assert_eq!(
            f.scan().await,
            ScanResult {
                added: 3,
                updated: 0,
                removed: 0
            }
        );
        let rows = f.rows().await;
        let paths: Vec<_> = rows
            .iter()
            .map(|r| (r.path.as_str(), r.enabled, r.kind.as_str()))
            .collect();
        assert_eq!(
            paths,
            [
                ("mods/iris-1.8.jar", false, "mod"),
                ("mods/sodium-0.6.0.jar", true, "mod"),
                ("resourcepacks/Fresh Animations.zip", true, "resourcepack"),
            ]
        );
        let sodium = &rows[1];
        assert_eq!(sodium.name, "sodium-0.6.0");
        assert_eq!(sodium.source, "local");
        assert_eq!(sodium.share_mode, "required");
        assert!(!sodium.is_managed && sodium.installed);
        assert_eq!(sodium.size_bytes, 6);
        assert_eq!(sodium.sha1.len(), 40);

        // Nothing changed: nothing to do.
        assert!(!f.scan().await.changed());
    }

    #[tokio::test]
    async fn keeps_ids_and_settings_across_changes() {
        let f = Fixture::new().await;
        f.write("mods/a.jar", b"one");
        f.scan().await;
        let before = f.rows().await.remove(0);
        content::set_share_mode(&f.pool, &before.id, "optional")
            .await
            .unwrap();

        // Disabled in the launcher: same row, enabled = false.
        std::fs::rename(f.dir.join("mods/a.jar"), f.dir.join("mods/a.jar.disabled")).unwrap();
        assert_eq!(f.scan().await.updated, 1);
        let row = f.rows().await.remove(0);
        assert_eq!((row.id.as_str(), row.enabled), (before.id.as_str(), false));

        // Replaced with different bytes: same row and setting, new hash.
        std::fs::remove_file(f.dir.join("mods/a.jar.disabled")).unwrap();
        f.write("mods/a.jar", b"two, longer");
        assert_eq!(f.scan().await.updated, 1);
        let row = f.rows().await.remove(0);
        assert_eq!(row.id, before.id);
        assert_eq!(row.share_mode, "optional");
        assert_ne!(row.sha1, before.sha1);
        assert!(row.enabled);

        // Deleted: the row goes (it wasn't from a shared version).
        std::fs::remove_file(f.dir.join("mods/a.jar")).unwrap();
        assert_eq!(f.scan().await.removed, 1);
        assert!(f.rows().await.is_empty());
    }

    #[tokio::test]
    async fn a_removed_managed_optional_file_becomes_not_installed() {
        let f = Fixture::new().await;
        f.write("mods/iris.jar", b"iris");
        f.write("mods/req.jar", b"req");
        f.scan().await;
        // Pretend both came from a shared version: one optional, one required.
        sqlx::query("UPDATE instance_content SET is_managed = 1, share_mode = CASE WHEN path LIKE '%iris%' THEN 'optional' ELSE 'required' END")
            .execute(&f.pool)
            .await
            .unwrap();

        std::fs::remove_file(f.dir.join("mods/iris.jar")).unwrap();
        std::fs::remove_file(f.dir.join("mods/req.jar")).unwrap();
        assert_eq!(f.scan().await.removed, 2);
        let rows = f.rows().await;
        assert_eq!(
            rows.len(),
            1,
            "the required one is gone (a local change, found via the synced manifest)"
        );
        assert_eq!(
            (rows[0].path.as_str(), rows[0].installed),
            ("mods/iris.jar", false)
        );

        // Not installed stays that way on later scans; putting the file back installs it.
        assert!(!f.scan().await.changed());
        f.write("mods/iris.jar", b"iris");
        assert_eq!(f.scan().await.updated, 1);
        assert!(f.rows().await[0].installed);
    }

    fn write_jar(f: &Fixture, rel: &str, mod_id: &str, name: &str, version: &str) {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(std::fs::File::create(f.dir.join(rel)).unwrap());
        z.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        let json = format!(
            r#"{{"id":"{mod_id}","name":"{name}","version":"{version}","authors":["Someone"]}}"#
        );
        z.write_all(json.as_bytes()).unwrap();
        z.finish().unwrap();
    }

    #[tokio::test]
    async fn jars_get_their_own_name_and_updates_keep_the_share_setting() {
        let f = Fixture::new().await;
        write_jar(&f, "mods/sodium-0.5.jar", "sodium", "Sodium", "0.5.0");
        f.write("resourcepacks/Faithful 32x.zip", b"pack");
        f.scan().await;
        let rows = f.rows().await;
        let sodium = &rows[0];
        assert_eq!(sodium.name, "Sodium");
        assert_eq!(sodium.version_label.as_deref(), Some("0.5.0"));
        assert_eq!(sodium.author.as_deref(), Some("Someone"));
        assert_eq!(sodium.mod_id.as_deref(), Some("sodium"));
        assert_eq!(rows[1].name, "Faithful 32x", "packs keep their file name");
        content::set_share_mode(&f.pool, &sodium.id, "optional")
            .await
            .unwrap();

        // Updated in the launcher: old jar replaced by a new file name, same mod id.
        std::fs::remove_file(f.dir.join("mods/sodium-0.5.jar")).unwrap();
        write_jar(&f, "mods/sodium-0.6.jar", "sodium", "Sodium", "0.6.0");
        write_jar(&f, "mods/lithium.jar", "lithium", "Lithium", "1.0");
        assert_eq!(
            f.scan().await,
            ScanResult {
                added: 2,
                updated: 0,
                removed: 1
            }
        );
        let rows = f.rows().await;
        let new = rows
            .iter()
            .find(|r| r.path == "mods/sodium-0.6.jar")
            .unwrap();
        assert_eq!(new.version_label.as_deref(), Some("0.6.0"));
        assert_eq!(new.share_mode, "optional", "carried over from 0.5");
        assert_eq!(
            new.published_share_mode, None,
            "the new jar itself was never published"
        );
        let lithium = rows.iter().find(|r| r.path == "mods/lithium.jar").unwrap();
        assert_eq!(
            lithium.share_mode, "required",
            "unrelated new mods start Required"
        );
    }

    #[tokio::test]
    async fn missing_folder_changes_nothing() {
        let f = Fixture::new().await;
        f.write("mods/a.jar", b"a");
        f.scan().await;
        let gone = f.dir.join("gone");
        assert!(!scan(&f.pool, &f.id, &gone).await.unwrap().changed());
        assert_eq!(f.rows().await.len(), 1);
    }
}

#[cfg(test)]
mod real {
    /// Scans every instance in this PC's launchers into a throwaway in-memory database:
    /// reads files only, writes nothing. Prints counts and how long hashing took.
    /// `cargo test --lib real_instance_scan -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn real_instance_scan() {
        use crate::{db, launchers};
        let pool = db::open_in_memory().await.unwrap();
        for kind in launchers::KINDS {
            for root in launchers::detect(kind) {
                for found in launchers::list_instances(kind, &root)
                    .await
                    .unwrap_or_default()
                {
                    let row = db::instances::insert(
                        &pool,
                        db::instances::NewInstance {
                            launcher_id: None,
                            kind,
                            launcher_instance_id: Some(found.launcher_instance_id.clone()),
                            game_dir: found.game_dir.to_string_lossy().into_owned(),
                            meta: db::instances::Metadata {
                                name: found.name.clone(),
                                icon: None,
                                minecraft_version: None,
                                loader: None,
                                loader_version: None,
                                last_played_at: None,
                                total_playtime_seconds: None,
                            },
                        },
                    )
                    .await
                    .unwrap();
                    let start = std::time::Instant::now();
                    let first = super::scan(&pool, &row.id, &found.game_dir).await.unwrap();
                    let first_time = start.elapsed();
                    let start = std::time::Instant::now();
                    let second = super::scan(&pool, &row.id, &found.game_dir).await.unwrap();
                    let rows = db::content::list(&pool, &row.id).await.unwrap();
                    let mb: i64 = rows.iter().map(|r| r.size_bytes).sum::<i64>() / 1_048_576;
                    let disabled = rows.iter().filter(|r| !r.enabled).count();
                    println!(
                        "{:38} {:4} files {:5} MB {:3} disabled  first {:>6.2?}  again {:>6.2?} (changed: {})",
                        found.name,
                        first.added,
                        mb,
                        disabled,
                        first_time,
                        start.elapsed(),
                        second.changed()
                    );
                }
            }
        }
    }
}
