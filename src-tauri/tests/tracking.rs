//! End to end, through the same IPC the UI uses: add launchers (the fixture
//! installs), see their instances, track some, change preferences, untrack.

mod common;

use common::{TempDir, TestApp, clean, fixture};
use serde_json::{Value, json};

fn by_name<'a>(list: &'a Value, name: &str) -> &'a Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == name)
        .unwrap_or_else(|| panic!("no {name} in {list}"))
}

#[test]
fn launchers_can_be_added_once_and_only_where_they_exist() {
    let app = TestApp::new();
    let prism = fixture("prism");

    let launcher = app.ok(
        "add_launcher",
        json!({ "kind": "prism", "rootPath": prism }),
    );
    assert_eq!(launcher["kind"], "prism");
    assert_eq!(launcher["name"], "Prism Launcher");
    assert_eq!(launcher["rootPath"], clean(&prism));
    assert_eq!(launcher["instanceCount"], 2);
    assert_eq!(launcher["canCreateInstances"], true);
    assert_eq!(launcher["addedByUser"], true);

    assert_eq!(
        app.err(
            "add_launcher",
            json!({ "kind": "prism", "rootPath": prism })
        ),
        "conflict"
    );
    assert_eq!(
        app.err(
            "add_launcher",
            json!({ "kind": "curseforge", "rootPath": prism })
        ),
        "invalid_input",
        "a Prism folder isn't a CurseForge install"
    );
    assert_eq!(
        app.err(
            "add_launcher",
            json!({ "kind": "folder", "rootPath": prism })
        ),
        "invalid_input"
    );

    let id = launcher["id"].as_str().unwrap();
    app.ok("remove_launcher", json!({ "id": id }));
    assert_eq!(app.ok("list_launchers", json!({})), json!([]));
    assert_eq!(app.err("remove_launcher", json!({ "id": id })), "not_found");
}

