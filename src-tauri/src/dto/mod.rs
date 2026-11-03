//! The shapes the UI receives and sends. src/lib/types.ts is written from them (and the
//! atlas-protocol ones they use) by typescript.rs's test.
//! Rules:
//! - structs are `rename_all = "camelCase"`, enum values are snake_case strings;
//! - `T | null` is `Option<T>` and serializes as `null` (never skipped);
//! - DTOs are separate from database rows: services convert rows into these.

pub mod activity;
pub mod app;
pub mod enums;
pub mod groups;
pub mod inputs;
pub mod instances;
pub mod people;
pub mod shared;
#[cfg(test)]
mod typescript;

pub use activity::*;
pub use app::*;
pub use enums::*;
pub use groups::*;
pub use inputs::*;
pub use instances::*;
pub use people::*;
pub use shared::*;

pub use atlas_protocol::{Id, IsoDate};
