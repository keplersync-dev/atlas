//! Launcher detection on a made-up PC: its own test binary, because it points the home and
//! app data folders at temporary ones.

mod common;

use std::path::Path;

use common::{TempDir, TestApp, clean, fixture};
use serde_json::{Value, json};

fn copy_dir(from: &Path, to: &Path) {
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

fn curseforge(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .filter(|l| l["kind"] == "curseforge")
        .map(|l| l["rootPath"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn curseforge_moved_in_its_settings_is_found_in_the_new_folder() {
    let pc = TempDir::new();
    let (home, appdata, f) = (pc.0.join("home"), pc.0.join("appdata"), pc.0.join("F"));
    std::fs::create_dir_all(&appdata).unwrap();
    // SAFETY: this binary has only this test, so nothing reads the environment meanwhile.
    unsafe {
        std::env::set_var("USERPROFILE", &home);
        std::env::set_var("HOME", &home);
        std::env::set_var("APPDATA", &appdata);
    }
    let old = home.join("curseforge/minecraft");
    copy_dir(&fixture("curseforge"), &old);

    let app = TestApp::with(|s| s.auto_detect = true);
    assert_eq!(
        curseforge(&app.ok("list_launchers", json!({}))),
        [clean(&old)]
    );
    let added = app.ok(
        "track_instances",
        json!({ "refs": [{ "launcherKind": "curseforge", "launcherInstanceId": "SkyFactory 4 (1)" }] }),
    );
    let id = added[0]["id"].as_str().unwrap().to_string();

    // CurseForge moves its folder to F: and leaves the old one with an empty Instances/.
    app.state().watcher.unwatch(&id);
    let new = f.join("curseforge");
    copy_dir(&old, &new);
    std::fs::remove_dir_all(old.join("Instances")).unwrap();
    std::fs::create_dir_all(old.join("Instances")).unwrap();
    let settings = json!({ "minecraftRoot": new }).to_string();
    std::fs::create_dir_all(appdata.join("CurseForge")).unwrap();
    std::fs::write(
        appdata.join("CurseForge/storage.json"),
        json!({ "minecraft-settings": settings }).to_string(),
    )
    .unwrap();

    let launchers = app.ok("detect_launchers", json!({}));
    assert_eq!(
        curseforge(&launchers),
        [clean(&new)],
        "only the new folder: {launchers:#}"
    );
    let moved = app.ok("rescan_instance", json!({ "id": id }));
    assert_eq!(moved["status"], "ready");
    assert_eq!(
        moved["gameDir"],
        clean(&new.join("Instances/SkyFactory 4 (1)"))
    );
    assert_eq!(moved["launcher"]["kind"], "curseforge");
}