#[test]
fn track_list_update_and_untrack() {
    let app = TestApp::new();
    for (kind, dir) in [
        ("prism", "prism"),
        ("curseforge", "curseforge"),
        ("official", "official"),
    ] {
        app.ok(
            "add_launcher",
            json!({ "kind": kind, "rootPath": fixture(dir) }),
        );
    }

    // Everything in every launcher, nothing tracked yet.
    let detected = app.ok("list_detected_instances", json!({}));
    assert_eq!(detected.as_array().unwrap().len(), 6, "{detected:#}");
    assert!(
        detected
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["instanceId"].is_null())
    );
    let sky = by_name(&detected, "SkyFactory 4 fixed");
    assert_eq!(sky["launcherKind"], "curseforge");
    assert_eq!(sky["launcherInstanceId"], "SkyFactory 4 (1)");
    assert_eq!(sky["loader"], "forge");

    // Track two, the way the Track dialog sends them.
    let refs = json!([
        { "launcherKind": "prism", "launcherInstanceId": "Survival SMP" },
        { "launcherKind": "curseforge", "launcherInstanceId": "SkyFactory 4 (1)" },
        { "launcherKind": "prism", "launcherInstanceId": "Not there" },
    ]);
    let added = app.ok("track_instances", json!({ "refs": refs }));
    assert_eq!(
        added.as_array().unwrap().len(),
        2,
        "the missing one is skipped: {added:#}"
    );
    let smp = by_name(&added, "Survival SMP");
    assert_eq!(
        smp["launcher"],
        json!({ "kind": "prism", "name": "Prism Launcher" })
    );
    assert_eq!(
        smp["gameDir"],
        clean(&fixture("prism/instances/Survival SMP/minecraft"))
    );
    assert_eq!(smp["minecraftVersion"], "1.21.1");
    assert_eq!(smp["loader"], "fabric");
    assert_eq!(smp["loaderVersion"], "0.16.5");
    assert_eq!(smp["status"], "ready");
    assert_eq!(smp["totalPlaytimeSeconds"], 36000);
    assert_eq!(smp["lastPlayedAt"], "2026-10-19T00:00:00.000Z");
    assert_eq!(smp["sync"], Value::Null);
    assert_eq!(smp["isFavorite"], false);
    let smp_id = smp["id"].as_str().unwrap().to_string();

    // Tracking again changes nothing; the detected list now knows which are tracked.
    assert_eq!(
        app.ok("track_instances", json!({ "refs": refs })),
        json!([])
    );
    let detected = app.ok("list_detected_instances", json!({}));
    assert_eq!(
        by_name(&detected, "Survival SMP")["instanceId"],
        smp_id.as_str()
    );
    assert!(by_name(&detected, "Old Vanilla")["instanceId"].is_null());

    assert_eq!(
        app.ok("list_instances", json!({}))
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // The instance page.
    let detail = app.ok("get_instance", json!({ "id": smp_id }));
    assert_eq!(detail["name"], "Survival SMP");
    assert_eq!(detail["content"], json!([]));
    assert_eq!(detail["backups"], json!([]));
    assert_eq!(
        app.err("get_instance", json!({ "id": "nope" })),
        "not_found"
    );

    // Atlas-only preferences.
    let updated = app.ok(
        "update_instance",
        json!({ "id": smp_id, "patch": { "isFavorite": true } }),
    );
    assert_eq!(updated["isFavorite"], true);
    assert_eq!(updated["hideActivity"], false);
    let updated = app.ok(
        "update_instance",
        json!({ "id": smp_id, "patch": { "hideActivity": true } }),
    );
    assert_eq!(
        (
            updated["isFavorite"].clone(),
            updated["hideActivity"].clone()
        ),
        (json!(true), json!(true))
    );

    // Rescan re-reads the launcher; the folder is there.
    let rescanned = app.ok("rescan_instance", json!({ "id": smp_id }));
    assert_eq!(rescanned["status"], "ready");
    assert!(rescanned["lastScannedAt"].is_string());

    // Untrack: gone from Atlas, still in the launcher (the folder is never touched).
    app.ok("untrack_instance", json!({ "id": smp_id }));
    assert_eq!(
        app.err("get_instance", json!({ "id": smp_id })),
        "not_found"
    );
    assert!(fixture("prism/instances/Survival SMP/instance.cfg").is_file());
    assert!(
        by_name(
            &app.ok("list_detected_instances", json!({})),
            "Survival SMP"
        )["instanceId"]
            .is_null()
    );
}

#[test]
fn track_a_folder_by_hand() {
    let app = TestApp::new();
    let dir = TempDir::new();

    let empty = dir.0.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert_eq!(
        app.err("track_folder", json!({ "gameDir": empty })),
        "invalid_input"
    );
    assert_eq!(
        app.err("track_folder", json!({ "gameDir": dir.0.join("missing") })),
        "invalid_input"
    );

    let game = dir.0.join("My Pack");
    std::fs::create_dir_all(game.join("mods")).unwrap();
    let tracked = app.ok("track_folder", json!({ "gameDir": game }));
    assert_eq!(tracked["name"], "My Pack");
    assert_eq!(
        tracked["launcher"],
        json!({ "kind": "folder", "name": "Folder" })
    );
    assert_eq!(tracked["minecraftVersion"], Value::Null);

    // Same folder again, even with different letter case.
    let upper = game.to_string_lossy().to_uppercase();
    assert_eq!(
        app.err("track_folder", json!({ "gameDir": upper })),
        "conflict"
    );

    // A hand-added folder has no launcher to open.
    let id = tracked["id"].as_str().unwrap();
    assert_eq!(app.err("open_in_launcher", json!({ "id": id })), "conflict");

    // Folder deleted outside Atlas: a rescan marks it missing.
    std::fs::remove_dir_all(&game).unwrap();
    assert_eq!(
        app.ok("rescan_instance", json!({ "id": id }))["status"],
        "missing"
    );
    assert_eq!(
        app.err("open_instance_folder", json!({ "id": id })),
        "conflict"
    );
}
