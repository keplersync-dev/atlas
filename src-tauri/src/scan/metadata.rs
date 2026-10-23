//! What a mod says about itself, read from inside its jar.
//!
//! Gives a readable name and version before (or without) a Modrinth match, and the mod's
//! own id, which is how Atlas recognises a mod across updates: "sodium-0.5.jar" replaced
//! by "sodium-0.6.jar" is still mod id "sodium", so its share setting carries over.
//!
//! Formats, first match wins:
//!   fabric.mod.json                id, name, version, authors
//!   quilt.mod.json                 quilt_loader.{id, version, metadata.name, metadata.contributors}
//!   META-INF/neoforge.mods.toml    [[mods]] modId, displayName, version, authors
//!   META-INF/mods.toml             (Forge 1.13+) same as above
//!   mcmod.info                     (Forge 1.12 and older) [{ modid, name, version, authorList }]
//! A version of "${file.jarVersion}" means "see META-INF/MANIFEST.MF Implementation-Version".
//!
//! Icons: fabric/quilt `icon` (a path, or sizes → paths; the largest wins), mods.toml and
//! mcmod.info `logoFile`, and `pack.png` for resource and data packs.

use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModInfo {
    pub mod_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub authors: Option<String>,
    /// Path of the icon inside the archive.
    pub icon: Option<String>,
}

/// Read a jar's metadata. Anything unreadable gives an empty ModInfo (never an error):
/// the file is still tracked under its file name.
pub fn read_jar(path: &Path) -> ModInfo {
    // Only for jars too big to read into memory (see `read_jar_bytes`).
    let Ok(file) = File::open(path) else {
        return ModInfo::default();
    };
    let Ok(mut zip) = zip::ZipArchive::new(BufReader::new(file)) else {
        return ModInfo::default();
    };
    read_archive(&mut zip)
}

/// `read_jar` for a jar already read into memory (the scan reads it once to hash it).
pub fn read_jar_bytes(data: &[u8]) -> ModInfo {
    let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(data)) else {
        return ModInfo::default();
    };
    read_archive(&mut zip)
}

fn read_archive<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> ModInfo {
    let mut text = |name: &str| -> Option<String> {
        let mut entry = zip.by_name(name).ok()?;
        if entry.size() > 1_000_000 {
            return None; // metadata files are small; don't inflate something odd
        }
        let mut s = String::new();
        entry.read_to_string(&mut s).ok()?;
        Some(s)
    };

    let mut info = if let Some(t) = text("fabric.mod.json") {
        fabric(&t)
    } else if let Some(t) = text("quilt.mod.json") {
        quilt(&t)
    } else if let Some(t) =
        text("META-INF/neoforge.mods.toml").or_else(|| text("META-INF/mods.toml"))
    {
        forge_toml(&t)
    } else if let Some(t) = text("mcmod.info") {
        mcmod_info(&t)
    } else {
        ModInfo::default()
    };

    if info.version.as_deref().is_some_and(|v| v.contains("${")) {
        info.version = text("META-INF/MANIFEST.MF").and_then(|m| manifest_version(&m));
    }
    info
}

/// Copy a mod's or pack's icon out of its archive into `out` (a .png path). Returns
/// false when it has none (or it isn't a PNG/JPEG under 2 MB). Blocking.
pub fn extract_icon(archive: &Path, out: &Path) -> bool {
    // From memory when it fits: the zip reader jumps around the file, which is slow
    // from disk even buffered (about 20x for a pack's worth of jars).
    let small = std::fs::metadata(archive).is_ok_and(|m| m.len() <= super::hash::IN_MEMORY);
    if small {
        let Ok(data) = std::fs::read(archive) else {
            return false;
        };
        return match zip::ZipArchive::new(Cursor::new(data.as_slice())) {
            Ok(mut zip) => save_icon(&mut zip, out),
            Err(_) => false,
        };
    }
    let Ok(file) = File::open(archive) else {
        return false;
    };
    let Ok(mut zip) = zip::ZipArchive::new(BufReader::new(file)) else {
        return false;
    };
    save_icon(&mut zip, out)
}

