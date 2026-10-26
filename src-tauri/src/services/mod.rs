//! The logic behind the commands: read the database and launchers, build the DTOs the
//! UI expects, emit events. One file per section of src/lib/api/api.ts.

pub mod accept;
pub mod friends;
pub mod groups;
pub mod instances;
pub mod launchers;
pub mod presence;
pub mod publish;
pub mod session;
pub mod shared;
pub mod sync;
pub mod updates;
pub mod work;

/// Show a file Atlas just saved, selected in Explorer. Best effort.
pub fn reveal<R: tauri::Runtime>(app: &tauri::AppHandle<R>, file: &std::path::Path) {
    use tauri_plugin_opener::OpenerExt;
    if let Err(e) = app.opener().reveal_item_in_dir(file) {
        tracing::info!(error = %e, "couldn't show a file in Explorer");
    }
}

/// Path as stored and shown: canonical (real case, no `\\?\` prefix) when it exists.
pub fn clean_path(path: &std::path::Path) -> String {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}
