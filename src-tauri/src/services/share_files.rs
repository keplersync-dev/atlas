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
use sqlx::SqlitePool;

use crate::db::file_hashes::FileHash;
use crate::dto::{FileRules, Personal, PersonalKeep};
use crate::error::AppResult;
use crate::sync::manifest::{CONTENT_DIRS, Keep, NEVER_SHARED};

/// Ticked by default, like the Modrinth App's whole `config/`, plus the folders packs
/// keep their scripts and loose textures in (SkyFactory 4 needs its CraftTweaker scripts,
/// and its menu art in `resources/`).
pub const DEFAULT_DIRS: [&str; 5] = [
    "config/",
    "defaultconfigs/",
    "kubejs/",
    "resources/",
    "scripts/",
];

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

use PersonalKeep::{Once, OnlyMe};

/// Files that hold one player's own settings or data, lower case, a folder ending in '/'.
/// `Once`: video, shader, key and HUD settings, which friends tune for their own PC.
/// `OnlyMe`: what's tied to this PC or this player (device names, keys, bookmarks, map
/// data, caches). Found on 24 real instances and checked against the mods' own files;
/// the sources are in the vault (research/research-personal-files.md).
const PERSONAL: [(&str, PersonalKeep, &str); 68] = [
    ("options.txt", Once, "keys, video and sound"),
    ("optionsof.txt", Once, "video settings"),
    ("optionsshaders.txt", Once, "shader choice"),
    ("config/sodium-options.json", Once, "video settings"),
    ("config/sodium-extra-options.json", Once, "video settings"),
    ("config/embeddium-options.json", Once, "video settings"),
    ("config/rubidium-options.json", Once, "video settings"),
    ("config/iris.properties", Once, "shader choice"),
    ("config/oculus.properties", Once, "shader choice"),
    (
        "config/distanthorizons.toml",
        Once,
        "Distant Horizons settings",
    ),
    ("config/dynamic_fps.json", Once, "frame rate settings"),
    ("config/xaerominimap.txt", Once, "minimap settings"),
    ("config/xaeroworldmap.txt", Once, "world map settings"),
    ("config/xaeroplus.txt", Once, "map settings"),
    ("journeymap/config/", Once, "map settings"),
    ("config/fzzy_config/keybinds.toml", Once, "key bindings"),
    ("config/ichunutil_keybinds.cfg", Once, "key bindings"),
    ("config/minihud.json", Once, "hotkeys and HUD settings"),
    ("config/tweakeroo.json", Once, "hotkeys and tweaks"),
    (
        "config/litematica.json",
        Once,
        "Litematica hotkeys and settings",
    ),
    (
        "config/controllable-client.toml",
        Once,
        "controller settings",
    ),
    (
        "config/presencefootsteps/userconfig.json",
        Once,
        "footstep sound settings",
    ),
    ("config/brandon3055/hud_settings.json", Once, "HUD layout"),
    ("config/combat_roll/hud_config.json", Once, "HUD layout"),
    (
        "config/pneumaticcraft/pneumaticarmorhudlayout.cfg",
        Once,
        "HUD layout",
    ),
    (
        "config/sodium-fingerprint.json",
        OnlyMe,
        "Sodium's install record",
    ),
    (
        "config/embeddium-fingerprint.json",
        OnlyMe,
        "Embeddium's install record",
    ),
    (
        "config/journeymap_modinfo.cfg",
        OnlyMe,
        "JourneyMap's usage report ID",
    ),
    ("config/tandem.json", OnlyMe, "Tandem settings"),
    (
        "config/tandem-crash-seen.txt",
        OnlyMe,
        "Tandem's crash record",
    ),
    (
        "config/nochatreports/ncr-encryption.json",
        OnlyMe,
        "chat encryption key",
    ),
    (
        "config/nochatreports/ncr-serverpreferences.json",
        OnlyMe,
        "chat choices for each server",
    ),
    (
        "config/voicechat/voicechat-client.properties",
        OnlyMe,
        "microphone and speaker settings",
    ),
    (
        "config/voicechat/voicechat-volumes.properties",
        OnlyMe,
        "voice chat volumes",
    ),
    ("config/voicechat/username-cache.json", OnlyMe, "name cache"),
    ("config/jei/bookmarks.ini", OnlyMe, "JEI bookmarks"),
    ("config/jei/world/", OnlyMe, "JEI bookmarks and history"),
    (
        "config/roughlyenoughitems/favorites.json5",
        OnlyMe,
        "REI favorites",
    ),
    (
        "config/lavender_client_storage.json",
        OnlyMe,
        "guide book bookmarks",
    ),
    (
        "config/ars_nouveau/doc_data.json",
        OnlyMe,
        "guide book bookmarks",
    ),
    (
        "config/fancymenu/user_variables.db",
        OnlyMe,
        "FancyMenu's saved values",
    ),
    ("config/jade/usernamecache.json", OnlyMe, "name cache"),
    ("config/bclib/cache.json", OnlyMe, "cache"),
    ("config/brandon3055/resourcecache/", OnlyMe, "cache"),
    ("config/spark/tmp/", OnlyMe, "temporary files"),
    ("config/spark/tmp-client/", OnlyMe, "temporary files"),
    (
        "config/presencefootsteps/updater.json",
        OnlyMe,
        "update check",
    ),
    ("emi.json", OnlyMe, "EMI favorites and history"),
    ("patchouli_data.json", OnlyMe, "guide book bookmarks"),
    ("knownkeys.txt", OnlyMe, "Default Options' key record"),
    ("trashslotsavestate.json", OnlyMe, "trash slot positions"),
    ("hotbar.nbt", OnlyMe, "saved hotbars"),
    ("command_history.txt", OnlyMe, "chat command history"),
    ("realms_persistence.json", OnlyMe, "Realms state"),
    ("servers.dat_old", OnlyMe, "old server list"),
    ("servers.essential.dat", OnlyMe, "Essential's server list"),
    ("irisupdateinfo.json", OnlyMe, "update check"),
    ("journeymap/data/", OnlyMe, "map data and waypoints"),
    ("xaero/", OnlyMe, "map data and waypoints"),
    ("xaerowaypoints/", OnlyMe, "waypoints"),
    ("xaeroworldmap/", OnlyMe, "map data"),
    (
        "distant_horizons_server_data/",
        OnlyMe,
        "Distant Horizons map data",
    ),
    (".bobby/", OnlyMe, "chunk cache"),
    ("local/", OnlyMe, "this PC's own mod data"),
    ("fancymenu_data/", OnlyMe, "FancyMenu's own data"),
    ("downloads/", OnlyMe, "downloads"),
    ("schematics/", OnlyMe, "schematics"),
    ("replay_recordings/", OnlyMe, "replays"),
];

