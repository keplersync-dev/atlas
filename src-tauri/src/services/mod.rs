//! The logic behind the commands: read the database and launchers, build the DTOs the
//! UI expects, emit events. One file per section of src/lib/api/api.ts.

pub mod instances;
pub mod launchers;

/// Path as stored and shown: canonical (real case, no `\\?\` prefix) when it exists.
pub fn clean_path(path: &std::path::Path) -> String {
    dunce::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}
