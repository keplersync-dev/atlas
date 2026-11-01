//! The files an owner can share besides mods and packs: listing the instance folder for
//! the share dialog's tree, and deciding which are ticked.
//!
//! Ticks are rules: paths ticked (`include`) or unticked (`exclude`), where a folder ends
//! in '/' and covers what's inside, and "" covers everything. The longest matching rule
//! wins. With none, the defaults decide: the config and script folders, and the files
//! the modpack itself ships. A folder ticked once stays ticked for files added later.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::db::file_hashes::FileHash;
use crate::dto::FileRules;
use crate::sync::manifest::{CONTENT_DIRS, Keep, NEVER_SHARED};

/// Ticked by default, like the Modrinth App's whole `config/`, plus the folders packs
/// keep their scripts in (SkyFactory 4 needs its CraftTweaker scripts).
pub const DEFAULT_DIRS: [&str; 4] = ["config/", "defaultconfigs/", "kubejs/", "scripts/"];

/// Never listed, as in the Modrinth App's export: caches, native libraries mods unpack,
/// and other programs' files.
const HIDDEN: [&str; 12] = [
    "modrinth_logs/",
    ".sable/natives/",
    "local/crash_assistant/",
    "config/super_resolution/libraries/",
    "config/veinminer/update/",
    "config/epicfight/native/",
    "__macosx/",
    ".tandem/",
    "usercache.json",
    "usernamecache.json",
    ".ds_store",
    "desktop.ini",
];

/// The player's own settings: a member gets the owner's copy once, then keeps theirs
/// (as the Modrinth App keeps players' options across modpack updates).
const FIRST_INSTALL: [&str; 1] = ["options.txt"];

fn listed(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    !CONTENT_DIRS
        .iter()
        .chain(&NEVER_SHARED)
        .chain(&HIDDEN)
        .any(|p| lower.starts_with(p) || name == *p)
}

/// A file the tree lists.
#[derive(Debug, Clone)]
pub struct Listed {
    /// "config/create/server.toml"
    pub path: String,
    pub file: PathBuf,
    pub size: u64,
    /// Milliseconds since 1970, when known.
    pub modified: Option<i64>,
}

/// Every file the tree lists, sorted by path. Hidden folders aren't walked at all (a
/// world can hold tens of thousands of files).
pub fn list(game: &Path) -> Vec<Listed> {
    let mut out = Vec::new();
    walk(game, "", &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Each file's sha256 and settings, reusing `saved` ones whose size and modified time
/// still match, so only changed files are read. `done` counts files, for progress;
/// setting `stop` ends it early with an `Interrupted` error. Blocking: call it from a
/// blocking thread.
pub fn hashes(
    files: Vec<Listed>,
    saved: &HashMap<String, FileHash>,
    done: &AtomicU64,
    stop: Option<&AtomicBool>,
) -> std::io::Result<Vec<FileHash>> {
    files
        .into_iter()
        .map(|l| {
            if stop.is_some_and(|s| s.load(Ordering::Relaxed)) {
                return Err(std::io::ErrorKind::Interrupted.into());
            }
            // Saved before its kind was compared by settings: read once more.
            let known = saved.get(&l.path).filter(|h| {
                l.modified == Some(h.modified_ms)
                    && l.size == h.size_bytes
                    && (h.settings.is_some() || !crate::sync::settings::compared(&l.path))
            });
            let hash = match known {
                Some(h) => h.clone(),
                None => {
                    let bytes = std::fs::read(&l.file)?;
                    FileHash {
                        sha256: hex::encode(Sha256::digest(&bytes)),
                        settings: crate::sync::settings::fingerprint(&l.path, &bytes),
                        size_bytes: bytes.len() as u64,
                        // Unknown: never matches, so it's read again next time.
                        modified_ms: l.modified.unwrap_or(-1),
                        path: l.path,
                    }
                }
            };
            done.fetch_add(1, Ordering::Relaxed);
            Ok(hash)
        })
        .collect()
}

/// Folder links (symlinks, junctions) aren't followed: one pointing back up would never end.
fn walk(dir: &Path, rel: &str, out: &mut Vec<Listed>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        if kind.is_symlink() {
            continue;
        } else if kind.is_dir() {
            if listed(&format!("{child}/")) {
                walk(&entry.path(), &child, out);
            }
        } else if kind.is_file()
            && listed(&child)
            && let Ok(meta) = entry.metadata()
        {
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64);
            out.push(Listed {
                path: child,
                file: entry.path(),
                size: meta.len(),
                modified,
            });
        }
    }
}