/// Whether `path` holds one player's own settings or data, and what Atlas does with it.
pub fn personal(path: &str) -> Option<Personal> {
    let lower = path.to_ascii_lowercase();
    PERSONAL
        .iter()
        .filter(|(rule, ..)| lower == *rule || rule.ends_with('/') && lower.starts_with(rule))
        .max_by_key(|(rule, ..)| rule.len())
        .map(|(rule, keep, what)| Personal {
            keep: *keep,
            what: (*what).to_owned(),
            rule: (*rule).to_owned(),
        })
}

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
/// A personal `OnlyMe` file the pack doesn't ship counts as unticked by a rule as long as
/// its entry, which a rule naming it (or something inside it) still beats.
pub fn shared(rules: &FileRules, path: &str, from_pack: bool) -> bool {
    let covers = |rule: &String| {
        *rule == path || rule.is_empty() || rule.ends_with('/') && path.starts_with(rule.as_str())
    };
    let best = |list: &[String]| list.iter().filter(|r| covers(r)).map(String::len).max();
    let mine = personal(path)
        .filter(|p| p.keep == OnlyMe && !from_pack)
        .map(|p| p.rule.len());
    match (best(&rules.include), best(&rules.exclude)) {
        (Some(i), e) => e.is_none_or(|e| i > e) && mine.is_none_or(|m| i >= m),
        (None, Some(_)) => false,
        (None, None) => {
            mine.is_none() && (from_pack || DEFAULT_DIRS.iter().any(|d| path.starts_with(d)))
        }
    }
}

/// How members' copies of a shared file are kept.
pub fn keep(rules: &FileRules, path: &str) -> Option<Keep> {
    let once_by_default = || {
        personal(path).is_some_and(|p| p.keep == Once)
            && !rules.every_update.iter().any(|p| p == path)
    };
    if path == "servers.dat" {
        Some(Keep::MergeServers)
    } else if FIRST_INSTALL.contains(&path)
        || rules.first_install.iter().any(|p| p == path)
        || once_by_default()
    {
        Some(Keep::FirstInstall)
    } else {
        None
    }
}

