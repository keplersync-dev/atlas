//! The official Minecraft Launcher.
//!
//! <root>/launcher_profiles.json → profiles{id: {name, type, gameDir?, lastVersionId, lastUsed, icon}}
//! "latest-release" and "latest-snapshot" profiles have no fixed version: the launcher's
//! copy of Mojang's version list (versions/version_manifest_v2.json) says what they are now.
//! No gameDir = the root itself. Several profiles often share one game dir, but Atlas
//! tracks folders, so each game dir is one instance, named after its most recently
//! used profile. Checked against the launcher on Windows, 2026.
//!
//! New instances are a "custom" profile with its own game dir in <root>/instances/.
//! Fabric and Quilt profiles also need versions/<id>/<id>.json, which their installers
//! fetch from the loader's meta server; the launcher downloads the rest on first play.
//! Forge and NeoForge need their own installer run (`install_forge`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};

use super::util::{any_running, env_dir, first_existing, free_folder, parse_time};
use super::{FoundInstance, NewInstance, ReadError, ReadResult};
use crate::dto::Loader;

pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(appdata) = env_dir("APPDATA") {
        roots.push(appdata.join(".minecraft"));
    }
    // Microsoft Store / Xbox app version of the launcher.
    if let Some(local) = env_dir("LOCALAPPDATA") {
        roots.push(
            local.join(
                r"Packages\Microsoft.4297127D64EC6_8wekyb3d8bbwe\LocalCache\Roaming\.minecraft",
            ),
        );
    }
    roots
}

pub fn is_root(path: &Path) -> bool {
    path.join("launcher_profiles.json").is_file()
}

pub fn program() -> Option<PathBuf> {
    let x86 =
        env_dir("ProgramFiles(x86)").map(|d| d.join(r"Minecraft Launcher\MinecraftLauncher.exe"));
    first_existing(
        [
            Some(PathBuf::from(
                r"C:\XboxGames\Minecraft Launcher\Content\Minecraft.exe",
            )),
            x86,
        ]
        .into_iter()
        .flatten(),
    )
}

#[derive(Deserialize)]
struct Profiles {
    #[serde(default)]
    profiles: HashMap<String, Profile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    #[serde(default)]
    name: String,
    #[serde(rename = "type", default)]
    kind: String,
    game_dir: Option<String>,
    last_version_id: Option<String>,
    last_used: Option<String>,
    icon: Option<String>,
}

/// What "latest-release" and "latest-snapshot" point at now. The launcher refreshes its
/// copy of Mojang's version list when it starts.
#[derive(Deserialize, Default)]
struct Latest {
    release: Option<String>,
    snapshot: Option<String>,
}

impl Latest {
    fn read(root: &Path) -> Self {
        #[derive(Deserialize)]
        struct VersionList {
            latest: Latest,
        }
        std::fs::read_to_string(root.join("versions").join("version_manifest_v2.json"))
            .ok()
            .and_then(|t| {
                serde_json::from_str::<VersionList>(t.trim_start_matches('\u{feff}')).ok()
            })
            .map(|v| v.latest)
            .unwrap_or_default()
    }

    fn of(&self, version_id: &str) -> Option<String> {
        match version_id {
            "latest-release" => self.release.clone(),
            "latest-snapshot" => self.snapshot.clone(),
            _ => None,
        }
    }
}

