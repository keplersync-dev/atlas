//! The files an owner can share besides mods and packs: listing the instance folder for
//! the share dialog's tree, and deciding which are ticked.
//!
//! Ticks are rules: paths ticked (`include`) or unticked (`exclude`), where a folder ends
//! in '/' and covers what's inside, and "" covers everything. The longest matching rule
//! wins. With none, the defaults decide: the config and script folders, and the files
//! the modpack itself ships. A folder ticked once stays ticked for files added later.

use std::path::{Path, PathBuf};

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

/// Every file the tree lists, `(path, file, size)`, sorted by path. Hidden folders aren't
/// walked at all (a world can hold tens of thousands of files).
pub fn list(game: &Path) -> Vec<(String, PathBuf, u64)> {
    let mut out = Vec::new();
    walk(game, "", &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Folder links (symlinks, junctions) aren't followed: one pointing back up would never end.
fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf, u64)>) {
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
            out.push((child, entry.path(), meta.len()));
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
        let paths: Vec<String> = list(&dir).into_iter().map(|(p, ..)| p).collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            paths,
            ["config/a.json", "global_packs/x/pack.mcmeta", "options.txt"]
        );
    }
}
