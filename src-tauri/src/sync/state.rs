//! Sync state, computed when a response is built and never stored.
//! Pure functions: the same rules as `localChanges` / `refreshSyncState` in the mock
//! (src/lib/api/mock.ts), plus deleted files, which the mock can't model.

use std::collections::HashSet;

use super::manifest::Manifest;
use crate::db::content::ContentRow;
use crate::dto::{MemberRole, ShareMode, SyncState, from_wire};

/// What differs from the synced version.
#[derive(Debug, Default)]
pub struct LocalChanges<'a> {
    /// Names of shared files that are disabled or deleted here.
    pub removed: Vec<String>,
    /// Owners/editors: shared files whose share setting changed since the last publish.
    pub mode_changed: Vec<&'a ContentRow>,
    /// Owners/editors: files not in the shared version that would now be shared.
    pub added: Vec<&'a ContentRow>,
}

impl LocalChanges<'_> {
    pub fn count(&self) -> u32 {
        (self.removed.len() + self.mode_changed.len() + self.added.len()) as u32
    }
}

fn mode(s: &str) -> Option<ShareMode> {
    from_wire(s)
}

/// Owners/editors: their folder is the source of the next version, so anything that would
/// change it counts. Viewers: only disabling or deleting a *required* file counts; their own
/// extras are theirs, and skipping or removing optional items is their choice.
pub fn local_changes<'a>(
    role: MemberRole,
    rows: &'a [ContentRow],
    synced: Option<&Manifest>,
) -> LocalChanges<'a> {
    let live: Vec<&ContentRow> = rows.iter().filter(|r| r.installed).collect();
    let viewer = role == MemberRole::Viewer;
    let mut out = LocalChanges::default();

    // Disabled shared files.
    for r in &live {
        let required = mode(&r.share_mode) == Some(ShareMode::Required);
        if r.is_managed && !r.enabled && (!viewer || required) {
            out.removed.push(r.name.clone());
        }
    }
    // Deleted shared files: in the synced version, no row at that path any more.
    // (Optional items the user removed keep a row with installed = 0.)
    if let Some(m) = synced {
        let paths: HashSet<String> = rows.iter().map(|r| r.path.to_lowercase()).collect();
        for e in &m.content {
            if !paths.contains(&e.path.to_lowercase()) && (!viewer || e.required) {
                out.removed.push(e.name.clone());
            }
        }
    }
    if viewer {
        return out;
    }
    for r in live.into_iter().filter(|r| r.enabled) {
        let now = mode(&r.share_mode);
        let published = r.published_share_mode.as_deref().and_then(mode);
        if r.is_managed && now != published {
            out.mode_changed.push(r);
        } else if !r.is_managed && now != Some(ShareMode::Private) && now != published {
            out.added.push(r);
        }
    }
    out
}