fn save_icon<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, out: &Path) -> bool {
    let inner = read_archive(zip).icon.unwrap_or_else(|| "pack.png".into());
    let Ok(mut entry) = zip.by_name(inner.trim_start_matches('/')) else {
        return false;
    };
    if entry.size() > 2_000_000 {
        return false;
    }
    let mut bytes = Vec::new();
    if entry.read_to_end(&mut bytes).is_err() || !is_image(&bytes) {
        return false;
    }
    let tmp = out.with_extension("part");
    let saved = std::fs::create_dir_all(out.parent().expect("icon path has a parent"))
        .and_then(|_| std::fs::write(&tmp, &bytes))
        .and_then(|_| std::fs::rename(&tmp, out));
    saved.is_ok()
}

fn is_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

/// fabric/quilt `icon`: "assets/x/icon.png", or { "16": "...", "128": "..." }.
fn icon_field(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        return clean(Some(s));
    }
    v.as_object()?
        .iter()
        .filter_map(|(size, path)| Some((size.parse::<u32>().unwrap_or(0), path.as_str()?)))
        .max_by_key(|(size, _)| *size)
        .and_then(|(_, path)| clean(Some(path)))
}

fn clean(s: Option<&str>) -> Option<String> {
    let s = s?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn join_names<'a>(names: impl Iterator<Item = &'a str>) -> Option<String> {
    let list: Vec<&str> = names.map(str::trim).filter(|n| !n.is_empty()).collect();
    (!list.is_empty()).then(|| list.join(", "))
}

/// Some mods ship slightly broken JSON (comments, stray control characters): be lenient.
fn parse_json(text: &str) -> Option<Value> {
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .ok()
        .or_else(|| {
            let cleaned: String = text
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n")
                .chars()
                .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                .collect();
            serde_json::from_str(&cleaned).ok()
        })
}

fn fabric(text: &str) -> ModInfo {
    let Some(j) = parse_json(text) else {
        return ModInfo::default();
    };
    let authors = j["authors"].as_array().and_then(|a| {
        join_names(
            a.iter()
                .filter_map(|x| x.as_str().or_else(|| x["name"].as_str())),
        )
    });
    ModInfo {
        mod_id: clean(j["id"].as_str()),
        name: clean(j["name"].as_str()),
        version: clean(j["version"].as_str()),
        authors,
        icon: icon_field(&j["icon"]),
    }
}

fn quilt(text: &str) -> ModInfo {
    let Some(j) = parse_json(text) else {
        return ModInfo::default();
    };
    let q = &j["quilt_loader"];
    let authors = q["metadata"]["contributors"]
        .as_object()
        .and_then(|m| join_names(m.keys().map(String::as_str)));
    ModInfo {
        mod_id: clean(q["id"].as_str()),
        name: clean(q["metadata"]["name"].as_str()),
        version: clean(q["version"].as_str()),
        authors,
        icon: icon_field(&q["metadata"]["icon"]),
    }
}

fn forge_toml(text: &str) -> ModInfo {
    let Ok(t) = text.parse::<toml::Table>() else {
        return ModInfo::default();
    };
    let Some(m) = t
        .get("mods")
        .and_then(|m| m.as_array())
        .and_then(|a| a.first())
    else {
        return ModInfo::default();
    };
    let get = |k: &str| clean(m.get(k).and_then(|v| v.as_str()));
    ModInfo {
        mod_id: get("modId"),
        name: get("displayName"),
        version: get("version"),
        authors: get("authors").or_else(|| clean(t.get("authors").and_then(|v| v.as_str()))),
        icon: get("logoFile").or_else(|| clean(t.get("logoFile").and_then(|v| v.as_str()))),
    }
}