/// Whether `path` is shared. `from_pack`: the modpack ships it (ticked by default).
pub fn shared(rules: &FileRules, path: &str, from_pack: bool) -> bool {
    let covers = |rule: &String| {
        *rule == path || rule.is_empty() || rule.ends_with('/') && path.starts_with(rule.as_str())
    };
    let best = |list: &[String]| list.iter().filter(|r| covers(r)).map(String::len).max();
    match (best(&rules.include), best(&rules.exclude)) {
        (Some(i), Some(e)) => i > e,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => from_pack || DEFAULT_DIRS.iter().any(|d| path.starts_with(d)),
    }
}

/// How members' copies of a shared file are kept.
pub fn keep(rules: &FileRules, path: &str) -> Option<Keep> {
    if path == "servers.dat" {
        Some(Keep::MergeServers)
    } else if FIRST_INSTALL.contains(&path) || rules.first_install.iter().any(|p| p == path) {
        Some(Keep::FirstInstall)
    } else {
        None
    }
}

/// Saved rules from before the tree: `sync_include` held globs ("config/**").
pub fn from_saved(
    include: Vec<String>,
    exclude: Vec<String>,
    first_install: Vec<String>,
) -> FileRules {
    let prefix = |r: String| match r.strip_suffix("**") {
        Some(dir) => dir.to_owned(),
        None => r,
    };
    FileRules {
        include: include.into_iter().map(prefix).collect(),
        exclude: exclude.into_iter().map(prefix).collect(),
        first_install,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How long Share's file work takes on a real instance. ATLAS_TEST_INSTANCE names the
    /// folder. `cargo test --release --lib time_share_files -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn time_share_files() {
        use std::time::Instant;
        let game =
            PathBuf::from(std::env::var("ATLAS_TEST_INSTANCE").expect("ATLAS_TEST_INSTANCE"));
        let t = Instant::now();
        let listed = list(&game);
        println!("list: {} files in {:?}", listed.len(), t.elapsed());
        let wanted: Vec<Listed> = listed
            .into_iter()
            .filter(|l| {
                shared(
                    &FileRules::default(),
                    &l.path,
                    l.path.starts_with("global_packs/"),
                )
            })
            .collect();
        let t = Instant::now();
        let read = hashes(wanted.clone(), &HashMap::new(), &AtomicU64::new(0), None).unwrap();
        println!("read and hash {} files: {:?}", read.len(), t.elapsed());
        let saved: HashMap<String, FileHash> =
            read.into_iter().map(|h| (h.path.clone(), h)).collect();
        let t = Instant::now();
        hashes(wanted, &saved, &AtomicU64::new(0), None).unwrap();
        println!("again, from saved hashes: {:?}", t.elapsed());
    }

    #[test]
    fn saved_hashes_are_reused_until_the_file_changes() {
        let dir = std::env::temp_dir().join(format!("atlas-hashes-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(dir.join("config/a.toml"), "a = 1").unwrap();
        let first = hashes(list(&dir), &HashMap::new(), &AtomicU64::new(0), None).unwrap();
        assert_eq!(first.len(), 1);
        assert!(first[0].settings.is_some());
        // Same size and date: the saved hash is used without reading the file.
        let mut saved = first[0].clone();
        saved.sha256 = "saved".into();
        let saved = HashMap::from([(saved.path.clone(), saved)]);
        assert_eq!(
            hashes(list(&dir), &saved, &AtomicU64::new(0), None).unwrap()[0].sha256,
            "saved"
        );
        // Stopped (Share took over from a read ahead): nothing comes back.
        let stop = AtomicBool::new(true);
        let stopped = hashes(list(&dir), &HashMap::new(), &AtomicU64::new(0), Some(&stop));
        assert_eq!(stopped.unwrap_err().kind(), std::io::ErrorKind::Interrupted);
        // Changed: read again.
        std::fs::write(dir.join("config/a.toml"), "a = 22").unwrap();
        assert_eq!(
            hashes(list(&dir), &saved, &AtomicU64::new(0), None).unwrap()[0]
                .sha256
                .len(),
            64
        );
    }

    fn rules(include: &[&str], exclude: &[&str]) -> FileRules {
        FileRules {
            include: include.iter().map(|s| s.to_string()).collect(),
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
            first_install: vec![],
        }
    }

    #[test]
    fn defaults_tick_configs_scripts_and_the_packs_own_files() {
        let none = rules(&[], &[]);
        assert!(shared(&none, "config/create/server.toml", false));
        assert!(shared(&none, "scripts/recipes.zs", false));
        assert!(
            shared(&none, "global_packs/a/pack.mcmeta", true),
            "from the pack"
        );
        assert!(!shared(&none, "journeymap/data.json", false));
        assert!(!shared(&none, "options.txt", false));
    }

    #[test]
    fn the_longest_rule_wins() {
        let r = rules(
            &["config/create/server.toml", "journeymap/"],
            &["config/create/"],
        );
        assert!(!shared(&r, "config/create/client.toml", false));
        assert!(
            shared(&r, "config/create/server.toml", false),
            "file beats folder"
        );
        assert!(shared(&r, "journeymap/data.json", false));
        assert!(
            shared(&r, "config/sodium.json", false),
            "default still applies"
        );
        let all_off = rules(&[], &[""]);
        assert!(!shared(&all_off, "config/sodium.json", false));
        assert!(!shared(&all_off, "global_packs/a.json", true));
        let all_on = rules(&[""], &[]);
        assert!(shared(&all_on, "journeymap/data.json", false));
    }

    #[test]
    fn old_saved_rules_still_read() {
        let r = from_saved(
            vec!["config/**".into()],
            vec!["config/create/".into()],
            vec![],
        );
        assert_eq!(r.include, ["config/"]);
        assert!(!shared(&r, "config/create/a.toml", false));
    }

    #[test]
    fn personal_files_are_kept_as_modrinth_keeps_them() {
        let none = rules(&[], &[]);
        assert_eq!(keep(&none, "options.txt"), Some(Keep::FirstInstall));
        assert_eq!(keep(&none, "servers.dat"), Some(Keep::MergeServers));
        assert_eq!(keep(&none, "config/a.json"), None);
        let mine = FileRules {
            first_install: vec!["optionsof.txt".into()],
            ..none
        };
        assert_eq!(keep(&mine, "optionsof.txt"), Some(Keep::FirstInstall));
    }

    #[test]
    fn the_tree_leaves_out_what_is_never_shared() {
        let dir = std::env::temp_dir().join(format!("atlas-tree-{}", crate::util::ids::new_id()));
        for f in [
            "config/a.json",
            "options.txt",
            "mods/sodium.jar",
            "saves/World/level.dat",
            "logs/latest.log",
            "usercache.json",
            "config/epicfight/native/lib.dll",
            "global_packs/x/pack.mcmeta",
        ] {
            let p = dir.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        }
        let paths: Vec<String> = list(&dir).into_iter().map(|l| l.path).collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            paths,
            ["config/a.json", "global_packs/x/pack.mcmeta", "options.txt"]
        );
    }
}