pub fn list(root: &Path) -> ReadResult<Vec<FoundInstance>> {
    let file = root.join("launcher_profiles.json");
    if !file.is_file() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(&file)?;
    let parsed: Profiles = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;

    // One instance per game dir: keep its most recently used profile.
    let mut by_dir: HashMap<String, (PathBuf, Profile, Option<String>)> = HashMap::new();
    for profile in parsed.profiles.into_values() {
        let dir = profile
            .game_dir
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| root.to_path_buf());
        let used = profile.last_used.as_deref().and_then(parse_time);
        let key = dir.to_string_lossy().to_lowercase();
        match by_dir.get(&key) {
            Some((_, _, best)) if *best >= used => {}
            _ => {
                by_dir.insert(key, (dir, profile, used));
            }
        }
    }

    let latest = Latest::read(root);
    Ok(by_dir
        .into_values()
        .map(|(game_dir, p, used)| {
            let version_id = p.last_version_id.clone().unwrap_or_default();
            let (minecraft_version, loader) = parse_version_id(&version_id);
            let minecraft_version = minecraft_version.or_else(|| latest.of(&version_id));
            FoundInstance {
                launcher_instance_id: game_dir.to_string_lossy().into_owned(),
                name: display_name(&p),
                game_dir,
                // Custom pictures are embedded images; named ones ("Grass") are built into
                // the launcher and have no file Atlas can show.
                icon: p.icon.filter(|i| i.starts_with("data:image/")),
                minecraft_version,
                loader: loader.as_ref().map(|(l, _)| *l),
                loader_version: loader.and_then(|(_, v)| v),
                last_played_at: used,
                total_playtime_seconds: None, // the launcher doesn't record it
            }
        })
        .collect())
}

fn display_name(p: &Profile) -> String {
    if !p.name.trim().is_empty() {
        return p.name.clone();
    }
    match p.kind.as_str() {
        "latest-release" => "Latest release".into(),
        "latest-snapshot" => "Latest snapshot".into(),
        _ => p
            .last_version_id
            .clone()
            .unwrap_or_else(|| "Minecraft".into()),
    }
}

/// "fabric-loader-0.16.9-1.17.1" → (Some("1.17.1"), Some((Fabric, Some("0.16.9")))).
/// "latest-release" has no fixed version: (None, Some((Vanilla, None))).
fn parse_version_id(id: &str) -> (Option<String>, Option<(Loader, Option<String>)>) {
    let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
    if id.starts_with("latest-") {
        return (None, Some((Loader::Vanilla, None)));
    }
    for (prefix, loader) in [
        ("fabric-loader-", Loader::Fabric),
        ("quilt-loader-", Loader::Quilt),
    ] {
        if let Some(rest) = id.strip_prefix(prefix) {
            let (version, mc) = rest.split_once('-').unwrap_or((rest, ""));
            return (some(mc), Some((loader, some(version))));
        }
    }
    if let Some(version) = id.strip_prefix("neoforge-") {
        return (
            neoforge_minecraft(version),
            Some((Loader::Neoforge, some(version))),
        );
    }
    // "1.20.1-forge-47.2.0", older "1.12.2-forge1.12.2-14.23.5.2860"
    if let Some((mc, rest)) = id.split_once("-forge") {
        let version = rest.rsplit('-').next().unwrap_or("");
        return (some(mc), Some((Loader::Forge, some(version))));
    }
    if id.starts_with(|c: char| c.is_ascii_digit()) {
        // Plain versions and snapshots ("1.21.1", "24w39a") are vanilla. Anything with
        // extra parts (OptiFine etc.) keeps its Minecraft version, loader unknown.
        return match id.split_once('-') {
            None => (some(id), Some((Loader::Vanilla, None))),
            // pre-releases and release candidates are vanilla too: "1.21.2-pre3", "1.21.7-rc1"
            Some((_, tag)) if tag.starts_with("pre") || tag.starts_with("rc") => {
                (some(id), Some((Loader::Vanilla, None)))
            }
            Some((mc, _)) => (some(mc), None),
        };
    }
    (None, None)
}

/// NeoForge 21.1.x is for Minecraft 1.21.1, 20.4.x for 1.20.4, 21.0.x for 1.21.
fn neoforge_minecraft(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    if !(20..26).contains(&major) {
        return None;
    }
    Some(if minor == 0 {
        format!("1.{major}")
    } else {
        format!("1.{major}.{minor}")
    })
}

