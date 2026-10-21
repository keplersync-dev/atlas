//! Contract test: every command the UI calls exists, and accepts the arguments the
//! UI sends, with the same names and types.
//!
//! It reads src/lib/api/tauri.ts, so adding a call there without a Rust command (or
//! renaming an argument on one side only) fails here instead of at runtime. A command
//! may answer with an error (most are T0 stubs); what it must not do is be missing or
//! reject the arguments, which is what a mismatch looks like.

use std::collections::BTreeMap;

use atlas_lib::state::{AppPaths, AppState};
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};

/// `call("name", { a, b: x })` → ("name", ["a", "b"]), in file order.
fn commands_in_tauri_ts() -> Vec<(String, Vec<String>)> {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/api/tauri.ts"))
        .expect("read src/lib/api/tauri.ts");
    let call = regex::Regex::new(r#"call\(\s*"([a-z_]+)"\s*(?:,\s*\{([^}]*)\})?"#).unwrap();
    let key = regex::Regex::new(r"^\s*([A-Za-z_]\w*)").unwrap();
    call.captures_iter(&src)
        .map(|c| {
            let keys = c
                .get(2)
                .map(|args| {
                    args.as_str()
                        .split(',')
                        .filter_map(|part| key.captures(part).map(|k| k[1].to_string()))
                        .collect()
                })
                .unwrap_or_default();
            (c[1].to_string(), keys)
        })
        .collect()
}

/// A valid value for an argument, shaped like what the UI sends (see types.ts).
fn sample(command: &str, key: &str) -> Value {
    match (command, key) {
        ("set_group_member_role", "role") => json!("member"),
        (_, "role") => json!("viewer"),
        ("create_group", "input") => json!({ "name": "The Boys" }),
        ("create_group_invite", "input") => json!({ "maxUses": null, "expiresInHours": 24 }),
        ("share_instance", "input") => json!({
            "description": null, "friendIds": ["u1"], "groupIds": [], "role": "viewer",
            "shareModes": { "c1": "optional" }, "confirmedUploadRights": true
        }),
        ("publish_instance", "input") => json!({
            "changelog": "Added Iris", "shareModes": { "c1": "private" }, "confirmedUploadRights": false
        }),
        ("invite_to_shared_instance", "input") => {
            json!({ "friendIds": ["u1"], "groupIds": ["g1"], "role": "editor" })
        }
        (_, "patch") => json!({}),
        ("sync_instance", "options") => {
            json!({ "configChoices": { "config/a.json": "mine" }, "installOptional": ["AANobbMI"] })
        }
        (_, "provider") => json!("discord"),
        (_, "kind") => json!("prism"),
        (_, "mode") => json!("optional"),
        (_, "enabled" | "installed" | "accept") => json!(true),
        (_, "refs") => json!([{ "launcherKind": "prism", "launcherInstanceId": "Survival SMP" }]),
        (_, "installOptional") => json!(["AANobbMI"]),
        (_, "ids") => json!("all"),
        (_, "rootPath" | "gameDir") => json!("C:\\Games\\Minecraft"),
        (_, "code") => json!("7KQ29XMB"),
        (_, k) if k == "id" || k.ends_with("Id") => json!("0192a7e2-0000-7000-8000-000000000000"),
        _ => panic!("no sample value for `{key}` in `{command}`: add one to sample()"),
    }
}

fn mock_app() -> (tauri::App<tauri::test::MockRuntime>, WebviewWindow<tauri::test::MockRuntime>, tempdir::Dir) {
    let dir = tempdir::Dir::new();
    let app = mock_builder()
        .invoke_handler(atlas_lib::invoke_handler())
        .build(mock_context(noop_assets()))
        .expect("build mock app");
    let db = tauri::async_runtime::block_on(atlas_lib::db::open_in_memory()).unwrap();
    app.manage(AppState { db, paths: AppPaths::new(&dir.0) });
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build webview");
    (app, webview, dir)
}

fn invoke(webview: &WebviewWindow<tauri::test::MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize::<Value>().unwrap())
}

const ERROR_CODES: [&str; 9] = [
    "not_found", "not_signed_in", "forbidden", "offline", "conflict",
    "invalid_input", "rate_limited", "quota_exceeded", "unknown",
];

#[test]
fn every_ui_call_reaches_a_command_that_accepts_its_arguments() {
    let calls = commands_in_tauri_ts();
    assert!(calls.len() > 60, "parsed only {} calls from tauri.ts", calls.len());

    let (_app, webview, _dir) = mock_app();
    let mut problems = BTreeMap::new();
    for (cmd, keys) in &calls {
        let args: serde_json::Map<String, Value> =
            keys.iter().map(|k| (k.clone(), sample(cmd, k))).collect();
        match invoke(&webview, cmd, Value::Object(args)) {
            Ok(_) => {}
            // A stub's answer: { code, message } with a code the UI knows.
            Err(Value::Object(e)) if e.get("code").and_then(Value::as_str).is_some_and(|c| ERROR_CODES.contains(&c)) => {}
            // Anything else is Tauri saying the command is missing or the args don't fit.
            Err(other) => {
                problems.insert(cmd.clone(), other.to_string());
            }
        }
    }
    assert!(problems.is_empty(), "commands that don't match tauri.ts:\n{problems:#?}");
}

#[test]
fn t0_answers_look_like_an_empty_signed_out_app() {
    let (_app, webview, _dir) = mock_app();
    assert_eq!(invoke(&webview, "get_session", json!({})).unwrap(), json!({ "user": null, "online": true }));
    assert_eq!(invoke(&webview, "list_instances", json!({})).unwrap(), json!([]));
    assert_eq!(invoke(&webview, "list_launchers", json!({})).unwrap(), json!([]));
    assert_eq!(invoke(&webview, "list_detected_instances", json!({})).unwrap(), json!([]));
    let err = invoke(&webview, "list_friends", json!({})).unwrap_err();
    assert_eq!(err["code"], "not_signed_in");
}

#[test]
fn wrong_argument_names_are_caught() {
    // Proves the first test can fail: snake_case keys are not what Tauri expects.
    let (_app, webview, _dir) = mock_app();
    let err = invoke(&webview, "set_share_mode", json!({ "instance_id": "a", "content_id": "b", "mode": "required" }))
        .unwrap_err();
    assert!(err.as_str().is_some_and(|s| s.contains("instanceId")), "{err}");
}

/// A temporary folder for AppPaths, removed when dropped. (Avoids a dev-dependency.)
mod tempdir {
    pub struct Dir(pub std::path::PathBuf);
    impl Dir {
        pub fn new() -> Self {
            let p = std::env::temp_dir().join(format!("atlas-test-{}", atlas_lib::util::ids::new_id()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
