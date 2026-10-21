//! T4 end to end over IPC, with a dev source standing in for the server: first install,
//! update, config conflicts, revert, optional items, restore, a failed update, and
//! recovering from a crash in the middle of applying.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use atlas_lib::scan::hash::hash_file;
use common::{TempDir, TestApp};
use serde_json::{Value, json};

/// A folder laid out like sync/source.rs expects, built by the test.
struct DevSource {
    dir: TempDir,
}

/// (path, display name, project id, required)
type E<'a> = (&'a str, &'a str, &'a str, bool);

impl DevSource {
    fn new() -> Self {
        let s = Self {
            dir: TempDir::new(),
        };
        std::fs::create_dir_all(s.dir.0.join("revisions")).unwrap();
        s
    }
    fn path(&self) -> &Path {
        &self.dir.0
    }
    fn file(&self, path: &str, bytes: &[u8]) {
        let p = self.dir.0.join("files").join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }
    /// Write revisions/<n>.json listing these files (already written with `file`).
    fn revision(&self, n: u32, content: &[E], configs: &[&str]) {
        let entry = |(path, name, project, required): &E| {
            let h = hash_file(&self.dir.0.join("files").join(path)).unwrap();
            let kind = if path.starts_with("mods/") {
                "mod"
            } else {
                "resourcepack"
            };
            json!({
                "kind": kind, "source": "modrinth", "projectId": project, "versionId": format!("{project}-v{n}"),
                "name": name, "versionLabel": format!("v{n}"), "path": path,
                "sha1": h.sha1, "sha256": h.sha256,
                "sizeBytes": std::fs::metadata(self.dir.0.join("files").join(path)).unwrap().len(),
                "downloadUrl": null, "side": "both", "required": required,
            })
        };
        let config = |path: &&str| {
            let h = hash_file(&self.dir.0.join("files").join(path)).unwrap();
            json!({ "path": path, "sha256": h.sha256, "sizeBytes": std::fs::metadata(self.dir.0.join("files").join(path)).unwrap().len() })
        };
        let m = json!({
            "schemaVersion": 1, "name": "Survival SMP",
            "minecraft": { "version": "1.21.1", "loader": "fabric", "loaderVersion": "0.16.5" },
            "content": content.iter().map(entry).collect::<Vec<_>>(),
            "overrides": configs.iter().map(config).collect::<Vec<_>>(),
        });
        std::fs::write(
            self.dir.0.join(format!("revisions/{n}.json")),
            serde_json::to_vec_pretty(&m).unwrap(),
        )
        .unwrap();
    }
}

struct World {
    app: TestApp,
    game: PathBuf,
    id: String,
    source: DevSource,
    _dir: TempDir,
}

impl World {
    /// A tracked folder with one mod of the user's own, and a dev source with revision 1:
    /// Sodium (required), Iris (optional), a config file.
    fn new() -> Self {
        let app = TestApp::new();
        let dir = TempDir::new();
        let game = dir.0.join("Survival SMP");
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/minimap.jar"), b"my own minimap").unwrap();
        let id = app.ok("track_folder", json!({ "gameDir": game }))["id"]
            .as_str()
            .unwrap()
            .to_string();
        app.ok("rescan_instance", json!({ "id": id }));

        let source = DevSource::new();
        source.file("mods/sodium-1.jar", b"sodium version one");
        source.file("mods/iris-1.jar", b"iris version one");
        source.file("config/sodium.json", b"{\n  \"quality\": \"fancy\"\n}\n");
        source.revision(
            1,
            &[
                ("mods/sodium-1.jar", "Sodium", "AANobbMI", true),
                ("mods/iris-1.jar", "Iris", "YL57xq9U", false),
            ],
            &["config/sodium.json"],
        );
        Self {
            app,
            game,
            id,
            source,
            _dir: dir,
        }
    }

    fn link(&self) {
        self.app.ok(
            "dev_link_manifest",
            json!({ "instanceId": self.id, "folder": self.source.path(), "role": "viewer", "adoptRevision": null }),
        );
    }

    fn detail(&self) -> Value {
        self.app.ok("get_instance", json!({ "id": self.id }))
    }

