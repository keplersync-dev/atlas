//! Rust mirrors of src/lib/types.ts — the shapes the UI receives and sends.
//!
//! Keep these in the same order as types.ts so they're easy to compare side by side.
//! Rules:
//! - structs are `rename_all = "camelCase"`, enum values are snake_case strings;
//! - `T | null` is `Option<T>` and serializes as `null` (never skipped);
//! - DTOs are separate from database rows: services convert rows into these.

pub mod activity;
pub mod enums;
pub mod groups;
pub mod inputs;
pub mod instances;
pub mod people;
pub mod shared;

pub use activity::*;
pub use enums::*;
pub use groups::*;
pub use inputs::*;
pub use instances::*;
pub use people::*;
pub use shared::*;

/// UUIDv7 string (see util::ids).
pub type Id = String;
/// ISO-8601 UTC string, e.g. "2026-10-19T16:39:00.000Z" (see util::time).
pub type IsoDate = String;
