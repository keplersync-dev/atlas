//! The sync engine: manifests, sync state, applying versions to folders.

pub mod adopt;
pub mod apply;
pub mod backup;
pub mod fetch;
pub mod plan;
pub mod prism_versions;
pub mod recovery;
pub mod source;
pub mod state;

pub use atlas_protocol::manifest;
