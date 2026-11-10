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

/// Let the first read of a just-tracked instance finish (a rescan waits for it), then stop
/// watching its folder: Windows won't move a folder Atlas is reading or watching, so these
/// moves happen as if Atlas were closed.
fn settle(app: &TestApp, id: &str) {
    app.ok("rescan_instance", json!({ "id": id }));
    app.state().watcher.unwatch(id);
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
    assert_eq!(
        launcher["creatableLoaders"],
        json!(["vanilla", "fabric", "quilt", "forge", "neoforge"])
    );
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

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

#[test]
fn instances_follow_their_launcher_to_another_drive() {
    let app = TestApp::new();
    let (a, b, c) = (TempDir::new(), TempDir::new(), TempDir::new());
    let prism = a.0.join("PrismLauncher");
    copy_dir(&fixture("prism"), &prism);
    app.ok(
        "add_launcher",
        json!({ "kind": "prism", "rootPath": prism }),
    );
    let added = app.ok(
        "track_instances",
        json!({ "refs": [{ "launcherKind": "prism", "launcherInstanceId": "Survival SMP" }] }),
    );
    let id = by_name(&added, "Survival SMP")["id"]
        .as_str()
        .unwrap()
        .to_string();

    // The launcher's instances folder moved in its settings.
    settle(&app, &id);
    std::fs::rename(prism.join("instances"), b.0.join("instances")).unwrap();
    std::fs::write(
        prism.join("prismlauncher.cfg"),
        format!("InstanceDir={}\n", b.0.join("instances").display()),
    )
    .unwrap();
    let moved = app.ok("rescan_instance", json!({ "id": id }));
    assert_eq!(moved["status"], "ready");
    assert_eq!(
        moved["gameDir"],
        clean(&b.0.join("instances/Survival SMP/minecraft"))
    );

    // The whole launcher moved, and was added again from its new place.
    let again = c.0.join("PrismLauncher");
    settle(&app, &id);
    std::fs::rename(&prism, &again).unwrap();
    std::fs::rename(b.0.join("instances"), again.join("instances")).unwrap();
    std::fs::write(again.join("prismlauncher.cfg"), "InstanceDir=instances\n").unwrap();
    assert_eq!(
        app.ok("rescan_instance", json!({ "id": id }))["status"],
        "missing"
    );
    app.ok(
        "add_launcher",
        json!({ "kind": "prism", "rootPath": again }),
    );
    let found = app.ok("rescan_instance", json!({ "id": id }));
    assert_eq!(found["status"], "ready");
    assert_eq!(
        found["gameDir"],
        clean(&again.join("instances/Survival SMP/minecraft"))
    );
    assert_eq!(
        found["totalPlaytimeSeconds"], 36000,
        "still the same instance"
    );
}

/// A Modrinth App folder (only what Atlas reads) with one instance, its database in `root`
/// and its instances in `data`, the way the app keeps them once its folder has moved.
fn modrinth_app(root: &std::path::Path, data: &std::path::Path) {
    use sqlx::{ConnectOptions, Executor};
    std::fs::create_dir_all(data.join("profiles/Aged/mods")).unwrap();
    std::fs::write(data.join("profiles/Aged/mods/sodium.jar"), b"jar").unwrap();
    std::fs::create_dir_all(root).unwrap();
    tauri::async_runtime::block_on(async {
        let mut c = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(root.join("app.db"))
            .create_if_missing(true)
            .connect()
            .await
            .unwrap();
        let custom = if data == root {
            "NULL".to_string()
        } else {
            format!("'{}'", data.display())
        };
        let sql: &'static str = Box::leak(
            format!(
                "CREATE TABLE instances (id TEXT, path TEXT, applied_content_set_id TEXT, install_stage TEXT,
                    name TEXT, icon_path TEXT, last_played INTEGER, submitted_time_played INTEGER, recent_time_played INTEGER);
                 CREATE TABLE instance_content_sets (id TEXT, game_version TEXT, loader TEXT, loader_version TEXT);
                 CREATE TABLE settings (id INTEGER, custom_dir TEXT, prev_custom_dir TEXT);
                 INSERT INTO instances VALUES ('local:aged', 'Aged', 'set', 'installed', 'Aged', NULL, 1780559691, 100, 20);
                 INSERT INTO instance_content_sets VALUES ('set', '1.21.1', 'fabric', '0.16.9');
                 INSERT INTO settings VALUES (0, {custom}, {custom});"
            )
            .into_boxed_str(),
        );
        c.execute(sql).await.unwrap();
    });
}

