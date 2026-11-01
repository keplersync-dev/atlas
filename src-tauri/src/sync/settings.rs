//! Whether a config file holds the same settings as another copy, ignoring formatting,
//! comments and key order. Mods rewrite their configs when the game runs, so a file
//! can differ from the modpack's byte for byte while setting nothing new.

use sha2::{Digest, Sha256};

/// A hash of the settings in a config file, or None when its kind isn't understood or
/// it doesn't parse (then only the bytes can be compared).
pub fn fingerprint(path: &str, bytes: &[u8]) -> Option<String> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    if !matches!(
        ext.as_str(),
        "json" | "jsonc" | "mcmeta" | "toml" | "properties" | "ini" | "cfg"
    ) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let canonical = match ext.as_str() {
        "json" | "jsonc" | "mcmeta" => {
            let value: serde_json::Value = serde_json::from_str(&strip_json(text)).ok()?;
            canonical_json(&value)
        }
        "toml" => {
            let value: toml::Table = toml::from_str(text).ok()?;
            canonical_json(&serde_json::to_value(value).ok()?)
        }
        "properties" | "ini" | "cfg" => key_values(text),
        _ => return None,
    };
    Some(hex::encode(Sha256::digest(format!("{ext}\n{canonical}"))))
}

/// JSONC to JSON: comments and trailing commas removed, strings left alone.
fn strip_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push(c);
                while let Some(c) = chars.next() {
                    out.push(c);
                    match c {
                        '\\' => out.extend(chars.next()),
                        '"' => break,
                        _ => {}
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => while chars.next_if(|&c| c != '\n').is_some() {},
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for c in chars.by_ref() {
                    if last == '*' && c == '/' {
                        break;
                    }
                    last = c;
                }
            }
            '}' | ']' => {
                let kept = out.trim_end().len();
                if out[..kept].ends_with(',') {
                    out.truncate(kept - 1);
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// JSON with object keys sorted, whatever map order serde_json was built with.
fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let fields: Vec<String> = keys
                .into_iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        serde_json::Value::from(k.as_str()),
                        canonical_json(&map[k])
                    )
                })
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        serde_json::Value::Array(items) => {
            let items: Vec<String> = items.iter().map(canonical_json).collect();
            format!("[{}]", items.join(","))
        }
        other => other.to_string(),
    }
}

/// `key = value` files (properties, ini, Forge's cfg): each setting with the section it
/// sits in, spacing and comments dropped, sorted.
/// shortcut: the order of values in a Forge list (`S:names <` … `>`) is ignored too;
/// compare those lines in order if a mod turns out to care.
fn key_values(text: &str) -> String {
    let mut section: Vec<String> = Vec::new();
    let mut lines = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';', '!']) || line.starts_with("//") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = vec![line.to_owned()];
        } else if let Some(name) = line.strip_suffix('{') {
            section.push(name.trim().to_owned());
        } else if line == "}" {
            section.pop();
        } else {
            // Forge's "B:enabled=true" has its type before a ':', so '=' goes first.
            let setting = match line.split_once('=').or_else(|| line.split_once(':')) {
                Some((k, v)) => format!("{}={}", k.trim(), v.trim()),
                None => line.to_owned(),
            };
            lines.push(format!("{}\t{setting}", section.join("/")));
        }
    }
    lines.sort();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(path: &str, a: &str, b: &str) -> bool {
        let fa = fingerprint(path, a.as_bytes());
        assert!(fa.is_some(), "{path} should parse: {a}");
        fa == fingerprint(path, b.as_bytes())
    }

    #[test]
    fn json_ignores_formatting_comments_and_key_order() {
        let pack = r#"{"a": 1, "b": {"c": [1, 2], "d": "x // not a comment"}}"#;
        let mine = "{\n  // the mod wrote this\n  \"b\": { \"d\": \"x // not a comment\", \"c\": [1, 2,], },\n  /* two */ \"a\": 1\n}";
        assert!(same("config/a.jsonc", pack, mine));
        assert!(!same(
            "config/a.json",
            pack,
            r#"{"a": 2, "b": {"c": [1, 2], "d": "x // not a comment"}}"#
        ));
        assert!(!same(
            "config/a.json",
            pack,
            r#"{"a": 1, "b": {"c": [2, 1], "d": "x // not a comment"}}"#
        ));
    }

    #[test]
    fn toml_ignores_formatting_comments_and_key_order() {
        let pack = "[general]\nspeed = 2\nname = \"x\"\n";
        let mine = "# Settings\n[general]\n\t# How fast\n\tname = \"x\"\n\tspeed = 2\n";
        assert!(same("config/create/server.toml", pack, mine));
        assert!(!same(
            "config/create/server.toml",
            pack,
            "[general]\nspeed = 3\nname = \"x\"\n"
        ));
    }

    #[test]
    fn key_value_files_keep_sections_apart() {
        let pack = "# Forge\ngeneral {\n    B:enabled=true\n    I:count=4\n}\n";
        let mine = "general {\n    # Count\n    I:count = 4\n\n    B:enabled=true\n}\n";
        assert!(same("config/old.cfg", pack, mine));
        assert!(!same(
            "config/old.cfg",
            pack,
            "general {\n    B:enabled=false\n    I:count=4\n}\n"
        ));
        let ini = "[a]\nx=1\n[b]\ny=2\n";
        assert!(same("config/x.ini", ini, "; c\n[b]\ny = 2\n[a]\nx = 1\n"));
        assert!(!same("config/x.ini", ini, "[a]\ny=2\n[b]\nx=1\n"));
    }

    #[test]
    fn unknown_or_broken_files_have_none() {
        assert_eq!(
            fingerprint("scripts/recipes.zs", b"recipes.remove(<x>);"),
            None
        );
        assert_eq!(fingerprint("config/a.json", b"{ not json"), None);
        assert_eq!(fingerprint("config/a.toml", b"= nope"), None);
    }
}