/// Is the launcher open? It keeps profiles in memory and saves them over the file, so
/// a profile added meanwhile would be lost.
pub fn running() -> bool {
    any_running(&["minecraft.exe", "minecraftlauncher.exe"])
}

/// "NeoForge", for messages.
fn loader_name(loader: Loader) -> &'static str {
    match loader {
        Loader::Vanilla => "Minecraft",
        Loader::Fabric => "Fabric",
        Loader::Quilt => "Quilt",
        Loader::Forge => "Forge",
        Loader::Neoforge => "NeoForge",
    }
}

/// A loader's version file for the launcher: versions/<id>/<id>.json.
pub struct VersionFile {
    pub id: String,
    /// None when an installer already put it in place (Forge, NeoForge).
    pub json: Option<Vec<u8>>,
}

/// The version file a Fabric or Quilt profile needs, from the loader's meta server (what
/// their installers do). None for vanilla, which the launcher knows itself.
pub async fn fetch_version(
    http: &reqwest::Client,
    new: &NewInstance,
) -> ReadResult<Option<VersionFile>> {
    let Some(loader_version) = new.loader_version()? else {
        return Ok(None);
    };
    let (mc, v) = (new.minecraft.as_str(), loader_version);
    if !safe_id(mc) || !safe_id(v) {
        return Err(ReadError::Format(format!("odd version: {mc} {v}")));
    }
    let url = match new.loader {
        Loader::Fabric => {
            format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{v}/profile/json")
        }
        Loader::Quilt => {
            format!("https://meta.quiltmc.org/v3/versions/loader/{mc}/{v}/profile/json")
        }
        other => {
            return Err(ReadError::Format(format!(
                "can't set up {other:?} in the Minecraft Launcher"
            )));
        }
    };
    let name = loader_name(new.loader);
    let failed = |e: reqwest::Error| {
        tracing::warn!(%url, error = %e, "couldn't download the loader's version file");
        ReadError::Shown(format!(
            "Couldn't download {name} for Minecraft {mc}. Check your internet connection and try again."
        ))
    };
    let res = http
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(failed)?;
    let json = res.bytes().await.map_err(failed)?.to_vec();
    let parsed: Value = serde_json::from_slice(&json)?;
    let id = parsed["id"].as_str().unwrap_or_default().to_owned();
    if !safe_id(&id) || parsed["inheritsFrom"].as_str() != Some(mc) {
        return Err(ReadError::Format(format!("{url} sent an odd version file")));
    }
    Ok(Some(VersionFile {
        id,
        json: Some(json),
    }))
}

