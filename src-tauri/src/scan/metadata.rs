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
//! Each also says which Minecraft versions the mod runs on (fabric/quilt `depends`, the
//! mods.toml `minecraft` dependency, mcmod.info `mcversion`), which is how a folder added
//! without a launcher gets its version (`guess_game`).
//!
//! Icons: fabric/quilt `icon` (a path, or sizes → paths; the largest wins), mods.toml and
//! mcmod.info `logoFile`, and `pack.png` for resource and data packs.

use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

use serde_json::Value;

use crate::dto::Loader;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModInfo {
    pub mod_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub authors: Option<String>,
    /// Path of the icon inside the archive.
    pub icon: Option<String>,
    /// The loader its metadata file is for.
    pub loader: Option<Loader>,
    /// The lowest Minecraft version it says it runs on: "1.20.1".
    pub minecraft: Option<String>,
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
    } else if let Some(t) = text("META-INF/neoforge.mods.toml") {
        forge_toml(&t, Loader::Neoforge)
    } else if let Some(t) = text("META-INF/mods.toml") {
        forge_toml(&t, Loader::Forge)
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
        loader: Some(Loader::Fabric),
        minecraft: lowest_of(&j["depends"]["minecraft"]),
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
    let minecraft = q["depends"]
        .as_array()
        .and_then(|deps| deps.iter().find(|d| d["id"] == "minecraft"))
        .and_then(|d| lowest_of(&d["versions"]));
    ModInfo {
        mod_id: clean(q["id"].as_str()),
        name: clean(q["metadata"]["name"].as_str()),
        version: clean(q["version"].as_str()),
        authors,
        icon: icon_field(&q["metadata"]["icon"]),
        loader: Some(Loader::Quilt),
        minecraft,
    }
}

/// `loader`: which file it was. A mods.toml that depends on NeoForge is NeoForge's (1.20.2
/// to 1.20.4 used that name).
fn forge_toml(text: &str, loader: Loader) -> ModInfo {
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
    // [[dependencies.<mod id>]] tables: { modId = "minecraft", versionRange = "[1.20.1,1.21)" }
    let deps: Vec<&toml::Table> = t
        .get("dependencies")
        .and_then(|d| d.as_table())
        .into_iter()
        .flat_map(|d| d.values())
        .filter_map(|v| v.as_array())
        .flatten()
        .filter_map(|v| v.as_table())
        .collect();
    let dep = |id: &str| {
        deps.iter()
            .find(|d| d.get("modId").and_then(|v| v.as_str()) == Some(id))
    };
    ModInfo {
        mod_id: get("modId"),
        name: get("displayName"),
        version: get("version"),
        authors: get("authors").or_else(|| clean(t.get("authors").and_then(|v| v.as_str()))),
        icon: get("logoFile").or_else(|| clean(t.get("logoFile").and_then(|v| v.as_str()))),
        loader: Some(if dep("neoforge").is_some() {
            Loader::Neoforge
        } else {
            loader
        }),
        minecraft: dep("minecraft")
            .and_then(|d| d.get("versionRange")?.as_str())
            .and_then(maven_lowest)
            .map(|v| join_version(&v)),
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
        loader: Some(Loader::Forge),
        minecraft: m["mcversion"]
            .as_str()
            .and_then(release)
            .map(|v| join_version(&v)),
    }
}

/// A folder's loader and Minecraft version, from what its mods say: the loader that loads
/// them all (Quilt loads Fabric mods; Forge with Sinytra Connector does too), and the
/// highest of their lowest Minecraft versions. Every mod in a working pack allows the
/// version it runs on, so this is never above it, and equals it once one mod names it as
/// its lowest. shortcut: a pack whose mods all say only ">=1.20" guesses 1.20; let the
/// owner pick if that turns up.
pub fn guess_game(mods: &[ModInfo]) -> Option<(String, Loader)> {
    let has = |l: Loader| mods.iter().any(|m| m.loader == Some(l));
    let loader = [
        Loader::Neoforge,
        Loader::Forge,
        Loader::Quilt,
        Loader::Fabric,
    ]
    .into_iter()
    .find(|&l| has(l))?;
    let version = mods
        .iter()
        .filter_map(|m| release(m.minecraft.as_deref()?))
        .max()?;
    Some((join_version(&version), loader))
}

/// "1.20.1-pre2" → [1, 20, 1], "1.20.x" → [1, 20], "26.1" → [26, 1]. Snapshots
/// ("24w14a") and placeholders ("${mc}") aren't releases: None.
fn release(v: &str) -> Option<Vec<u32>> {
    let head = v
        .trim()
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .next()?;
    let parts: Vec<u32> = head.split('.').map_while(|p| p.parse().ok()).collect();
    (parts.len() >= 2).then_some(parts)
}

fn join_version(v: &[u32]) -> String {
    v.iter().map(u32::to_string).collect::<Vec<_>>().join(".")
}

/// Fabric/Quilt: one range, or a list meaning any of them (so the lowest of theirs).
/// None when any of them has no lower bound ("*").
fn lowest_of(v: &Value) -> Option<String> {
    let lows: Vec<Option<Vec<u32>>> = match v {
        Value::String(s) => vec![semver_lowest(s)],
        Value::Array(a) => a
            .iter()
            .map(|x| x.as_str().and_then(semver_lowest))
            .collect(),
        _ => return None,
    };
    let lowest = lows
        .into_iter()
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .min()?;
    Some(join_version(&lowest))
}

/// ">=1.20.1 <1.21", "~1.20.1", "^1.20", "=1.20.1", "1.20.x": predicates that must all
/// hold, so the highest of their lower bounds. "<", "<=" and ">" give none.
fn semver_lowest(range: &str) -> Option<Vec<u32>> {
    range
        .split_whitespace()
        .filter_map(|p| {
            let v = p
                .strip_prefix(">=")
                .or_else(|| p.strip_prefix(['~', '^', '=']))
                .or_else(|| (!p.starts_with(['<', '>'])).then_some(p))?;
            release(v)
        })
        .max()
}

