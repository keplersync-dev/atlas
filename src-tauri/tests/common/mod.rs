//! Shared helpers for integration tests: a mock Tauri app with Atlas's real command
//! handler and an in-memory database, and a way to call commands like the UI does.
#![allow(dead_code)] // each test file uses a different subset

use std::path::{Path, PathBuf};

use atlas_lib::state::{AppPaths, AppState};
use serde_json::Value;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder};

pub struct TestApp {
    _app: App<MockRuntime>,
    pub webview: WebviewWindow<MockRuntime>,
    pub dir: TempDir,
}

impl TestApp {
    /// Launcher auto-detection is off, so results never depend on this machine.
    pub fn new() -> Self {
        let dir = TempDir::new();
        let app = mock_builder()
            .invoke_handler(atlas_lib::invoke_handler())
            .build(mock_context(noop_assets()))
            .expect("build mock app");
        let db = tauri::async_runtime::block_on(atlas_lib::db::open_in_memory()).unwrap();
        let mut state = AppState::new(db, AppPaths::new(&dir.0.join("appdata")));
        state.auto_detect = false;
        state.modrinth_api = None; // never call Modrinth from tests
        app.manage(state);
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("build webview");
        Self {
            _app: app,
            webview,
            dir,
        }
    }

    /// The app's state, for setting up situations a test can't reach through commands.
    pub fn state(&self) -> AppState {
        self._app.state::<AppState>().inner().clone()
    }

    /// Call a command the way src/lib/api/tauri.ts does: camelCase args as JSON.
    /// Ok = the command's result, Err = what the UI would receive as the error.
    pub fn call(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        get_ipc_response(
            &self.webview,
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

    /// Call and expect success.
    pub fn ok(&self, cmd: &str, args: Value) -> Value {
        self.call(cmd, args)
            .unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
    }

    /// Call and expect an `{ code, message }` error; returns the code.
    pub fn err(&self, cmd: &str, args: Value) -> String {
        match self.call(cmd, args) {
            Ok(v) => panic!("{cmd} should have failed, got {v}"),
            Err(e) => e["code"]
                .as_str()
                .unwrap_or_else(|| panic!("{cmd}: not an AppError: {e}"))
                .to_string(),
        }
    }
}

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The path as Atlas stores it (canonical, no `\\?\`).
pub fn clean(path: &Path) -> String {
    dunce::canonicalize(path)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// A temporary folder, removed when dropped.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> Self {
        let p = std::env::temp_dir().join(format!("atlas-test-{}", atlas_lib::util::ids::new_id()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