/// Forge and NeoForge come with an installer that sets up their version and libraries.
/// Atlas runs it the way its own "Install client" does, with a Java the Minecraft Launcher
/// has downloaded (else JAVA_HOME's). Returns the version it made; its file is in place
/// already. The launcher must be closed: the installer edits launcher_profiles.json too.
pub async fn install_forge(
    http: &reqwest::Client,
    root: &Path,
    new: &NewInstance,
) -> ReadResult<VersionFile> {
    let Some(loader_version) = new.loader_version()? else {
        return Err(ReadError::Format("Forge needs a version".into()));
    };
    let installer = forge_installer(new.loader, &new.minecraft, loader_version)?;
    let name = loader_name(new.loader);
    let java = find_java(root).ok_or_else(|| {
        ReadError::Shown(format!(
            "There's no Java to run {name}'s installer with. Play any version in the Minecraft \
             Launcher once so it downloads Java, then try again."
        ))
    })?;
    let didnt_work = || ReadError::Shown(format!("{name}'s installer didn't work. Try again."));

    let dir = std::env::temp_dir().join(format!("atlas-forge-{}", crate::util::ids::new_id()));
    std::fs::create_dir_all(&dir)?;
    let result = async {
        let jar = dir.join("installer.jar");
        let failed = |e: reqwest::Error| {
            tracing::warn!(url = %installer.url, error = %e, "couldn't download the installer");
            ReadError::Shown(format!(
                "Couldn't download {name}'s installer. Check your internet connection and try again."
            ))
        };
        let bytes = http
            .get(&installer.url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(failed)?
            .bytes()
            .await
            .map_err(failed)?;
        std::fs::write(&jar, &bytes)?;

        let before = version_dirs(root);
        let (root2, dir2, flag) = (root.to_path_buf(), dir.clone(), installer.flag);
        let status = tokio::time::timeout(
            INSTALLER_TIMEOUT,
            tokio::task::spawn_blocking(move || {
                let mut run = std::process::Command::new(&java);
                run.arg("-jar")
                    .arg(&jar)
                    .arg(flag)
                    .arg(&root2)
                    .current_dir(&dir2);
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                    run.creation_flags(CREATE_NO_WINDOW);
                }
                run.output()
            }),
        )
        .await
        .map_err(|_| ReadError::Shown(format!("{name}'s installer took too long. Try again.")))?
        .map_err(|e| {
            tracing::warn!(error = %e, "the installer's thread failed");
            didnt_work()
        })?
        .map_err(|e| {
            tracing::warn!(error = %e, "couldn't run the installer");
            didnt_work()
        })?;
        if !status.status.success() {
            tracing::warn!(
                stdout = %String::from_utf8_lossy(&status.stdout),
                "the Forge installer failed"
            );
            return Err(didnt_work());
        }
        // Its usual name, or whatever new Forge version folder it made (older ones name
        // theirs differently).
        let made = Some(installer.id.clone())
            .filter(|id| {
                root.join("versions")
                    .join(id)
                    .join(format!("{id}.json"))
                    .is_file()
            })
            .or_else(|| {
                version_dirs(root)
                    .into_iter()
                    .find(|d| !before.contains(d) && d.to_lowercase().contains("forge"))
            })
            .ok_or_else(|| {
                tracing::warn!("the installer didn't add a version");
                didnt_work()
            })?;
        Ok(VersionFile {
            id: made,
            json: None,
        })
    }
    .await;
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Installers can download a few hundred MB of libraries.
const INSTALLER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

struct Installer {
    url: String,
    /// The version it makes, usually.
    id: String,
    /// Its "install the client into this folder" option.
    flag: &'static str,
}

fn forge_installer(loader: Loader, mc: &str, v: &str) -> ReadResult<Installer> {
    if !safe_id(mc) || !safe_id(v) {
        return Err(ReadError::Format(format!("odd version: {mc} {v}")));
    }
    match loader {
        Loader::Neoforge => Ok(Installer {
            url: format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar"
            ),
            id: format!("neoforge-{v}"),
            flag: "--install-client",
        }),
        Loader::Forge => Ok(Installer {
            url: format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{v}/forge-{mc}-{v}-installer.jar"
            ),
            id: format!("{mc}-forge-{v}"),
            flag: "--installClient",
        }),
        other => Err(ReadError::Format(format!("{other:?} has no installer"))),
    }
}