/// Maven ranges in mods.toml: "[1.20.1,1.21)", "[1.20.1]", "[1.20,)", or a bare "1.20.1".
/// With several ranges the first is the lowest. "(" excludes its bound: none.
fn maven_lowest(range: &str) -> Option<Vec<u32>> {
    let r = range.trim();
    match r.strip_prefix('[') {
        Some(rest) => release(rest.split([',', ']']).next()?),
        None if r.starts_with('(') => None,
        None => release(r),
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
                loader: Some(Loader::Fabric),
                minecraft: None,
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
    fn minecraft_versions_from_every_format() {
        let fabric = |minecraft: &str| {
            let json = format!(r#"{{"id":"a","depends":{{"minecraft":{minecraft}}}}}"#);
            read(&[("fabric.mod.json", json.as_str())]).minecraft
        };
        assert_eq!(fabric(r#""~1.20.1""#).as_deref(), Some("1.20.1"));
        assert_eq!(fabric(r#"">=1.20.1- <1.21""#).as_deref(), Some("1.20.1"));
        assert_eq!(fabric(r#""1.20.x""#).as_deref(), Some("1.20"));
        assert_eq!(fabric(r#""26.1""#).as_deref(), Some("26.1"));
        assert_eq!(
            fabric(r#"["1.20.1", "1.20"]"#).as_deref(),
            Some("1.20"),
            "a list means any of them"
        );
        for none in [
            r#""*""#,
            r#""<1.21""#,
            r#"">1.20""#,
            r#""24w14a""#,
            r#"["1.20", "*"]"#,
        ] {
            assert_eq!(fabric(none), None, "{none}");
        }

        let quilt = read(&[(
            "quilt.mod.json",
            r#"{"quilt_loader":{"id":"q","depends":["fabric-api",{"id":"minecraft","versions":">=1.20.1"}]}}"#,
        )]);
        assert_eq!(
            (quilt.loader, quilt.minecraft.as_deref()),
            (Some(Loader::Quilt), Some("1.20.1"))
        );

        let toml = |file: &str, deps: &str| {
            let text = format!("[[mods]]\nmodId=\"m\"\n{deps}");
            read(&[(file, text.as_str())])
        };
        let dep = |id: &str, range: &str| {
            format!("[[dependencies.m]]\nmodId=\"{id}\"\nversionRange=\"{range}\"\n")
        };
        let forge = toml(
            "META-INF/mods.toml",
            &(dep("forge", "[47,)") + &dep("minecraft", "[1.20.1,1.21)")),
        );
        assert_eq!(
            (forge.loader, forge.minecraft.as_deref()),
            (Some(Loader::Forge), Some("1.20.1"))
        );
        let neo_in_mods_toml = toml("META-INF/mods.toml", &dep("neoforge", "[20.4,)"));
        assert_eq!(neo_in_mods_toml.loader, Some(Loader::Neoforge));
        for (range, lowest) in [
            ("[1.21.1]", Some("1.21.1")),
            ("1.21.1", Some("1.21.1")),
            ("[1.20,)", Some("1.20")),
            ("[,1.21)", None),
            ("(1.20,1.21)", None),
        ] {
            let neo = toml("META-INF/neoforge.mods.toml", &dep("minecraft", range));
            assert_eq!(neo.loader, Some(Loader::Neoforge));
            assert_eq!(neo.minecraft.as_deref(), lowest, "{range}");
        }

        let old = read(&[("mcmod.info", r#"[{"modid":"jei","mcversion":"1.12.2"}]"#)]);
        assert_eq!(
            (old.loader, old.minecraft.as_deref()),
            (Some(Loader::Forge), Some("1.12.2"))
        );
    }

    #[test]
    fn a_folder_runs_the_highest_lowest_version_on_the_loader_that_loads_everything() {
        let m = |loader, minecraft: Option<&str>| ModInfo {
            loader: Some(loader),
            minecraft: minecraft.map(Into::into),
            ..Default::default()
        };
        let guess = |mods: &[ModInfo]| guess_game(mods);
        assert_eq!(
            guess(&[
                m(Loader::Fabric, Some("1.20")),
                m(Loader::Fabric, Some("1.20.1")),
                m(Loader::Fabric, None),
                ModInfo::default(),
            ]),
            Some(("1.20.1".into(), Loader::Fabric))
        );
        // Quilt and Forge (with Connector) load Fabric mods; NeoForge loads Forge's.
        let loader = |mods: &[ModInfo]| guess(mods).map(|g| g.1);
        let fabric = m(Loader::Fabric, Some("1.20.1"));
        assert_eq!(
            loader(&[fabric.clone(), m(Loader::Quilt, None)]),
            Some(Loader::Quilt)
        );
        assert_eq!(
            loader(&[fabric.clone(), m(Loader::Forge, None)]),
            Some(Loader::Forge)
        );
        assert_eq!(
            loader(&[m(Loader::Forge, Some("1.20.1")), m(Loader::Neoforge, None)]),
            Some(Loader::Neoforge)
        );
        assert_eq!(
            guess(&[
                m(Loader::Fabric, Some("1.21.11")),
                m(Loader::Fabric, Some("26.1")),
                m(Loader::Fabric, Some("1.9")),
            ])
            .map(|g| g.0)
            .as_deref(),
            Some("26.1"),
            "compared as numbers"
        );
        assert_eq!(guess(&[]), None);
        assert_eq!(
            guess(&[m(Loader::Fabric, None)]),
            None,
            "no version to go on"
        );
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
