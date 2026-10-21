//! The sync engine: manifests, sync state, applying versions to folders
//! (docs/tauri-plan.md §10, §12).

pub mod adopt;
pub mod apply;
pub mod backup;
pub mod fetch;
pub mod manifest;
pub mod plan;
pub mod prism_versions;
pub mod recovery;
pub mod source;
pub mod state;