fn mcmod_info(text: &str) -> ModInfo {
    let Some(j) = parse_json(text) else {
        return ModInfo::default();
    };
    // Either a list of mods, or { "modListVersion": 2, "modList": [...] }.
    let first = j
        .as_array()
        .or_else(|| j["modList"].as_array())
        .and_then(|a| a.first())
        .cloned();
    let Some(m) = first else {
        return ModInfo::default();
    };
    ModInfo {
        mod_id: clean(m["modid"].as_str()),
        name: clean(m["name"].as_str()),
        version: clean(m["version"].as_str()),
        authors: m["authorList"]
            .as_array()
            .and_then(|a| join_names(a.iter().filter_map(Value::as_str))),
        icon: clean(m["logoFile"].as_str()).map(|p| p.trim_start_matches('/').to_string()),
    }
}

fn manifest_version(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .find_map(|l| l.strip_prefix("Implementation-Version:"))
        .and_then(|v| clean(Some(v)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::{SimpleFileOptions, ZipWriter};

    fn jar(files: &[(&str, &str)]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("atlas-jar-{}.jar", crate::util::ids::new_id()));
        let mut z = ZipWriter::new(File::create(&p).unwrap());
        for (name, body) in files {
            z.start_file(*name, SimpleFileOptions::default()).unwrap();
            z.write_all(body.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        p
    }

    fn read(files: &[(&str, &str)]) -> ModInfo {
        let p = jar(files);
        let info = read_jar(&p);
        std::fs::remove_file(p).ok();
        info
    }

    #[test]
    fn fabric_with_mixed_author_formats() {
        let info = read(&[(
            "fabric.mod.json",
            r#"{ "schemaVersion": 1, "id": "sodium", "name": "Sodium", "version": "0.6.0+mc1.21.1",
                 "authors": ["JellySquid", { "name": "IMS" }] }"#,
        )]);
        assert_eq!(
            info,
            ModInfo {
                mod_id: Some("sodium".into()),
                name: Some("Sodium".into()),
                version: Some("0.6.0+mc1.21.1".into()),
                authors: Some("JellySquid, IMS".into()),
                icon: None,
            }
        );
    }

    #[test]
    fn quilt() {
        let info = read(&[(
            "quilt.mod.json",
            r#"{ "quilt_loader": { "id": "qsl", "version": "7.0.0",
                 "metadata": { "name": "Quilt Standard Libraries", "contributors": { "QuiltMC": "Owner" } } } }"#,
        )]);
        assert_eq!(info.mod_id.as_deref(), Some("qsl"));
        assert_eq!(info.name.as_deref(), Some("Quilt Standard Libraries"));
        assert_eq!(info.authors.as_deref(), Some("QuiltMC"));
    }

    #[test]
    fn neoforge_with_version_from_the_manifest() {
        let info = read(&[
            (
                "META-INF/neoforge.mods.toml",
                "modLoader=\"javafml\"\nloaderVersion=\"[4,)\"\n[[mods]]\nmodId=\"jei\"\nversion=\"${file.jarVersion}\"\ndisplayName=\"Just Enough Items\"\nauthors=\"mezz\"\n",
            ),
            (
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\r\nImplementation-Version: 19.21.0.247\r\n",
            ),
        ]);
        assert_eq!(info.mod_id.as_deref(), Some("jei"));
        assert_eq!(info.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(info.version.as_deref(), Some("19.21.0.247"));
        assert_eq!(info.authors.as_deref(), Some("mezz"));
    }

    #[test]
    fn old_forge_mcmod_info_in_both_shapes() {
        let list = read(&[(
            "mcmod.info",
            r#"[{ "modid": "jei", "name": "Just Enough Items", "version": "4.16.1", "authorList": ["mezz"] }]"#,
        )]);
        assert_eq!(list.mod_id.as_deref(), Some("jei"));
        assert_eq!(list.version.as_deref(), Some("4.16.1"));
        let v2 = read(&[(
            "mcmod.info",
            r#"{ "modListVersion": 2, "modList": [{ "modid": "ic2", "name": "IndustrialCraft 2" }] }"#,
        )]);
        assert_eq!(v2.name.as_deref(), Some("IndustrialCraft 2"));
    }

    #[test]
    fn broken_or_missing_metadata_gives_nothing() {
        assert_eq!(read(&[("readme.txt", "hi")]), ModInfo::default());
        assert_eq!(
            read(&[("fabric.mod.json", "{ not json")]),
            ModInfo::default()
        );
        let not_a_zip =
            std::env::temp_dir().join(format!("atlas-notzip-{}", crate::util::ids::new_id()));
        std::fs::write(&not_a_zip, b"plain bytes").unwrap();
        assert_eq!(read_jar(&not_a_zip), ModInfo::default());
        std::fs::remove_file(not_a_zip).ok();
    }

    fn jar_bytes(files: &[(&str, &[u8])]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("atlas-icon-{}.jar", crate::util::ids::new_id()));
        let mut z = ZipWriter::new(File::create(&p).unwrap());
        for (name, body) in files {
            z.start_file(*name, SimpleFileOptions::default()).unwrap();
            z.write_all(body).unwrap();
        }
        z.finish().unwrap();
        p
    }

    fn png() -> Vec<u8> {
        let mut b = vec![0x89];
        b.extend_from_slice(b"PNG\r\n\x1a\nrest of a png");
        b
    }

    #[test]
    fn icons_from_every_format() {
        let out =
            std::env::temp_dir().join(format!("atlas-icon-out-{}", crate::util::ids::new_id()));
        type Case<'a> = (&'a str, Vec<(&'a str, Vec<u8>)>);
        let cases: Vec<Case> = vec![
            (
                "fabric sizes",
                vec![
                    (
                        "fabric.mod.json",
                        br#"{"id":"a","icon":{"16":"small.png","128":"assets/a/big.png"}}"#
                            .to_vec(),
                    ),
                    ("assets/a/big.png", png()),
                ],
            ),
            (
                "forge logoFile",
                vec![
                    (
                        "META-INF/mods.toml",
                        b"[[mods]]\nmodId=\"b\"\nlogoFile=\"logo.png\"\n".to_vec(),
                    ),
                    ("logo.png", png()),
                ],
            ),
            (
                "mcmod.info",
                vec![
                    (
                        "mcmod.info",
                        br#"[{"modid":"c","logoFile":"/assets/c/logo.png"}]"#.to_vec(),
                    ),
                    ("assets/c/logo.png", png()),
                ],
            ),
            (
                "resource pack",
                vec![("pack.mcmeta", b"{}".to_vec()), ("pack.png", png())],
            ),
        ];
        for (what, files) in cases {
            let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
            let jar = jar_bytes(&refs);
            let target = out.join(format!("{}.png", what.replace(' ', "-")));
            assert!(extract_icon(&jar, &target), "{what}");
            assert_eq!(std::fs::read(&target).unwrap(), png(), "{what}");
            std::fs::remove_file(jar).ok();
        }
        // Declared but missing, or not an image: no icon.
        let jar = jar_bytes(&[("fabric.mod.json", br#"{"id":"d","icon":"nope.png"}"#)]);
        assert!(!extract_icon(&jar, &out.join("d.png")));
        let jar2 = jar_bytes(&[("pack.png", b"not an image")]);
        assert!(!extract_icon(&jar2, &out.join("e.png")));
        for j in [jar, jar2] {
            std::fs::remove_file(j).ok();
        }
        std::fs::remove_dir_all(out).ok();
    }

    #[test]
    fn lenient_json_with_comments() {
        let info = read(&[(
            "fabric.mod.json",
            "{\n  // a comment\n  \"id\": \"x\",\n  \"name\": \"X\"\n}",
        )]);
        assert_eq!(info.mod_id.as_deref(), Some("x"));
    }
}
