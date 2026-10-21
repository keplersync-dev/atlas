//! `#[tauri::command]` wrappers, one file per section of src/lib/api/api.ts.
//!
//! Commands stay thin: parse arguments, call one service function, return. Names and
//! argument names must match src/lib/api/tauri.ts (Tauri turns the UI's camelCase keys
//! into these snake_case parameters). tests/contract.rs checks that for every command.
//!
//! Phase T0: everything is registered. Commands that later phases build return the
//! errors from docs/tauri-plan.md §5 for now: social ones `not_signed_in`, local ones
//! "not found" (there's nothing tracked yet) or `not_built()`.

pub mod activity;
pub mod friends;
pub mod groups;
pub mod instances;
pub mod launchers;
pub mod session;
pub mod sharing;