/// `every_update` is saved in the first-install column as "!path", so it needs no column
/// of its own and builds from before it ignore it (no file is named "!...").
pub fn saved_first_install(rules: &FileRules) -> Vec<String> {
    rules
        .first_install
        .iter()
        .cloned()
        .chain(rules.every_update.iter().map(|p| format!("!{p}")))
        .collect()
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
    let (every_update, first_install): (Vec<String>, Vec<String>) =
        first_install.into_iter().partition(|p| p.starts_with('!'));
    FileRules {
        include: include.into_iter().map(prefix).collect(),
        exclude: exclude.into_iter().map(prefix).collect(),
        first_install,
        every_update: every_update
            .into_iter()
            .map(|p| p[1..].to_owned())
            .collect(),
    }
}

/// The Files tab's choices as saved on the instance.
pub async fn saved_rules(db: &SqlitePool, id: &str) -> AppResult<FileRules> {
    let saved: Option<(String, String, String)> = sqlx::query_as(
        "SELECT sync_include, sync_exclude, sync_first_install FROM instance WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    let (include, exclude, first) = saved.unwrap_or_default();
    let list = |json: &str| serde_json::from_str::<Vec<String>>(json).unwrap_or_default();
    Ok(from_saved(list(&include), list(&exclude), list(&first)))
}

pub async fn save_rules(
    db: impl sqlx::SqliteExecutor<'_>,
    id: &str,
    rules: &FileRules,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE instance SET sync_include = ?, sync_exclude = ?, sync_first_install = ? WHERE id = ?",
    )
    .bind(serde_json::to_string(&rules.include)?)
    .bind(serde_json::to_string(&rules.exclude)?)
    .bind(serde_json::to_string(&saved_first_install(rules))?)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
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
            ..FileRules::default()
        }
    }

    #[test]
    fn defaults_tick_configs_scripts_and_the_packs_own_files() {
        let none = rules(&[], &[]);
        assert!(shared(&none, "config/create/server.toml", false));
        assert!(shared(&none, "scripts/recipes.zs", false));
        assert!(shared(
            &none,
            "resources/test/textures/gui/title/background/bg.png",
            false
        ));
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
    fn personal_files_get_their_own_defaults() {
        let none = rules(&[], &[]);
        assert_eq!(
            keep(&none, "config/sodium-options.json"),
            Some(Keep::FirstInstall)
        );
        assert!(!shared(&none, "config/jei/bookmarks.ini", false), "only me");
        assert!(
            shared(&none, "config/jei/bookmarks.ini", true),
            "a pack's own copy still goes out"
        );
        assert!(!shared(
            &none,
            "config/jei/world/local/a/bookmarks.json",
            false
        ));
        // Ticking the folder above leaves personal files out; naming one shares it.
        let folder = rules(&["config/jei/"], &[]);
        assert!(!shared(&folder, "config/jei/bookmarks.ini", false));
        assert!(shared(&folder, "config/jei/jei-client.ini", false));
        let named = rules(&["config/jei/bookmarks.ini", "config/jei/world/"], &[]);
        assert!(shared(&named, "config/jei/bookmarks.ini", false));
        assert!(shared(
            &named,
            "config/jei/world/local/a/bookmarks.json",
            false
        ));
        // Case doesn't matter, and the longest entry wins.
        assert_eq!(
            personal("journeymap/config/5.7/journeymap.core.config").map(|p| p.keep),
            Some(Once)
        );
        assert_eq!(
            personal("JourneyMap/data/sp/World/x.png").map(|p| p.keep),
            Some(OnlyMe)
        );
        assert_eq!(personal("config/create-client.toml"), None);
    }

    #[test]
    fn every_update_is_saved_beside_first_install() {
        let mine = FileRules {
            first_install: vec!["optionsof.txt".into()],
            every_update: vec!["config/iris.properties".into()],
            ..FileRules::default()
        };
        assert_eq!(
            keep(&mine, "config/iris.properties"),
            None,
            "sent on every update"
        );
        let saved = saved_first_install(&mine);
        assert_eq!(saved, ["optionsof.txt", "!config/iris.properties"]);
        assert_eq!(from_saved(vec![], vec![], saved), mine);
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
