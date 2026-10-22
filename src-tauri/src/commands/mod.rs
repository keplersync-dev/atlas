//! `#[tauri::command]` wrappers, one file per section of src/lib/api/api.ts.
//!
//! Commands stay thin: parse arguments, call one service function, return. Names and
//! argument names must match src/lib/api/tauri.ts (Tauri turns the UI's camelCase keys
//! into these snake_case parameters). tests/contract.rs checks that for every command.
//!
//! Commands that aren't built yet answer "Not available yet"; social ones ask you to
//! sign in first (see [`not_ready`]).

pub mod activity;
pub mod friends;
pub mod groups;
pub mod instances;
pub mod launchers;
pub mod session;
pub mod sharing;

/// The answer of a social command the server can't do yet: sign in first, or, when
/// signed in, `message` (that the feature is coming).
pub(crate) fn not_ready(state: &crate::state::AppState, message: &str) -> crate::error::AppError {
    if state.cloud.is_signed_in() {
        crate::error::AppError::unknown(message)
    } else {
        crate::error::AppError::not_signed_in()
    }
}
