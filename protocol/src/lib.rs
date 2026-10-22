//! Types and rules shared by the Atlas desktop app and the Atlas server, so both sides
//! parse, validate and hash exactly the same way.

pub mod api;
pub mod codes;
pub mod enums;
pub mod manifest;
pub mod pkce;

/// UUIDv7 string.
pub type Id = String;
/// ISO-8601 UTC string with millis, e.g. "2026-10-19T16:39:00.000Z".
pub type IsoDate = String;