    fn item<'a>(detail: &'a Value, path: &str) -> Option<&'a Value> {
        detail["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["path"] == path)
    }

    /// Wait for the running job to finish (jobs run in the background).
    fn wait(&self) -> Value {
        let start = Instant::now();
        loop {
            let d = self.detail();
            if d["status"] != "syncing" {
                return d;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "job didn't finish"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn sync(&self, options: Value) -> Value {
        let started = self.app.ok(
            "sync_instance",
            json!({ "instanceId": self.id, "options": options }),
        );
        assert_eq!(
            started["status"], "syncing",
            "returns right away while the job runs"
        );
        self.wait()
    }

    fn read(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(self.game.join(path)).ok()
    }
}

#[test]
fn first_install_puts_the_shared_files_in_and_leaves_mine_alone() {
    let w = World::new();
    w.link();
    let d = w.detail();
    assert_eq!(d["sync"]["state"], "update_available");
    assert_eq!(d["sync"]["syncedRevision"], 0);
    assert_eq!(d["sync"]["latestRevision"], 1);
    assert_eq!(d["sync"]["role"], "viewer");

    let preview = w.app.ok("preview_sync", json!({ "instanceId": w.id }));
    assert_eq!(preview["fromRevision"], 0);
    assert_eq!(preview["toRevision"], 1);
    assert_eq!(preview["changes"]["added"], json!(["Sodium"]));
    assert_eq!(preview["newOptional"][0]["key"], "YL57xq9U");
    assert_eq!(preview["newOptional"][0]["name"], "Iris");
    assert!(preview["downloadBytes"].as_u64().unwrap() > 0);
    assert_eq!(preview["gameRunning"], false);

    let d = w.sync(json!({}));
    assert_eq!(
        w.read("mods/sodium-1.jar").as_deref(),
        Some("sodium version one")
    );
    assert!(
        w.read("mods/iris-1.jar").is_none(),
        "optional items start unticked"
    );
    assert_eq!(
        w.read("mods/minimap.jar").as_deref(),
        Some("my own minimap")
    );
    assert!(w.read("config/sodium.json").unwrap().contains("fancy"));
    assert!(
        !w.game.join(".atlas-staging").exists(),
        "staging cleaned up"
    );

    assert_eq!(d["sync"]["state"], "up_to_date");
    assert_eq!(d["sync"]["syncedRevision"], 1);
    let sodium = World::item(&d, "mods/sodium-1.jar").unwrap();
    assert_eq!(
        (sodium["isManaged"].clone(), sodium["shareMode"].clone()),
        (json!(true), json!("required"))
    );
    assert_eq!(sodium["name"], "Sodium");
    let iris = World::item(&d, "mods/iris-1.jar").unwrap();
    assert_eq!(
        (iris["installed"].clone(), iris["shareMode"].clone()),
        (json!(false), json!("optional"))
    );
    let mine = World::item(&d, "mods/minimap.jar").unwrap();
    assert_eq!(mine["isManaged"], false);
    assert_eq!(d["backups"].as_array().unwrap().len(), 1);
    assert_eq!(d["backups"][0]["reason"], "before_update");
}

#[test]
fn an_update_renames_files_and_respects_config_choices() {
    let w = World::new();
    w.link();
    w.sync(json!({ "installOptional": ["YL57xq9U"] }));
    assert!(w.read("mods/iris-1.jar").is_some(), "ticked, so installed");

    // Both sides change the config; the publisher updates Sodium (new file name).
    std::fs::write(
        w.game.join("config/sodium.json"),
        "{\n  \"quality\": \"fast\"\n}\n",
    )
    .unwrap();
    w.source.file("mods/sodium-2.jar", b"sodium version two");
    w.source
        .file("config/sodium.json", b"{\n  \"quality\": \"fabulous\"\n}\n");
    w.source.revision(
        2,
        &[
            ("mods/sodium-2.jar", "Sodium", "AANobbMI", true),
            ("mods/iris-1.jar", "Iris", "YL57xq9U", false),
        ],
        &["config/sodium.json"],
    );
    w.app.ok("rescan_instance", json!({ "id": w.id }));
    assert_eq!(w.detail()["sync"]["state"], "update_available");

    let preview = w.app.ok("preview_sync", json!({ "instanceId": w.id }));
    assert_eq!(
        preview["changes"]["updated"],
        json!([{ "name": "Sodium", "from": "v1", "to": "v2" }])
    );
    assert_eq!(preview["configConflicts"][0]["path"], "config/sodium.json");
    assert_eq!(
        preview["configConflicts"][0]["mine"],
        "\"quality\": \"fast\""
    );
    assert_eq!(
        preview["configConflicts"][0]["theirs"],
        "\"quality\": \"fabulous\""
    );

    let d = w.sync(json!({ "configChoices": { "config/sodium.json": "mine" } }));
    assert!(w.read("mods/sodium-1.jar").is_none(), "old version removed");
    assert_eq!(
        w.read("mods/sodium-2.jar").as_deref(),
        Some("sodium version two")
    );
    assert!(w.read("mods/iris-1.jar").is_some(), "their decision kept");
    assert!(
        w.read("config/sodium.json").unwrap().contains("fast"),
        "kept mine"
    );
    assert_eq!(d["sync"]["syncedRevision"], 2);
    assert_eq!(d["sync"]["state"], "up_to_date");
}

#[test]
fn revert_and_optional_items() {
    let w = World::new();
    w.link();
    w.sync(json!({}));
    let d = w.detail();
    let sodium = World::item(&d, "mods/sodium-1.jar").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let iris = World::item(&d, "mods/iris-1.jar").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // A viewer disabling a required file is a local change; revert undoes it.
    w.app.ok(
        "set_content_enabled",
        json!({ "instanceId": w.id, "contentId": sodium, "enabled": false }),
    );
    let d = w.detail();
    assert_eq!(d["sync"]["state"], "local_changes");
    assert_eq!(d["sync"]["localChangeCount"], 1);
    let after = w
        .app
        .ok("revert_local_changes", json!({ "instanceId": w.id }));
    assert_eq!(after["sync"]["state"], "up_to_date");
    assert!(w.read("mods/sodium-1.jar").is_some());
    assert!(!w.game.join("mods/sodium-1.jar.disabled").exists());

    // Optional: download, then remove again. Neither is a local change.
    w.app.ok(
        "set_optional_installed",
        json!({ "instanceId": w.id, "contentId": iris, "installed": true }),
    );
    assert_eq!(
        w.read("mods/iris-1.jar").as_deref(),
        Some("iris version one")
    );
    assert_eq!(w.detail()["sync"]["state"], "up_to_date");
    w.app.ok(
        "set_optional_installed",
        json!({ "instanceId": w.id, "contentId": iris, "installed": false }),
    );
    assert!(w.read("mods/iris-1.jar").is_none());
    let d = w.detail();
    assert_eq!(
        World::item(&d, "mods/iris-1.jar").unwrap()["installed"],
        false
    );
    assert_eq!(d["sync"]["state"], "up_to_date");

    // Only optional items can be toggled this way.
    assert_eq!(
        w.app.err(
            "set_optional_installed",
            json!({ "instanceId": w.id, "contentId": sodium, "installed": false })
        ),
        "invalid_input"
    );
}

#[test]
fn restoring_a_backup_puts_files_and_revision_back() {
    let w = World::new();
    w.link();
    w.sync(json!({}));
    w.source.file("mods/sodium-2.jar", b"sodium version two");
    w.source.revision(
        2,
        &[("mods/sodium-2.jar", "Sodium", "AANobbMI", true)],
        &["config/sodium.json"],
    );
    let d = w.sync(json!({}));
    assert!(w.read("mods/sodium-2.jar").is_some());

    // Newest backup = the one taken before updating to v2.
    let backup = d["backups"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(d["backups"][0]["fromRevision"], 1);
    let started = w.app.ok(
        "restore_backup",
        json!({ "instanceId": w.id, "backupId": backup }),
    );
    assert_eq!(started["status"], "syncing");
    let d = w.wait();
    assert_eq!(
        w.read("mods/sodium-1.jar").as_deref(),
        Some("sodium version one")
    );
    assert!(w.read("mods/sodium-2.jar").is_none());
    assert_eq!(d["sync"]["syncedRevision"], 1);
    assert_eq!(
        d["sync"]["state"], "update_available",
        "v2 is still out there"
    );
    assert_eq!(
        d["backups"][0]["reason"], "before_restore",
        "the restore can be undone too"
    );
    assert!(
        d["backups"].as_array().unwrap().len() <= 3,
        "only the last 3 are kept"
    );
}

#[test]
fn a_failed_update_changes_nothing() {
    let w = World::new();
    w.link();
    w.sync(json!({}));
    // Revision 2 lists a file the source doesn't have and that has no download link.
    w.source.file("mods/ghost.jar", b"ghost");
    w.source.revision(
        2,
        &[
            ("mods/sodium-1.jar", "Sodium", "AANobbMI", true),
            ("mods/ghost.jar", "Ghost", "ghost", true),
        ],
        &[],
    );
    std::fs::remove_file(w.source.path().join("files/mods/ghost.jar")).unwrap();

    let d = w.sync(json!({}));
    assert_eq!(d["status"], "ready");
    assert_eq!(d["sync"]["syncedRevision"], 1, "still on the old version");
    assert_eq!(d["sync"]["state"], "update_available");
    assert!(w.read("mods/ghost.jar").is_none());
    assert!(w.read("mods/sodium-1.jar").is_some());
    assert!(!w.game.join(".atlas-staging").exists());
}

#[test]
fn an_interrupted_update_is_undone_at_startup() {
    let w = World::new();
    // As if Atlas died mid-apply: it had removed old.jar (backed up) and added new.jar.
    std::fs::write(w.game.join("mods/new.jar"), b"half-applied").unwrap();
    let backup = w.game.parent().unwrap().join("backup");
    std::fs::create_dir_all(backup.join("mods")).unwrap();
    std::fs::write(backup.join("mods/old.jar"), b"the original").unwrap();
    let staging = w.game.join(".atlas-staging/job1");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(
        staging.join("journal.json"),
        serde_json::to_vec(&json!({
            "backup_dir": backup,
            "done": [{ "path": "mods/old.jar", "action": "removed" }, { "path": "mods/new.jar", "action": "added" }]
        }))
        .unwrap(),
    )
    .unwrap();

    let recovered = tauri::async_runtime::block_on(atlas_lib::sync::recovery::run(&w.app.state()));
    assert_eq!(recovered, std::slice::from_ref(&w.id));
    assert!(w.read("mods/new.jar").is_none());
    assert_eq!(w.read("mods/old.jar").as_deref(), Some("the original"));
    assert!(!w.game.join(".atlas-staging").exists());
}

#[test]
fn guards() {
    let w = World::new();
    // Not shared yet.
    assert_eq!(
        w.app.err("preview_sync", json!({ "instanceId": w.id })),
        "conflict"
    );
    assert_eq!(
        w.app.err(
            "sync_instance",
            json!({ "instanceId": w.id, "options": {} })
        ),
        "conflict"
    );
    // A folder without revisions can't be linked.
    let empty = TempDir::new();
    assert_eq!(
        w.app.err("dev_link_manifest", json!({ "instanceId": w.id, "folder": empty.0, "role": "viewer", "adoptRevision": null })),
        "invalid_input"
    );
    // While the game runs, the update waits (it doesn't fail), and file commands refuse.
    w.link();
    w.app.state().running.insert(&w.id);
    w.app.ok(
        "sync_instance",
        json!({ "instanceId": w.id, "options": {} }),
    );
    std::thread::sleep(Duration::from_millis(300));
    let d = w.detail();
    assert_eq!(d["status"], "syncing");
    assert_eq!(d["progress"]["stage"], "waiting_for_game_exit");
    assert_eq!(
        w.app.err("untrack_instance", json!({ "id": w.id })),
        "conflict"
    );
    assert_eq!(
        w.app.err(
            "sync_instance",
            json!({ "instanceId": w.id, "options": {} })
        ),
        "conflict",
        "one job at a time"
    );
    w.app.state().running.remove(&w.id);
    assert_eq!(w.wait()["sync"]["state"], "up_to_date");
}

#[test]
fn the_demo_dev_source_in_the_repo_works() {
    let app = TestApp::new();
    let dir = TempDir::new();
    let game = dir.0.join("Demo");
    std::fs::create_dir_all(game.join("mods")).unwrap();
    let id = app.ok("track_folder", json!({ "gameDir": game }))["id"]
        .as_str()
        .unwrap()
        .to_string();
    let folder = common::fixture("dev-source");
    app.ok(
        "dev_link_manifest",
        json!({ "instanceId": id, "folder": folder, "role": "viewer", "adoptRevision": null }),
    );
    app.ok(
        "sync_instance",
        json!({ "instanceId": id, "options": { "installOptional": ["atlasextras"] } }),
    );
    let start = Instant::now();
    while app.ok("get_instance", json!({ "id": id }))["status"] == "syncing" {
        assert!(start.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(game.join("resourcepacks/Atlas Demo 2.0.zip").is_file());
    assert!(game.join("resourcepacks/Atlas Extras.zip").is_file());
    assert!(
        std::fs::read_to_string(game.join("config/atlas-demo.json"))
            .unwrap()
            .contains("hello again")
    );
    assert_eq!(
        app.ok("get_instance", json!({ "id": id }))["sync"]["state"],
        "up_to_date"
    );
}