pub fn sync_state(
    frozen: bool,
    synced_revision: u32,
    latest_revision: u32,
    local_change_count: u32,
) -> SyncState {
    let behind = synced_revision < latest_revision;
    match (frozen, local_change_count > 0, behind) {
        (true, _, _) => SyncState::Frozen,
        (_, true, true) => SyncState::Conflict,
        (_, true, false) => SyncState::LocalChanges,
        (_, false, true) => SyncState::UpdateAvailable,
        (_, false, false) => SyncState::UpToDate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{ContentKind, ContentSource, Loader, Side};
    use crate::sync::manifest::{Entry, MinecraftSpec};

    /// A row as the scanner would leave it, after a publish/sync marked it.
    fn row(
        name: &str,
        managed: bool,
        enabled: bool,
        mode: &str,
        published: Option<&str>,
    ) -> ContentRow {
        ContentRow {
            id: name.into(),
            instance_id: "i".into(),
            kind: "mod".into(),
            path: format!("mods/{name}.jar"),
            enabled,
            size_bytes: 1,
            mtime_ms: Some(1),
            sha1: "a".repeat(40),
            sha256: "b".repeat(64),
            murmur2: None,
            mod_id: None,
            source: "modrinth".into(),
            project_id: None,
            version_id: None,
            name: name.into(),
            version_label: None,
            author: None,
            icon_url: None,
            download_url: None,
            page_url: None,
            side: "both".into(),
            resolved_at: None,
            is_managed: managed,
            share_mode: mode.into(),
            published_share_mode: published.map(Into::into),
            installed: true,
        }
    }

    fn manifest(entries: &[(&str, bool)]) -> Manifest {
        Manifest {
            schema_version: 1,
            name: "m".into(),
            description: None,
            icon: None,
            minecraft: MinecraftSpec {
                version: "1.21.1".into(),
                loader: Loader::Fabric,
                loader_version: None,
            },
            recommended_memory_mb: None,
            content: entries
                .iter()
                .map(|(n, required)| Entry {
                    kind: ContentKind::Mod,
                    source: ContentSource::Modrinth,
                    project_id: Some(n.to_string()),
                    version_id: None,
                    name: n.to_string(),
                    version_label: None,
                    path: format!("mods/{n}.jar"),
                    sha1: "a".repeat(40),
                    sha256: "b".repeat(64),
                    size_bytes: 1,
                    download_url: None,
                    page_url: None,
                    side: Side::Both,
                    required: *required,
                })
                .collect(),
            overrides: vec![],
        }
    }

    #[test]
    fn in_step_with_the_shared_version_is_no_change() {
        let rows = [
            row("sodium", true, true, "required", Some("required")),
            row("iris", true, true, "optional", Some("optional")),
            row("mine", false, true, "private", Some("private")),
        ];
        let m = manifest(&[("sodium", true), ("iris", false)]);
        for role in [MemberRole::Owner, MemberRole::Viewer] {
            assert_eq!(local_changes(role, &rows, Some(&m)).count(), 0, "{role:?}");
        }
    }

    #[test]
    fn owner_changes() {
        let rows = [
            row("sodium", true, false, "required", Some("required")), // disabled
            row("iris", true, true, "private", Some("optional")),     // setting changed
            row("new", false, true, "required", None),                // would be shared
            row("mine", false, true, "private", None),                // only me: not a change
            row("off", false, false, "required", None),               // disabled extra: not shared
        ];
        let m = manifest(&[("sodium", true), ("iris", false), ("lithium", true)]); // lithium deleted
        let c = local_changes(MemberRole::Owner, &rows, Some(&m));
        assert_eq!(c.removed, ["sodium", "lithium"]);
        assert_eq!(
            c.mode_changed
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            ["iris"]
        );
        assert_eq!(
            c.added.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["new"]
        );
        assert_eq!(c.count(), 4);
        // Editors follow the same rules.
        assert_eq!(
            local_changes(MemberRole::Editor, &rows, Some(&m)).count(),
            4
        );
    }

    #[test]
    fn viewer_changes_are_only_required_files() {
        let mut skipped = row("iris", true, true, "optional", Some("optional"));
        skipped.installed = false; // chose not to download it
        let rows = [
            row("sodium", true, false, "required", Some("required")), // disabled required: counts
            row("fa", true, false, "optional", Some("optional")), // disabled optional: their choice
            skipped,
            row("extra", false, true, "private", None), // their own extra
        ];
        let m = manifest(&[
            ("sodium", true),
            ("fa", false),
            ("iris", false),
            ("lithium", true),
            ("zoom", false),
        ]);
        let c = local_changes(MemberRole::Viewer, &rows, Some(&m));
        // lithium (required) deleted counts; zoom (optional) gone doesn't.
        assert_eq!(c.removed, ["sodium", "lithium"]);
        assert_eq!(c.count(), 2);
    }

    #[test]
    fn states() {
        assert_eq!(sync_state(false, 3, 3, 0), SyncState::UpToDate);
        assert_eq!(sync_state(false, 2, 3, 0), SyncState::UpdateAvailable);
        assert_eq!(sync_state(false, 3, 3, 1), SyncState::LocalChanges);
        assert_eq!(sync_state(false, 2, 3, 1), SyncState::Conflict);
        assert_eq!(sync_state(true, 2, 3, 1), SyncState::Frozen);
    }
}