#[test]
fn modrinth_instances_follow_the_apps_folder_to_another_drive() {
    for old_atlas_opened_in_between in [false, true] {
        let app = TestApp::new();
        let (c, f) = (TempDir::new(), TempDir::new());
        let root = c.0.join("ModrinthApp");
        modrinth_app(&root, &root);
        app.ok(
            "add_launcher",
            json!({ "kind": "modrinth", "rootPath": root }),
        );
        let added = app.ok(
            "track_instances",
            json!({ "refs": [{ "launcherKind": "modrinth", "launcherInstanceId": "Aged" }] }),
        );
        let id = by_name(&added, "Aged")["id"].as_str().unwrap().to_string();
        assert_eq!(added[0]["gameDir"], clean(&root.join("profiles/Aged")));

        // The app moves its folder to F: and keeps only its database behind.
        settle(&app, &id);
        std::fs::remove_dir_all(&root).unwrap();
        modrinth_app(&root, &f.0.join("ModrinthApp"));
        if old_atlas_opened_in_between {
            // Atlas 0.5.0 saw no profiles folder there and dropped the launcher.
            let db = app.state().db;
            tauri::async_runtime::block_on(sqlx::query("DELETE FROM launcher").execute(&db))
                .unwrap();
            app.ok(
                "add_launcher",
                json!({ "kind": "modrinth", "rootPath": root }),
            );
        }

        let moved = app.ok("rescan_instance", json!({ "id": id }));
        assert_eq!(
            moved["status"], "ready",
            "opened in between: {old_atlas_opened_in_between}"
        );
        assert_eq!(
            moved["gameDir"],
            clean(&f.0.join("ModrinthApp/profiles/Aged"))
        );
        assert_eq!(moved["launcher"]["kind"], "modrinth");
        assert_eq!(
            moved["totalPlaytimeSeconds"], 120,
            "still the same instance"
        );
        let detail = app.ok("get_instance", json!({ "id": id }));
        assert!(
            detail.to_string().contains("sodium"),
            "its mods are read from F: {detail}"
        );
    }
}

/// A Minecraft Launcher folder whose installations each have their own game directory.
fn official_launcher(root: &std::path::Path, installs: &[(&str, &str, &std::path::Path)]) {
    std::fs::create_dir_all(root).unwrap();
    let mut profiles = serde_json::Map::new();
    for (id, name, dir) in installs {
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        profiles.insert(
            id.to_string(),
            json!({ "name": name, "type": "custom", "gameDir": dir, "lastVersionId": "fabric-loader-0.16.9-1.21.1" }),
        );
    }
    std::fs::write(
        root.join("launcher_profiles.json"),
        json!({ "profiles": profiles }).to_string(),
    )
    .unwrap();
}

