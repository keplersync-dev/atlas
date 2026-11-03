//! Small parsers shared by the adapters.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::de::DeserializeOwned;

use super::ReadResult;
use crate::dto::Loader;
use crate::util::time::iso;

/// An environment folder like %APPDATA%, if set.
pub fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

/// Where launchers keep their data: %APPDATA% on Windows, ~/Library/Application Support
/// on macOS.
pub fn app_data() -> Option<PathBuf> {
    if cfg!(windows) {
        env_dir("APPDATA")
    } else {
        home().map(|h| h.join("Library").join("Application Support"))
    }
}

/// The user's home folder.
pub fn home() -> Option<PathBuf> {
    env_dir("USERPROFILE").or_else(|| env_dir("HOME"))
}

/// A macOS app's program: `binary` inside `bundle` (like "Prism Launcher.app"), in
/// /Applications or ~/Applications.
pub fn mac_app(bundle: &str, binary: &str) -> Option<PathBuf> {
    let user = home().map(|h| h.join("Applications"));
    first_existing(
        [Some(PathBuf::from("/Applications")), user]
            .into_iter()
            .flatten()
            .map(|d| d.join(bundle).join("Contents").join("MacOS").join(binary)),
    )
}

/// The first of these paths that exists.
pub fn first_existing(paths: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|p| p.exists())
}

/// A process name to compare: lowercase, without Windows' ".exe".
pub fn program_name(name: &std::ffi::OsStr) -> String {
    let name = name.to_string_lossy().to_lowercase();
    name.strip_suffix(".exe").unwrap_or(&name).to_owned()
}

/// Is a program with one of these names (lowercase, no ".exe") running?
pub fn any_running(names: &[&str]) -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    system
        .processes()
        .values()
        .any(|p| names.contains(&program_name(p.name()).as_str()))
}

/// Is a program whose path ends in `tail` running? For programs named too plainly to tell
/// apart by name ("Minecraft.app/Contents/MacOS/launcher").
pub fn running_at(tail: &str) -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    system
        .processes()
        .values()
        .any(|p| p.exe().is_some_and(|exe| exe.ends_with(tail)))
}

/// A launcher's JSON file, which may start with a UTF-8 BOM (CurseForge's do).
pub fn read_json<T: DeserializeOwned>(file: &Path) -> ReadResult<T> {
    let text = std::fs::read_to_string(file)?;
    Ok(serde_json::from_str(text.trim_start_matches('\u{feff}'))?)
}

/// Write a launcher's file whole or not at all: next to it, on disk, then swapped in, so
/// the launcher never reads half of one.
pub fn write_atomic(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(".atlas");
    let tmp = file.with_file_name(name);
    let mut out = std::fs::File::create(&tmp)?;
    out.write_all(bytes)?;
    out.sync_all()?;
    drop(out);
    std::fs::rename(&tmp, file)
}

/// A new folder in the system's temp folder, removed with what's in it when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> std::io::Result<Self> {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
}

impl std::ops::Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A folder name for a new instance in `dir`: the name with the characters Windows
/// forbids replaced, and " (2)", " (3)", ... added when it's taken.
pub fn free_folder(dir: &Path, name: &str) -> String {
    let base = file_stem(name);
    (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base} ({n})")
            }
        })
        .find(|f| !dir.join(f).exists())
        .expect("some folder name is free")
}

/// The name, usable as a Windows file or folder name.
pub fn file_stem(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if r#"<>:"/\|?*"#.contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .to_owned();
    if safe.is_empty() {
        "Atlas".to_owned()
    } else if crate::sync::manifest::is_reserved_name(&safe) {
        // Windows reserves the part before the first dot: "nul" → "nul_", "aux.x" → "aux_.x".
        match safe.split_once('.') {
            Some((stem, rest)) => format!("{stem}_.{rest}"),
            None => format!("{safe}_"),
        }
    } else {
        safe
    }
}

/// "png", "jpg", "gif" or "webp", from the file's first bytes.
pub fn image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"GIF8") {
        Some("gif")
    } else if bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
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
            let v = match v.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
                Some(quoted) => ini_unescape(quoted),
                None => v.to_string(),
            };
            (k.trim().to_string(), v)
        })
        .collect()
}

/// A value for a Qt settings file, quoted: unquoted, Qt reads `,` as a list separator and
/// `;` as the start of a comment. Inside quotes it escapes `\` and `"`.
pub fn ini_quoted(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\r', '\n'], " ");
    format!("\"{escaped}\"")
}

/// Undo `ini_quoted`'s escapes inside a quoted value.
fn ini_unescape(quoted: &str) -> String {
    let mut out = String::with_capacity(quoted.len());
    let mut chars = quoted.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('\\', Some(&next @ ('"' | '\\'))) => {
                out.push(next);
                chars.next();
            }
            _ => out.push(c),
        }
    }
    out
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
    fn names_windows_keeps_for_devices_get_an_underscore() {
        assert_eq!(file_stem("CON"), "CON_");
        assert_eq!(file_stem("aux.pack"), "aux_.pack");
        assert_eq!(file_stem("Com1"), "Com1_");
        assert_eq!(file_stem("Console SMP"), "Console SMP");
        assert_eq!(file_stem("a:b"), "a_b");
    }

    #[test]
    fn program_names_without_exe() {
        assert_eq!(program_name("Modrinth App.exe".as_ref()), "modrinth app");
        assert_eq!(program_name("java".as_ref()), "java");
        assert_eq!(program_name("JAVAW.EXE".as_ref()), "javaw");
    }

    #[test]
    fn quoted_ini_values_round_trip() {
        for name in [
            "Create, Above and Beyond",
            r#"Mods; "quoted" \ and more"#,
            "Plain",
        ] {
            let ini = parse_ini(&format!("[General]\nname={}\n", ini_quoted(name)));
            assert_eq!(ini["name"], name);
        }
        assert_eq!(
            ini_quoted("Create, Above and Beyond"),
            r#""Create, Above and Beyond""#
        );
    }

    #[test]
    fn a_bom_is_skipped_in_launcher_json() {
        let dir = TempDir::new("atlas-json").unwrap();
        let file = dir.join("mmc-pack.json");
        std::fs::write(&file, "\u{feff}{\"formatVersion\": 1}").unwrap();
        let v: serde_json::Value = read_json(&file).unwrap();
        assert_eq!(v["formatVersion"], 1);
        write_atomic(&file, b"{}").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"{}");
        assert_eq!(
            std::fs::read_dir(&*dir).unwrap().count(),
            1,
            "no temp file left"
        );
        let kept = dir.to_path_buf();
        drop(dir);
        assert!(!kept.exists(), "removed when dropped");
    }

    #[test]
    fn programs_are_found_by_where_they_are() {
        let me = std::env::current_exe().unwrap();
        let tail = format!(
            "{}/{}",
            me.parent().unwrap().file_name().unwrap().to_string_lossy(),
            me.file_name().unwrap().to_string_lossy()
        );
        assert!(running_at(&tail), "{tail}");
        assert!(!running_at("No Such.app/Contents/MacOS/launcher"));
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
