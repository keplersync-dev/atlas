//! The one error type every command returns.
//!
//! The UI turns a rejected `invoke` into `ApiError(code, message)` (src/lib/api/tauri.ts),
//! so `AppError` serializes as `{ "code": "...", "message": "..." }`. Codes match
//! `ApiErrorCode` in src/lib/api/api.ts. Messages are shown to players as they are.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    NotSignedIn,
    Forbidden,
    Offline,
    Conflict,
    InvalidInput,
    RateLimited,
    QuotaExceeded,
    Unknown,
}

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, m)
    }
    pub fn not_signed_in() -> Self {
        Self::new(ErrorCode::NotSignedIn, "Sign in to use friends and sharing")
    }
    pub fn forbidden(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, m)
    }
    pub fn offline() -> Self {
        Self::new(ErrorCode::Offline, "You're offline")
    }
    pub fn conflict(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, m)
    }
    pub fn invalid_input(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, m)
    }
    pub fn unknown(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unknown, m)
    }
    /// For local commands whose phase hasn't been built yet (docs/tauri-plan.md §20).
    pub fn not_built() -> Self {
        Self::unknown("Not available yet")
    }
}

// Unexpected errors: log the detail for us, show the player something short.

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = ?e, "database error");
        Self::unknown("Database error")
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        tracing::error!(error = ?e, "migration error");
        Self::unknown("Couldn't update Atlas's database")
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match e.kind() {
            ErrorKind::NotFound => Self::not_found("File or folder not found"),
            ErrorKind::PermissionDenied => {
                Self::conflict("A file is in use. Close the launcher and try again.")
            }
            _ => {
                tracing::error!(error = ?e, "io error");
                Self::unknown("Couldn't read or write a file")
            }
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        tracing::error!(error = ?e, "json error");
        Self::unknown("Couldn't read saved data")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_like_the_ui_expects() {
        let json = serde_json::to_value(AppError::conflict("Close the game first")).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "code": "conflict", "message": "Close the game first" })
        );
        let json = serde_json::to_value(AppError::not_signed_in()).unwrap();
        assert_eq!(json["code"], "not_signed_in");
    }
}
