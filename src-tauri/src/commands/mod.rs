//! `#[tauri::command]` wrappers, one file per section of src/lib/api/api.ts.
//!
//! Commands stay thin: parse arguments, call one service function, return. Names and
//! argument names must match src/lib/api/tauri.ts (Tauri turns the UI's camelCase keys
//! into these snake_case parameters). tests/contract.rs checks that for every command.

pub mod activity;
pub mod friends;
pub mod groups;
pub mod instances;
pub mod launchers;
pub mod session;
pub mod sharing;
