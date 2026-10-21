//! Decide what an update does to a folder (tauri-plan §12.2–12.3). Pure: no files, no
//! network. The same planner drives updates, first installs, revert and installing an
//! optional item: they only differ in the target revision and which optional items are wanted.
//!
//! Rules:
//! - Only managed files are removed or replaced. The user's own files are never touched,
//!   except one sitting exactly where the update needs to put a file (backed up first).
//! - Optional items keep the user's decision (installed or not). New ones are installed
//!   only if ticked (`install_optional`).
//! - A shared file the user disabled is enabled again (that's "overwrittenLocalChanges").
//! - Config overrides merge three ways: synced (base), on disk (mine), target (theirs).
//!   Only both-changed is a conflict; the user's choice decides, default theirs.

use std::collections::{HashMap, HashSet};

use super::manifest::{Entry, Manifest, Override};
use crate::db::content::ContentRow;
use crate::dto::{ConfigChoice, OptionalItem, RevisionChanges, UpdatedItem};

/// One step of applying a plan, in order: all removals, then placements, then configs.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Delete a file. `path` is the name on disk (may end in ".disabled").
    Remove { path: String },
    /// Put a manifest file at `entry.path`. `replaces`: a file already at that exact path
    /// (backed up, then overwritten). `local`: a file on disk with these exact bytes that
    /// can be used instead of downloading (re-enabling a disabled mod).
    Place { entry: Entry, replaces: bool, local: Option<String> },
    /// Write a config file from the target version.
    WriteConfig { config: Override, replaces: bool },
    /// Delete a config file the target version no longer has (only if the user didn't edit it).
    RemoveConfig { path: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub path: String,
    /// sha256 of the user's file and of the target's.
    pub mine: String,
    pub theirs: String,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub actions: Vec<Action>,
    /// What the revision changes, for the review screen.
    pub changes: RevisionChanges,
    pub new_optional: Vec<OptionalItem>,
    pub conflicts: Vec<Conflict>,
}

impl Plan {
    pub fn places(&self) -> impl Iterator<Item = (&Entry, Option<&String>)> {
        self.actions.iter().filter_map(|a| match a {
            Action::Place { entry, local, .. } => Some((entry, local.as_ref())),
            _ => None,
        })
    }
}

pub struct Input<'a> {
    /// The version the folder was last made equal to (None: nothing installed from it yet).
    pub synced: Option<&'a Manifest>,
    pub target: &'a Manifest,
    pub rows: &'a [ContentRow],
    /// Keys of new optional items the user ticked.
    pub install_optional: &'a HashSet<String>,
    pub config_choices: &'a HashMap<String, ConfigChoice>,
    /// sha256 of a config file on disk, by relative path (None = no file).
    pub config_on_disk: &'a dyn Fn(&str) -> Option<String>,
}

/// The file name on disk for a row (disabled files end in ".disabled").
pub fn disk_path(row: &ContentRow) -> String {
    if row.enabled { row.path.clone() } else { format!("{}.disabled", row.path) }
}

fn label(e: &Entry) -> String {
    e.version_label.clone().unwrap_or_else(|| e.path.rsplit('/').next().unwrap_or(&e.path).to_string())
}

fn kind_label(required: bool) -> &'static str {
    if required { "Required" } else { "Optional" }
}