fn version_dirs(root: &Path) -> Vec<String> {
    std::fs::read_dir(root.join("versions"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// A Java to run an installer with: one the Minecraft Launcher downloaded for the game
/// (newest first; the Microsoft Store launcher keeps them elsewhere), else JAVA_HOME's.
fn find_java(root: &Path) -> Option<PathBuf> {
    const RUNTIMES: [&str; 4] = [
        "java-runtime-delta",
        "java-runtime-gamma",
        "java-runtime-beta",
        "java-runtime-alpha",
    ];
    let store = env_dir("LOCALAPPDATA").map(|d| {
        d.join(r"Packages\Microsoft.4297127D64EC6_8wekyb3d8bbwe\LocalCache\Local\runtime")
    });
    let runtimes = [Some(root.join("runtime")), store];
    let launcher_java = RUNTIMES.iter().flat_map(|name| {
        runtimes.iter().flatten().map(move |dir| {
            dir.join(name)
                .join("windows-x64")
                .join(name)
                .join("bin")
                .join("javaw.exe")
        })
    });
    let java_home = env_dir("JAVA_HOME").map(|d| d.join("bin").join("javaw.exe"));
    first_existing(launcher_java.chain(java_home))
}

/// Version ids end up in URLs and folder names: letters, digits and `.-_+` only.
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_+".contains(c))
}

/// Add a profile for a shared instance, with its own game dir. Returns the game dir,
/// which is the instance's launcher id.
pub fn create(root: &Path, new: &NewInstance, version: Option<VersionFile>) -> ReadResult<String> {
    let file = root.join("launcher_profiles.json");
    let text = std::fs::read_to_string(&file)?;
    let mut doc: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    let Some(top) = doc.as_object_mut() else {
        return Err(ReadError::Format(
            "launcher_profiles.json isn't an object".into(),
        ));
    };
    let Some(profiles) = top
        .entry("profiles")
        .or_insert_with(|| json!({}))
        .as_object_mut()
    else {
        return Err(ReadError::Format(
            "launcher_profiles.json has odd profiles".into(),
        ));
    };

    let version_id = match version {
        Some(v) => {
            if let Some(json) = &v.json {
                let dir = root.join("versions").join(&v.id);
                std::fs::create_dir_all(&dir)?;
                std::fs::write(dir.join(format!("{}.json", v.id)), json)?;
                // The installers leave an empty jar; the launcher fills it from the parent
                // version.
                let jar = dir.join(format!("{}.jar", v.id));
                if !jar.exists() {
                    std::fs::write(jar, b"")?;
                }
            }
            v.id
        }
        None => new.minecraft.clone(),
    };

    let instances = root.join("instances");
    let game_dir = instances.join(free_folder(&instances, &new.name));
    std::fs::create_dir_all(&game_dir)?;
    let game_dir = game_dir.to_string_lossy().into_owned();

    // Custom pictures are embedded PNGs; other formats get a built-in block.
    let icon = new
        .icon
        .as_deref()
        .filter(|b| b.starts_with(b"\x89PNG"))
        .map(|b| {
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(b)
            )
        })
        .unwrap_or_else(|| "Furnace".into());
    let now = crate::util::time::iso(chrono::Utc::now());
    profiles.insert(
        uuid::Uuid::now_v7().simple().to_string(),
        json!({
            "created": now,
            "gameDir": game_dir,
            "icon": icon,
            "lastUsed": now,
            "lastVersionId": version_id,
            "name": new.name,
            "type": "custom",
        }),
    );

    // Written next to it and swapped in, so the launcher never reads half a file.
    let tmp = root.join("launcher_profiles.json.atlas");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&doc)?)?;
    std::fs::rename(&tmp, &file)?;
    Ok(game_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/official")
    }

    #[test]
    fn one_instance_per_game_dir_named_after_its_latest_profile() {
        let root = fixture();
        assert!(is_root(&root));
        let mut list = list(&root).unwrap();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(list.len(), 2, "{list:#?}");

        // three profiles share the root; "latest-release" was used most recently
        let main = &list[1];
        assert_eq!(main.name, "Latest release");
        assert_eq!(main.game_dir, root);
        assert_eq!(main.launcher_instance_id, root.to_string_lossy());
        assert_eq!(
            main.minecraft_version.as_deref(),
            Some("1.21.4"),
            "the latest release in the launcher's version list"
        );
        assert_eq!(main.loader, Some(Loader::Vanilla));
        assert_eq!(main.icon, None, "built-in icon name");
        assert_eq!(
            main.last_played_at.as_deref(),
            Some("2025-12-31T13:02:19.217Z")
        );

        // its own game dir and a custom (embedded) picture
        let fabric = &list[0];
        assert_eq!(fabric.name, "Fabric 1.21.1");
        assert_eq!(fabric.game_dir, PathBuf::from(r"C:\Games\fabric-1.21.1"));
        assert_eq!(fabric.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(fabric.loader, Some(Loader::Fabric));
        assert_eq!(fabric.loader_version.as_deref(), Some("0.16.9"));
        assert_eq!(
            fabric.icon.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
        assert_eq!(
            fabric.last_played_at.as_deref(),
            Some("2024-11-01T12:43:39.000Z")
        );
    }

    #[test]
    fn latest_without_a_version_list() {
        let none = Latest::read(Path::new(r"C:\no\such\folder"));
        assert_eq!(none.of("latest-release"), None);
        let latest = Latest::read(&fixture());
        assert_eq!(latest.of("latest-snapshot").as_deref(), Some("25w02a"));
        assert_eq!(latest.of("1.21.1"), None, "only the latest-* ids");
    }

    #[test]
    fn version_ids() {
        let v = |id| parse_version_id(id);
        assert_eq!(
            v("1.21.1"),
            (Some("1.21.1".into()), Some((Loader::Vanilla, None)))
        );
        assert_eq!(
            v("24w39a"),
            (Some("24w39a".into()), Some((Loader::Vanilla, None)))
        );
        assert_eq!(
            v("1.21.2-pre3"),
            (Some("1.21.2-pre3".into()), Some((Loader::Vanilla, None)))
        );
        assert_eq!(
            v("quilt-loader-0.26.0-1.20.1"),
            (
                Some("1.20.1".into()),
                Some((Loader::Quilt, Some("0.26.0".into())))
            )
        );
        assert_eq!(
            v("1.20.1-forge-47.2.0"),
            (
                Some("1.20.1".into()),
                Some((Loader::Forge, Some("47.2.0".into())))
            )
        );
        assert_eq!(
            v("1.12.2-forge1.12.2-14.23.5.2860"),
            (
                Some("1.12.2".into()),
                Some((Loader::Forge, Some("14.23.5.2860".into())))
            )
        );
        assert_eq!(
            v("neoforge-21.1.77"),
            (
                Some("1.21.1".into()),
                Some((Loader::Neoforge, Some("21.1.77".into())))
            )
        );
        assert_eq!(v("neoforge-21.0.10").0, Some("1.21".into()));
        assert_eq!(v("1.20.1-OptiFine_HD_U_I6"), (Some("1.20.1".into()), None));
        assert_eq!(v("latest-snapshot"), (None, Some((Loader::Vanilla, None))));
        assert_eq!(v(""), (None, None));
    }

    #[test]
    fn created_profiles_read_back() {
        let root =
            std::env::temp_dir().join(format!("atlas-official-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::copy(
            fixture().join("launcher_profiles.json"),
            root.join("launcher_profiles.json"),
        )
        .unwrap();
        let png = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prism/icons/smp.png"),
        )
        .unwrap();
        let fabric = NewInstance {
            name: "Friday: SMP".into(),
            minecraft: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.5".into()),
            icon: Some(png.clone()),
        };
        let version = VersionFile {
            id: "fabric-loader-0.16.5-1.21.1".into(),
            json: Some(br#"{"id":"fabric-loader-0.16.5-1.21.1","inheritsFrom":"1.21.1"}"#.to_vec()),
        };
        let a = create(&root, &fabric, Some(version)).unwrap();
        let b = create(
            &root,
            &NewInstance {
                loader: Loader::Vanilla,
                loader_version: None,
                icon: None,
                ..fabric.clone()
            },
            None,
        )
        .unwrap();
        assert_eq!(
            PathBuf::from(&a),
            root.join("instances").join("Friday_ SMP")
        );
        assert_eq!(
            PathBuf::from(&b),
            root.join("instances").join("Friday_ SMP (2)")
        );
        assert!(Path::new(&a).is_dir());
        let versions = root.join("versions/fabric-loader-0.16.5-1.21.1");
        assert!(versions.join("fabric-loader-0.16.5-1.21.1.json").is_file());
        assert!(versions.join("fabric-loader-0.16.5-1.21.1.jar").is_file());

        let list = list(&root).unwrap();
        assert_eq!(list.len(), 4, "the two old game dirs and two new ones");
        let first = list.iter().find(|i| i.launcher_instance_id == a).unwrap();
        assert_eq!(first.name, "Friday: SMP");
        assert_eq!(first.game_dir, PathBuf::from(&a));
        assert_eq!(first.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(first.loader, Some(Loader::Fabric));
        assert_eq!(first.loader_version.as_deref(), Some("0.16.5"));
        let icon = first.icon.as_deref().unwrap();
        let encoded = icon.strip_prefix("data:image/png;base64,").unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap(),
            png
        );
        let second = list.iter().find(|i| i.launcher_instance_id == b).unwrap();
        assert_eq!(second.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(second.loader, Some(Loader::Vanilla));
        assert_eq!(second.icon, None, "built-in block");

        // what was there is kept
        let doc: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("launcher_profiles.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(doc["settings"]["crashAssistance"], true);
        assert_eq!(doc["profiles"].as_object().unwrap().len(), 6);
        std::fs::remove_dir_all(&root).ok();
    }

    /// Fetches real version files from Fabric's and Quilt's meta servers. Opt-in:
    /// `cargo test --lib fetches_loader_versions -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn fetches_loader_versions() {
        let http = reqwest::Client::new();
        for (loader, mc, v, id) in [
            (
                Loader::Fabric,
                "1.21.1",
                "0.16.5",
                "fabric-loader-0.16.5-1.21.1",
            ),
            (
                Loader::Quilt,
                "1.20.1",
                "0.26.0",
                "quilt-loader-0.26.0-1.20.1",
            ),
        ] {
            let new = NewInstance {
                name: "x".into(),
                minecraft: mc.into(),
                loader,
                loader_version: Some(v.into()),
                icon: None,
            };
            let file = fetch_version(&http, &new).await.unwrap().unwrap();
            assert_eq!(file.id, id);
        }
    }

    #[test]
    fn forge_and_neoforge_installers() {
        let neo = forge_installer(Loader::Neoforge, "1.21.1", "21.1.77").unwrap();
        assert_eq!(
            neo.url,
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.77/neoforge-21.1.77-installer.jar"
        );
        assert_eq!(neo.id, "neoforge-21.1.77");
        let forge = forge_installer(Loader::Forge, "1.20.1", "47.3.0").unwrap();
        assert_eq!(forge.id, "1.20.1-forge-47.3.0");
        assert_eq!(
            parse_version_id(&forge.id),
            (
                Some("1.20.1".into()),
                Some((Loader::Forge, Some("47.3.0".into())))
            ),
            "Atlas reads back what it made"
        );
        assert!(forge_installer(Loader::Forge, "1.20.1", "../x").is_err());
        assert!(forge_installer(Loader::Fabric, "1.20.1", "0.16.5").is_err());
    }

    #[test]
    fn java_from_the_launchers_runtimes_newest_first() {
        let root = std::env::temp_dir().join(format!("atlas-java-{}", crate::util::ids::new_id()));
        let java = |name: &str| {
            root.join("runtime")
                .join(name)
                .join("windows-x64")
                .join(name)
                .join("bin")
                .join("javaw.exe")
        };
        for name in ["java-runtime-gamma", "java-runtime-delta"] {
            std::fs::create_dir_all(java(name).parent().unwrap()).unwrap();
            std::fs::write(java(name), b"").unwrap();
        }
        assert_eq!(find_java(&root), Some(java("java-runtime-delta")));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn version_ids_must_be_plain() {
        assert!(safe_id("fabric-loader-0.16.5-1.21.1"));
        assert!(safe_id("0.16.5+build.1"));
        assert!(!safe_id("../evil"));
        assert!(!safe_id("a/b"));
        assert!(!safe_id(""));
    }
}
