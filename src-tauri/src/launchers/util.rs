//! Small parsers shared by the adapters.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, NaiveDateTime, Utc};

use crate::dto::Loader;
use crate::util::time::iso;

/// An environment folder like %APPDATA%, if set.
pub fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

/// The first of these paths that exists.
pub fn first_existing(paths: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|p| p.exists())
}

/// Minimal INI reader for Qt-style files (Prism's instance.cfg, prismlauncher.cfg):
/// `key=value` lines, `[sections]` and `#;` comments ignored, surrounding quotes removed.
pub fn parse_ini(text: &str) -> HashMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with(['[', '#', ';']))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| {
            let v = v.trim();
            let v = v
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(v);
            (k.trim().to_string(), v.to_string())
        })
        .collect()
}

/// Loader names as launchers write them ("fabric", "NeoForge", "net.neoforged", ...).
pub fn loader_from(name: &str) -> Option<Loader> {
    match name.to_ascii_lowercase().as_str() {
        "vanilla" | "minecraft" => Some(Loader::Vanilla),
        "fabric" => Some(Loader::Fabric),
        "quilt" => Some(Loader::Quilt),
        "forge" => Some(Loader::Forge),
        "neoforge" => Some(Loader::Neoforge),
        _ => None,
    }
}

/// Timestamps in the formats launchers use, as ISO UTC. Placeholder dates
/// ("0001-01-01", the 1970 epoch) mean "never" and give None.
pub fn parse_time(s: &str) -> Option<String> {
    let s = s.trim();
    let parsed = DateTime::parse_from_rfc3339(s)
        .or_else(|_| DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .map(|t| t.with_timezone(&Utc))
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f").map(|t| t.and_utc()))
        .ok()?;
    real_date(parsed)
}

/// Unix seconds as ISO UTC (None for 0 / placeholder values).
pub fn from_unix_seconds(secs: i64) -> Option<String> {
    DateTime::from_timestamp(secs, 0).and_then(real_date)
}

/// Unix milliseconds as ISO UTC (None for 0 / placeholder values).
pub fn from_unix_millis(ms: i64) -> Option<String> {
    DateTime::from_timestamp_millis(ms).and_then(real_date)
}

fn real_date(t: DateTime<Utc>) -> Option<String> {
    (t.timestamp() > 946_684_800).then(|| iso(t)) // after 2000-01-01
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_in_every_launcher_format() {
        assert_eq!(
            parse_time("2025-12-31T13:02:19.217Z").unwrap(),
            "2025-12-31T13:02:19.217Z"
        );
        // official launcher, offset without a colon
        assert_eq!(
            parse_time("2024-11-01T23:43:39+1100").unwrap(),
            "2024-11-01T12:43:39.000Z"
        );
        // CurseForge, 7 decimal places
        assert_eq!(
            parse_time("2026-01-23T10:03:20.0904994Z").unwrap(),
            "2026-01-23T10:03:20.090Z"
        );
        // no zone at all: taken as UTC
        assert_eq!(
            parse_time("2026-01-23T10:03:20").unwrap(),
            "2026-01-23T10:03:20.000Z"
        );
        // placeholders mean "never"
        assert_eq!(parse_time("0001-01-01T00:00:00"), None);
        assert_eq!(parse_time("1970-01-01T00:00:00.000Z"), None);
        assert_eq!(parse_time("not a date"), None);
        assert_eq!(
            from_unix_seconds(1_782_206_545).unwrap(),
            "2026-06-23T09:22:25.000Z"
        );
        assert_eq!(from_unix_millis(0), None);
    }

    #[test]
    fn ini_with_sections_quotes_and_comments() {
        let ini = parse_ini(
            "[General]\n# comment\nname=\"Survival SMP\"\nInstanceType=OneSix\n\nbad line\n",
        );
        assert_eq!(ini["name"], "Survival SMP");
        assert_eq!(ini["InstanceType"], "OneSix");
        assert_eq!(ini.len(), 2);
    }
}