pub fn plan(input: &Input) -> Plan {
    let rows: HashMap<String, &ContentRow> = input.rows.iter().map(|r| (r.path.to_lowercase(), r)).collect();
    let row_at = |path: &str| rows.get(&path.to_lowercase()).copied();
    let synced_by_key: HashMap<&str, &Entry> =
        input.synced.map(|m| m.content.iter().map(|e| (e.key(), e)).collect()).unwrap_or_default();
    let target_keys: HashSet<&str> = input.target.content.iter().map(Entry::key).collect();

    let mut out = Plan::default();
    let mut removes = Vec::new();
    let mut places = Vec::new();

    for e in &input.target.content {
        let before = synced_by_key.get(e.key()).copied();
        let old_row = before.and_then(|b| row_at(&b.path));
        let new_row = row_at(&e.path);

        // What the review screen shows.
        match before {
            None if e.required => out.changes.added.push(e.name.clone()),
            None => {}
            Some(b) if b.sha1 != e.sha1 => {
                out.changes.updated.push(UpdatedItem { name: e.name.clone(), from: label(b), to: label(e) })
            }
            Some(b) if b.required != e.required => out.changes.updated.push(UpdatedItem {
                name: e.name.clone(),
                from: kind_label(b.required).into(),
                to: kind_label(e.required).into(),
            }),
            Some(_) => {}
        }

        // Does the user want this file? Required: yes. Optional: if ticked now, else
        // whatever they decided before (installed or not). Never seen before = new optional.
        let existing = old_row.or(new_row);
        if !e.required && before.is_none() && existing.is_none() {
            out.new_optional.push(OptionalItem {
                key: e.key().to_string(),
                name: e.name.clone(),
                kind: e.kind,
                size_bytes: e.size_bytes,
            });
        }
        let wanted = e.required
            || input.install_optional.contains(e.key())
            || existing.is_some_and(|r| r.installed);
        if !wanted {
            continue;
        }

        // Already there with the right bytes?
        if let Some(r) = new_row.filter(|r| r.installed && r.sha1 == e.sha1) {
            if !r.enabled {
                // Disabled by the user: enable it again, reusing its own bytes.
                removes.push(Action::Remove { path: disk_path(r) });
                places.push(Action::Place { entry: e.clone(), replaces: false, local: Some(disk_path(r)) });
            }
            continue;
        }
        // The old version under a different name goes.
        if let Some(r) = old_row.filter(|r| r.installed && r.is_managed && !r.path.eq_ignore_ascii_case(&e.path)) {
            removes.push(Action::Remove { path: disk_path(r) });
        }
        // Something else at this exact path: replace it (disabled copies are removed first).
        let replaces = match new_row.filter(|r| r.installed) {
            Some(r) if r.enabled => true,
            Some(r) => {
                removes.push(Action::Remove { path: disk_path(r) });
                false
            }
            None => false,
        };
        places.push(Action::Place { entry: e.clone(), replaces, local: None });
    }

    // Shared files the target no longer has.
    if let Some(synced) = input.synced {
        for b in synced.content.iter().filter(|b| !target_keys.contains(b.key())) {
            out.changes.removed.push(b.name.clone());
            if let Some(r) = row_at(&b.path).filter(|r| r.installed && r.is_managed) {
                removes.push(Action::Remove { path: disk_path(r) });
            }
        }
    }

    // Configs, three ways.
    let base: HashMap<String, &Override> = input
        .synced
        .map(|m| m.overrides.iter().map(|o| (o.path.to_lowercase(), o)).collect())
        .unwrap_or_default();
    let mut configs = Vec::new();
    for o in &input.target.overrides {
        let mine = (input.config_on_disk)(&o.path);
        let base_sha = base.get(&o.path.to_lowercase()).map(|b| b.sha256.as_str());
        let Some(mine) = mine else {
            configs.push(Action::WriteConfig { config: o.clone(), replaces: false });
            continue;
        };
        if mine == o.sha256 || base_sha == Some(o.sha256.as_str()) {
            continue; // already the same, or only the user changed it: keep the user's
        }
        if base_sha == Some(mine.as_str()) {
            configs.push(Action::WriteConfig { config: o.clone(), replaces: true }); // only they changed it
            continue;
        }
        // Both changed (or the user already had their own file there).
        out.conflicts.push(Conflict { path: o.path.clone(), mine, theirs: o.sha256.clone() });
        if input.config_choices.get(&o.path) != Some(&ConfigChoice::Mine) {
            configs.push(Action::WriteConfig { config: o.clone(), replaces: true });
        }
    }
    let target_configs: HashSet<String> = input.target.overrides.iter().map(|o| o.path.to_lowercase()).collect();
    for (path, b) in &base {
        if !target_configs.contains(path) && (input.config_on_disk)(&b.path).as_deref() == Some(b.sha256.as_str()) {
            configs.push(Action::RemoveConfig { path: b.path.clone() });
        }
    }

    removes.dedup();
    out.actions = removes.into_iter().chain(places).chain(configs).collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{ContentKind, ContentSource, Loader, Side};
    use crate::sync::manifest::MinecraftSpec;

    fn entry(key: &str, file: &str, sha: char, required: bool) -> Entry {
        Entry {
            kind: ContentKind::Mod,
            source: ContentSource::Modrinth,
            project_id: Some(key.into()),
            version_id: None,
            name: key.into(),
            version_label: Some(file.into()),
            path: format!("mods/{file}.jar"),
            sha1: sha.to_string().repeat(40),
            sha256: sha.to_string().repeat(64),
            size_bytes: 10,
            download_url: None,
            page_url: None,
            side: Side::Both,
            required,
        }
    }

    fn manifest(content: Vec<Entry>, overrides: Vec<Override>) -> Manifest {
        Manifest {
            schema_version: 1,
            name: "m".into(),
            description: None,
            icon: None,
            minecraft: MinecraftSpec { version: "1.21.1".into(), loader: Loader::Fabric, loader_version: None },
            recommended_memory_mb: None,
            content,
            overrides,
        }
    }

    fn row(e: &Entry, managed: bool, enabled: bool, installed: bool) -> ContentRow {
        ContentRow {
            id: e.name.clone(),
            instance_id: "i".into(),
            kind: "mod".into(),
            path: e.path.clone(),
            enabled,
            size_bytes: 10,
            mtime_ms: Some(1),
            sha1: e.sha1.clone(),
            sha256: e.sha256.clone(),
            murmur2: None,
            mod_id: None,
            source: "modrinth".into(),
            project_id: e.project_id.clone(),
            version_id: None,
            name: e.name.clone(),
            version_label: None,
            author: None,
            icon_url: None,
            download_url: None,
            page_url: None,
            side: "both".into(),
            resolved_at: None,
            is_managed: managed,
            share_mode: if e.required { "required" } else { "optional" }.into(),
            published_share_mode: Some(if e.required { "required" } else { "optional" }.into()),
            installed,
        }
    }

    fn run(synced: Option<&Manifest>, target: &Manifest, rows: &[ContentRow], ticked: &[&str]) -> Plan {
        run_with(synced, target, rows, ticked, &HashMap::new(), &|_| None)
    }

    fn run_with(
        synced: Option<&Manifest>,
        target: &Manifest,
        rows: &[ContentRow],
        ticked: &[&str],
        choices: &HashMap<String, ConfigChoice>,
        disk: &dyn Fn(&str) -> Option<String>,
    ) -> Plan {
        let ticked: HashSet<String> = ticked.iter().map(|s| s.to_string()).collect();
        plan(&Input { synced, target, rows, install_optional: &ticked, config_choices: choices, config_on_disk: disk })
    }

    fn paths(p: &Plan) -> Vec<String> {
        p.actions
            .iter()
            .map(|a| match a {
                Action::Remove { path } => format!("- {path}"),
                Action::Place { entry, replaces, local } => {
                    format!("+ {}{}{}", entry.path, if *replaces { " (replace)" } else { "" }, if local.is_some() { " (local)" } else { "" })
                }
                Action::WriteConfig { config, replaces } => format!("cfg {}{}", config.path, if *replaces { " (replace)" } else { "" }),
                Action::RemoveConfig { path } => format!("cfg- {path}"),
            })
            .collect()
    }

    #[test]
    fn first_install_puts_required_files_and_ticked_optional_ones() {
        let target = manifest(vec![entry("sodium", "sodium-1", 'a', true), entry("iris", "iris-1", 'b', false), entry("zoom", "zoom-1", 'c', false)], vec![]);
        let p = run(None, &target, &[], &["iris"]);
        assert_eq!(paths(&p), ["+ mods/sodium-1.jar", "+ mods/iris-1.jar"]);
        assert_eq!(p.changes.added, ["sodium"]);
        assert_eq!(p.new_optional.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), ["iris", "zoom"]);
    }

    #[test]
    fn an_update_replaces_renames_and_removes_only_shared_files() {
        let s1 = entry("sodium", "sodium-1", 'a', true);
        let l1 = entry("lithium", "lithium-1", 'b', true);
        let synced = manifest(vec![s1.clone(), l1.clone()], vec![]);
        let s2 = entry("sodium", "sodium-2", 'c', true); // new version, new file name
        let target = manifest(vec![s2, entry("ferrite", "ferrite-1", 'd', true)], vec![]); // lithium dropped
        let extra = entry("minimap", "minimap", 'e', true);
        let rows = [row(&s1, true, true, true), row(&l1, true, true, true), row(&extra, false, true, true)];
        let p = run(Some(&synced), &target, &rows, &[]);
        assert_eq!(paths(&p), ["- mods/sodium-1.jar", "- mods/lithium-1.jar", "+ mods/sodium-2.jar", "+ mods/ferrite-1.jar"]);
        assert_eq!(p.changes.added, ["ferrite"]);
        assert_eq!(p.changes.removed, ["lithium"]);
        assert_eq!(p.changes.updated, [UpdatedItem { name: "sodium".into(), from: "sodium-1".into(), to: "sodium-2".into() }]);
        // The user's minimap is never touched.
        assert!(!paths(&p).iter().any(|a| a.contains("minimap")));
    }

    #[test]
    fn up_to_date_is_nothing_to_do() {
        let s = entry("sodium", "sodium-1", 'a', true);
        let m = manifest(vec![s.clone()], vec![]);
        assert!(run(Some(&m), &m, &[row(&s, true, true, true)], &[]).actions.is_empty());
    }

    #[test]
    fn disabled_or_deleted_shared_files_come_back() {
        let s = entry("sodium", "sodium-1", 'a', true);
        let l = entry("lithium", "lithium-1", 'b', true);
        let m = manifest(vec![s.clone(), l], vec![]);
        // sodium disabled, lithium deleted (no row): a revert to the same revision.
        let p = run(Some(&m), &m, &[row(&s, true, false, true)], &[]);
        assert_eq!(paths(&p), ["- mods/sodium-1.jar.disabled", "+ mods/sodium-1.jar (local)", "+ mods/lithium-1.jar"]);
    }

    #[test]
    fn optional_items_keep_the_users_decision() {
        let iris = entry("iris", "iris-1", 'a', false);
        let fa = entry("fa", "fa-1", 'b', false);
        let synced = manifest(vec![iris.clone(), fa.clone()], vec![]);
        let iris2 = entry("iris", "iris-2", 'c', false);
        let fa2 = entry("fa", "fa-2", 'd', false);
        let target = manifest(vec![iris2, fa2, entry("zoom", "zoom-1", 'e', false)], vec![]);
        // iris installed, fa skipped (row with installed = 0), zoom new and not ticked.
        let rows = [row(&iris, true, true, true), row(&fa, true, true, false)];
        let p = run(Some(&synced), &target, &rows, &[]);
        assert_eq!(paths(&p), ["- mods/iris-1.jar", "+ mods/iris-2.jar"]);
        assert_eq!(p.new_optional.len(), 1, "only zoom is new");
        // Ticking zoom installs it; ticking fa installs the one they skipped before.
        let p = run(Some(&synced), &target, &rows, &["zoom", "fa"]);
        assert!(paths(&p).contains(&"+ mods/zoom-1.jar".to_string()));
        assert!(paths(&p).contains(&"+ mods/fa-2.jar".to_string()));
    }

    #[test]
    fn a_users_file_in_the_way_is_replaced() {
        let s = entry("sodium", "sodium", 'a', true);
        let mut theirs = row(&s, false, true, true);
        theirs.sha1 = "f".repeat(40); // their own different sodium.jar
        let target = manifest(vec![s], vec![]);
        assert_eq!(paths(&run(None, &target, &[theirs], &[])), ["+ mods/sodium.jar (replace)"]);
    }

    #[test]
    fn configs_merge_three_ways() {
        let o = |path: &str, sha: char| Override { path: path.into(), sha256: sha.to_string().repeat(64), size_bytes: 1 };
        let synced = manifest(vec![], vec![o("config/a.json", '1'), o("config/b.json", '1'), o("config/c.json", '1'), o("config/gone.json", '1'), o("config/kept.json", '1')]);
        let target = manifest(vec![], vec![o("config/a.json", '2'), o("config/b.json", '1'), o("config/c.json", '2'), o("config/new.json", '1')]);
        let disk: HashMap<&str, char> = HashMap::from([
            ("config/a.json", '1'),    // untouched by the user, changed by them: take theirs
            ("config/b.json", '9'),    // changed by the user only: keep
            ("config/c.json", '9'),    // both changed: conflict
            ("config/gone.json", '1'), // dropped by them, untouched here: remove
            ("config/kept.json", '9'), // dropped by them, edited here: keep
        ]);
        let lookup = |p: &str| disk.get(p).map(|c| c.to_string().repeat(64));
        let p = run_with(Some(&synced), &target, &[], &[], &HashMap::new(), &lookup);
        assert_eq!(paths(&p), ["cfg config/a.json (replace)", "cfg config/c.json (replace)", "cfg config/new.json", "cfg- config/gone.json"]);
        assert_eq!(p.conflicts.len(), 1);
        assert_eq!(p.conflicts[0].path, "config/c.json");

        // Choosing "mine" keeps the user's file.
        let choices = HashMap::from([("config/c.json".to_string(), ConfigChoice::Mine)]);
        let p = run_with(Some(&synced), &target, &[], &[], &choices, &lookup);
        assert!(!paths(&p).contains(&"cfg config/c.json (replace)".to_string()));
    }
}