#[test]
fn minecraft_launcher_installations_follow_a_new_game_directory() {
    let app = TestApp::new();
    let (c, f) = (TempDir::new(), TempDir::new());
    let root = c.0.join(".minecraft");
    let (old, new) = (c.0.join("smp"), f.0.join("smp"));
    official_launcher(
        &root,
        &[
            ("a1", "SMP", &old),
            ("b2", "Creative", &c.0.join("creative")),
        ],
    );
    app.ok(
        "add_launcher",
        json!({ "kind": "official", "rootPath": root }),
    );
    let detected = app.ok("list_detected_instances", json!({}));
    let lid = by_name(&detected, "SMP")["launcherInstanceId"].clone();
    let added = app.ok(
        "track_instances",
        json!({ "refs": [{ "launcherKind": "official", "launcherInstanceId": lid }] }),
    );
    let id = by_name(&added, "SMP")["id"].as_str().unwrap().to_string();

    // The installation's game directory is pointed at another drive.
    settle(&app, &id);
    std::fs::remove_dir_all(&old).unwrap();
    official_launcher(
        &root,
        &[
            ("a1", "SMP", &new),
            ("b2", "Creative", &c.0.join("creative")),
        ],
    );
    let moved = app.ok("rescan_instance", json!({ "id": id }));
    assert_eq!(moved["status"], "ready");
    assert_eq!(moved["gameDir"], clean(&new));
    assert_eq!(
        app.ok("rescan_instance", json!({ "id": id }))["gameDir"],
        clean(&new),
        "and it stays there"
    );

    // Two installations by that name: Atlas can't tell which, so it doesn't guess.
    settle(&app, &id);
    std::fs::remove_dir_all(&new).unwrap();
    let (x, y) = (f.0.join("smp-x"), f.0.join("smp-y"));
    official_launcher(&root, &[("a1", "SMP", &x), ("c3", "SMP", &y)]);
    assert_eq!(
        app.ok("rescan_instance", json!({ "id": id }))["status"],
        "missing"
    );
}

#[test]
fn an_instance_whose_launcher_was_dropped_is_linked_again_where_it_is() {
    // What happened on a real PC: the Modrinth App's folder moved, then Atlas 0.5.0 (which
    // didn't know that) dropped the app, and 0.6.0 found it again. The instance's folder
    // never moved after that, yet it lost its launcher, and with it its modpack.
    let app = TestApp::new();
    let (c, f) = (TempDir::new(), TempDir::new());
    let root = c.0.join("ModrinthApp");
    modrinth_app(&root, &f.0.join("ModrinthApp"));
    app.ok(
        "add_launcher",
        json!({ "kind": "modrinth", "rootPath": root }),
    );
    let added = app.ok(
        "track_instances",
        json!({ "refs": [{ "launcherKind": "modrinth", "launcherInstanceId": "Aged" }] }),
    );
    let id = added[0]["id"].as_str().unwrap().to_string();
    let db = app.state().db;
    let launcher_of = || -> Option<String> {
        tauri::async_runtime::block_on(
            sqlx::query_scalar("SELECT launcher_id FROM instance WHERE id = ?")
                .bind(&id)
                .fetch_one(&db),
        )
        .unwrap()
    };
    assert!(launcher_of().is_some());

    tauri::async_runtime::block_on(sqlx::query("DELETE FROM launcher").execute(&db)).unwrap();
    assert_eq!(launcher_of(), None, "the old Atlas dropped it");
    app.ok(
        "add_launcher",
        json!({ "kind": "modrinth", "rootPath": root }),
    );
    let again = app.ok("rescan_instance", json!({ "id": id }));
    assert_eq!(again["status"], "ready");
    assert_eq!(
        again["gameDir"],
        clean(&f.0.join("ModrinthApp/profiles/Aged"))
    );
    assert!(launcher_of().is_some(), "linked to the launcher again");

    // Another install of that launcher with an instance by that id, in another folder:
    // not this one.
    tauri::async_runtime::block_on(sqlx::query("DELETE FROM launcher").execute(&db)).unwrap();
    let (c2, f2) = (TempDir::new(), TempDir::new());
    let other = c2.0.join("ModrinthApp");
    modrinth_app(&other, &f2.0.join("ModrinthApp"));
    app.ok(
        "add_launcher",
        json!({ "kind": "modrinth", "rootPath": other }),
    );
    app.ok("rescan_instance", json!({ "id": id }));
    assert_eq!(launcher_of(), None, "never a namesake elsewhere");
}
