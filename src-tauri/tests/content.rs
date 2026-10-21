//! End to end over IPC: an instance's files show up after a scan, can be turned
//! on and off, get share settings, and are protected while the game runs.

mod common;

use common::{TempDir, TestApp};
use serde_json::{Value, json};

fn item<'a>(detail: &'a Value, path: &str) -> &'a Value {
    detail["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["path"] == path)
        .unwrap_or_else(|| panic!("no {path} in {detail:#}"))
}

/// A tracked folder with two mods (one disabled) and a resource pack, scanned.
fn setup() -> (TestApp, TempDir, String) {
    let app = TestApp::new();
    let dir = TempDir::new();
    let game = dir.0.join("Pack");
    std::fs::create_dir_all(game.join("mods")).unwrap();
    std::fs::create_dir_all(game.join("resourcepacks")).unwrap();
    std::fs::write(game.join("mods/sodium.jar"), b"sodium").unwrap();
    std::fs::write(game.join("mods/iris.jar.disabled"), b"iris").unwrap();
    std::fs::write(game.join("resourcepacks/Fresh Animations.zip"), b"fa").unwrap();
    let id = app.ok("track_folder", json!({ "gameDir": game }))["id"]
        .as_str()
        .unwrap()
        .to_string();
    let summary = app.ok("rescan_instance", json!({ "id": id }));
    assert_eq!(summary["contentCount"], 3);
    (app, dir, id)
}

#[test]
fn scanned_files_appear_on_the_instance_page() {
    let (app, _dir, id) = setup();
    let detail = app.ok("get_instance", json!({ "id": id }));
    assert_eq!(detail["content"].as_array().unwrap().len(), 3);

    let sodium = item(&detail, "mods/sodium.jar");
    assert_eq!(sodium["kind"], "mod");
    assert_eq!(sodium["source"], "local");
    assert_eq!(sodium["name"], "sodium");
    assert_eq!(sodium["enabled"], true);
    assert_eq!(sodium["shareMode"], "required");
    assert_eq!(sodium["publishedShareMode"], Value::Null);
    assert_eq!(sodium["isManaged"], false);
    assert_eq!(sodium["installed"], true);
    assert_eq!(sodium["sizeBytes"], 6);

    assert_eq!(
        item(&detail, "mods/iris.jar")["enabled"],
        false,
        ".disabled is stored without the suffix"
    );
    assert_eq!(
        item(&detail, "resourcepacks/Fresh Animations.zip")["kind"],
        "resourcepack"
    );
}

#[test]
fn turning_files_on_and_off_renames_them() {
    let (app, dir, id) = setup();
    let game = dir.0.join("Pack");
    let detail = app.ok("get_instance", json!({ "id": id }));
    let sodium = item(&detail, "mods/sodium.jar")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let iris = item(&detail, "mods/iris.jar")["id"]
        .as_str()
        .unwrap()
        .to_string();

    app.ok(
        "set_content_enabled",
        json!({ "instanceId": id, "contentId": sodium, "enabled": false }),
    );
    assert!(game.join("mods/sodium.jar.disabled").is_file());
    assert!(!game.join("mods/sodium.jar").exists());

    app.ok(
        "set_content_enabled",
        json!({ "instanceId": id, "contentId": iris, "enabled": true }),
    );
    assert!(game.join("mods/iris.jar").is_file());

    let detail = app.ok("get_instance", json!({ "id": id }));
    assert_eq!(item(&detail, "mods/sodium.jar")["enabled"], false);
    assert_eq!(item(&detail, "mods/iris.jar")["enabled"], true);
    assert_eq!(
        item(&detail, "mods/sodium.jar")["id"],
        sodium.as_str(),
        "same row after the rename"
    );

    // A rescan agrees with what the command recorded.
    let before = app.ok("get_instance", json!({ "id": id }))["content"].clone();
    app.ok("rescan_instance", json!({ "id": id }));
    assert_eq!(
        app.ok("get_instance", json!({ "id": id }))["content"],
        before
    );

    // Asking for the state it's already in is fine; unknown ids aren't.
    app.ok(
        "set_content_enabled",
        json!({ "instanceId": id, "contentId": iris, "enabled": true }),
    );
    assert_eq!(
        app.err(
            "set_content_enabled",
            json!({ "instanceId": id, "contentId": "nope", "enabled": true })
        ),
        "not_found"
    );
}

#[test]
fn share_settings_are_saved_per_file() {
    let (app, _dir, id) = setup();
    let detail = app.ok("get_instance", json!({ "id": id }));
    let fa = item(&detail, "resourcepacks/Fresh Animations.zip")["id"]
        .as_str()
        .unwrap()
        .to_string();

    app.ok(
        "set_share_mode",
        json!({ "instanceId": id, "contentId": fa, "mode": "optional" }),
    );
    let detail = app.ok("get_instance", json!({ "id": id }));
    assert_eq!(
        item(&detail, "resourcepacks/Fresh Animations.zip")["shareMode"],
        "optional"
    );

    // Survives a rescan.
    app.ok("rescan_instance", json!({ "id": id }));
    let detail = app.ok("get_instance", json!({ "id": id }));
    assert_eq!(
        item(&detail, "resourcepacks/Fresh Animations.zip")["shareMode"],
        "optional"
    );

    // Viewers of a shared instance can't change it.
    let state = app.state();
    tauri::async_runtime::block_on(
        sqlx::query(
            "UPDATE instance SET shared_instance_id = 's1', shared_role = 'viewer' WHERE id = ?",
        )
        .bind(&id)
        .execute(&state.db),
    )
    .unwrap();
    assert_eq!(
        app.err(
            "set_share_mode",
            json!({ "instanceId": id, "contentId": fa, "mode": "private" })
        ),
        "forbidden"
    );
}

#[test]
fn files_are_left_alone_while_the_game_runs() {
    let (app, dir, id) = setup();
    let detail = app.ok("get_instance", json!({ "id": id }));
    let sodium = item(&detail, "mods/sodium.jar")["id"]
        .as_str()
        .unwrap()
        .to_string();

    app.state().running.insert(&id);
    assert_eq!(
        app.ok("get_instance", json!({ "id": id }))["isRunning"],
        true
    );
    assert_eq!(app.ok("list_instances", json!({}))[0]["isRunning"], true);
    assert_eq!(
        app.err(
            "set_content_enabled",
            json!({ "instanceId": id, "contentId": sodium, "enabled": false })
        ),
        "conflict"
    );
    assert!(dir.0.join("Pack/mods/sodium.jar").is_file(), "untouched");
}
