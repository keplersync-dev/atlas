//! Where files come from: look unknown files up by hash.
//!
//! Runs in the background after a scan, for rows on no mod site (`local`, or `upload`
//! once published: a file uploaded earlier may be on Modrinth or CurseForge by now) that
//! were never looked up or were last looked up over a week ago. Modrinth first (by sha1); files it
//! doesn't know go to CurseForge (by fingerprint) through the Atlas server, which keeps
//! the API key. A match fills in the project, version, title, icon, download and page
//! links. Offline, signed out or rate-limited: misses aren't marked, so the next scan
//! simply tries again.

pub mod modrinth;

use std::collections::HashMap;

use atlas_protocol::api::{CurseForgeFile, CurseForgeLookup, CurseForgeLookupResult};
use reqwest::Method;
use sqlx::SqlitePool;

use crate::cloud::Cloud;
use crate::db::content::ContentRow;
use crate::dto::{ContentSource, Side};
use crate::error::ErrorCode;
use crate::util::time::{iso, now_iso};

/// Look files up again after this long (a mod may have been uploaded since).
const RETRY_AFTER_DAYS: i64 = 7;
/// The most fingerprints the server takes in one lookup.
const CURSEFORGE_BATCH: usize = 2000;

/// What a match tells us about one row.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub content_id: String,
    pub sha1: String,
    pub source: ContentSource,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub version_label: String,
    pub author: Option<String>,
    pub icon_url: Option<String>,
    /// None: a CurseForge file its author only lets people download on CurseForge.
    pub download_url: Option<String>,
    pub page_url: String,
    pub side: Side,
}

/// Rows worth asking about.
pub fn needs_lookup(rows: &[ContentRow]) -> Vec<&ContentRow> {
    let cutoff = iso(chrono::Utc::now() - chrono::Duration::days(RETRY_AFTER_DAYS));
    rows.iter()
        .filter(|r| r.installed && matches!(r.source.as_str(), "local" | "upload"))
        .filter(|r| r.resolved_at.as_deref().is_none_or(|t| t < cutoff.as_str()))
        .collect()
}

/// Pair rows with what Modrinth returned. Pure, so it's easy to test.
pub fn match_rows(
    rows: &[&ContentRow],
    versions: &HashMap<String, modrinth::Version>,
    projects: &HashMap<String, modrinth::Project>,
) -> Vec<Match> {
    rows.iter()
        .filter_map(|r| {
            let v = versions.get(&r.sha1)?;
            let p = projects.get(&v.project_id)?;
            let file = v.files.iter().find(|f| f.hashes.sha1 == r.sha1)?;
            Some(Match {
                content_id: r.id.clone(),
                sha1: r.sha1.clone(),
                source: ContentSource::Modrinth,
                project_id: p.id.clone(),
                version_id: v.id.clone(),
                name: p.title.clone(),
                version_label: v.version_number.clone(),
                author: None,
                icon_url: p.icon_url.clone(),
                download_url: Some(file.url.clone()),
                page_url: format!("https://modrinth.com/{}/{}", p.project_type, p.slug),
                side: side(&p.client_side, &p.server_side),
            })
        })
        .collect()
}

/// The row's CurseForge fingerprint, if the scan worked one out.
fn fingerprint(r: &ContentRow) -> Option<u32> {
    r.murmur2.and_then(|m| u32::try_from(m).ok())
}

/// Pair rows with what CurseForge returned. Fingerprints are only 32 bits, so a match
/// also needs the same sha1.
pub fn match_curseforge(rows: &[&ContentRow], files: &[CurseForgeFile]) -> Vec<Match> {
    rows.iter()
        .filter_map(|r| {
            let fp = fingerprint(r)?;
            let f = files
                .iter()
                .find(|f| f.fingerprint == fp && f.sha1.as_deref() == Some(r.sha1.as_str()))?;
            Some(Match {
                content_id: r.id.clone(),
                sha1: r.sha1.clone(),
                source: ContentSource::Curseforge,
                project_id: f.project_id.clone(),
                version_id: f.file_id.clone(),
                name: f.name.clone(),
                version_label: f.version_label.clone(),
                author: f.author.clone(),
                icon_url: f.icon_url.clone(),
                download_url: f.download_url.clone(),
                page_url: f.page_url.clone(),
                side: f.side,
            })
        })
        .collect()
}

/// Modrinth's client/server support → which side the file is for.
fn side(client: &str, server: &str) -> Side {
    match (client, server) {
        (_, "unsupported") => Side::Client,
        ("unsupported", _) => Side::Server,
        _ => Side::Both,
    }
}

/// Save matches, and (with `mark_misses`) note that the other rows were asked about.
/// Each update checks the row still has the same file (sha1), in case it changed while
/// we were online.
pub async fn save(
    db: &SqlitePool,
    asked: &[&ContentRow],
    matches: &[Match],
    mark_misses: bool,
) -> sqlx::Result<usize> {
    let now = now_iso();
    let mut tx = db.begin().await?;
    let mut saved = 0;
    for m in matches {
        let done = sqlx::query(
            "UPDATE instance_content SET source = ?, project_id = ?, version_id = ?, name = ?,
                version_label = ?, author = COALESCE(?, author), icon_url = COALESCE(?, icon_url),
                download_url = ?, page_url = ?, side = ?, resolved_at = ?
             WHERE id = ? AND sha1 = ?",
        )
        .bind(crate::dto::to_wire(m.source))
        .bind(&m.project_id)
        .bind(&m.version_id)
        .bind(&m.name)
        .bind(&m.version_label)
        .bind(&m.author)
        .bind(&m.icon_url)
        .bind(&m.download_url)
        .bind(&m.page_url)
        .bind(crate::dto::to_wire(m.side))
        .bind(&now)
        .bind(&m.content_id)
        .bind(&m.sha1)
        .execute(&mut *tx)
        .await?;
        saved += done.rows_affected() as usize;
    }
    let misses = asked
        .iter()
        .filter(|r| !matches.iter().any(|m| m.content_id == r.id));
    if mark_misses {
        for r in misses {
            sqlx::query("UPDATE instance_content SET resolved_at = ? WHERE id = ? AND sha1 = ?")
                .bind(&now)
                .bind(&r.id)
                .bind(&r.sha1)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(saved)
}

/// Ask CurseForge, through the Atlas server, about rows Modrinth didn't know.
/// None: it couldn't be asked right now (signed out, offline), so try again later.
async fn ask_curseforge(cloud: &Cloud, rows: &[&ContentRow]) -> Option<Vec<Match>> {
    let fingerprints: Vec<u32> = rows.iter().filter_map(|r| fingerprint(r)).collect();
    if fingerprints.is_empty() {
        return Some(Vec::new());
    }
    if !cloud.is_signed_in() {
        return None;
    }
    let mut files = Vec::new();
    for batch in fingerprints.chunks(CURSEFORGE_BATCH) {
        let body = CurseForgeLookup {
            fingerprints: batch.to_vec(),
        };
        let found = cloud
            .call::<CurseForgeLookupResult>(Method::POST, "/curseforge/fingerprints", Some(&body))
            .await;
        match found {
            Ok(found) => files.extend(found.files),
            // This server has no CurseForge API key: asking again soon won't change that.
            Err(e) if e.code == ErrorCode::Conflict => {
                tracing::debug!(error = %e, "the Atlas server can't look files up on CurseForge");
                return Some(Vec::new());
            }
            Err(e) => {
                tracing::debug!(error = %e, "CurseForge lookup failed; will retry on the next scan");
                return None;
            }
        }
    }
    Some(match_curseforge(rows, &files))
}

/// Look up an instance's unknown files and save what's found. `cloud`: also ask
/// CurseForge through the Atlas server. Returns how many rows changed. Network problems
/// are logged and give Ok(0).
pub async fn resolve_instance(
    db: &SqlitePool,
    http: &reqwest::Client,
    api: &str,
    cloud: Option<&Cloud>,
    instance_id: &str,
) -> sqlx::Result<usize> {
    let rows = crate::db::content::list(db, instance_id).await?;
    let asked = needs_lookup(&rows);
    if asked.is_empty() {
        return Ok(0);
    }
    let hashes: Vec<String> = asked.iter().map(|r| r.sha1.clone()).collect();
    let lookup = async {
        let versions = modrinth::versions_by_sha1(http, api, &hashes).await?;
        let mut ids: Vec<String> = versions.values().map(|v| v.project_id.clone()).collect();
        ids.sort();
        ids.dedup();
        let projects = modrinth::projects(http, api, &ids).await?;
        Ok::<_, reqwest::Error>((versions, projects))
    };
    let mut matches = match lookup.await {
        Ok((versions, projects)) => match_rows(&asked, &versions, &projects),
        Err(e) => {
            tracing::debug!(error = %e, "Modrinth lookup failed; will retry on the next scan");
            return Ok(0);
        }
    };
    let on_modrinth = matches.len();
    let rest: Vec<&ContentRow> = asked
        .iter()
        .copied()
        .filter(|r| !matches.iter().any(|m| m.content_id == r.id))
        .collect();
    let curseforge = match cloud {
        Some(cloud) => ask_curseforge(cloud, &rest).await,
        None => Some(Vec::new()),
    };
    let asked_everywhere = curseforge.is_some();
    matches.extend(curseforge.unwrap_or_default());
    let saved = save(db, &asked, &matches, asked_everywhere).await?;
    tracing::info!(
        instance = %instance_id,
        asked = asked.len(),
        modrinth = on_modrinth,
        curseforge = matches.len() - on_modrinth,
        "looked files up"
    );
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    // Shapes copied from real Modrinth responses, trimmed to the fields Atlas reads.
    const VERSIONS: &str = r#"{
      "8f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e": {
        "id": "RncWhTxD", "project_id": "AANobbMI", "version_number": "mc1.21.1-0.6.0", "name": "Sodium 0.6.0",
        "files": [
          { "hashes": { "sha1": "8f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e", "sha512": "x" },
            "url": "https://cdn.modrinth.com/data/AANobbMI/versions/RncWhTxD/sodium-fabric-0.6.0.jar",
            "filename": "sodium-fabric-0.6.0.jar", "primary": true, "size": 6 }
        ]
      }
    }"#;
    const PROJECTS: &str = r#"[
      { "id": "AANobbMI", "slug": "sodium", "title": "Sodium", "project_type": "mod",
        "icon_url": "https://cdn.modrinth.com/data/AANobbMI/icon.png",
        "client_side": "required", "server_side": "unsupported", "team": "4reLOAKe" }
    ]"#;

    async fn rows_with(files: &[(&str, &str)]) -> (SqlitePool, String) {
        let pool = db::open_in_memory().await.unwrap();
        let inst = db::instances::insert(
            &pool,
            db::instances::NewInstance {
                launcher_id: None,
                kind: crate::dto::LauncherKind::Folder,
                launcher_instance_id: None,
                game_dir: "C:\\x".into(),
                meta: db::instances::Metadata {
                    name: "x".into(),
                    icon: None,
                    minecraft_version: None,
                    loader: None,
                    loader_version: None,
                    last_played_at: None,
                    total_playtime_seconds: None,
                },
            },
        )
        .await
        .unwrap();
        for (path, sha1) in files {
            sqlx::query(
                "INSERT INTO instance_content (id, instance_id, kind, path, size_bytes, sha1, sha256, name)
                 VALUES (?, ?, 'mod', ?, 6, ?, 'sha256', ?)",
            )
            .bind(crate::util::ids::new_id())
            .bind(&inst.id)
            .bind(path)
            .bind(sha1)
            .bind(path)
            .execute(&pool)
            .await
            .unwrap();
        }
        (pool, inst.id)
    }

    #[tokio::test]
    async fn matches_fill_in_the_row_and_misses_are_noted() {
        let (pool, id) = rows_with(&[
            (
                "mods/sodium.jar",
                "8f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e",
            ),
            (
                "mods/custom.jar",
                "0000000000000000000000000000000000000000",
            ),
        ])
        .await;
        let rows = db::content::list(&pool, &id).await.unwrap();
        let asked = needs_lookup(&rows);
        assert_eq!(asked.len(), 2);

        let versions: HashMap<String, modrinth::Version> = serde_json::from_str(VERSIONS).unwrap();
        let projects: HashMap<String, modrinth::Project> =
            serde_json::from_str::<Vec<modrinth::Project>>(PROJECTS)
                .unwrap()
                .into_iter()
                .map(|p| (p.id.clone(), p))
                .collect();
        let matches = match_rows(&asked, &versions, &projects);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].page_url, "https://modrinth.com/mod/sodium");
        assert_eq!(
            matches[0].side,
            Side::Client,
            "server unsupported = client only"
        );

        assert_eq!(save(&pool, &asked, &matches, true).await.unwrap(), 1);
        let rows = db::content::list(&pool, &id).await.unwrap();
        let sodium = rows.iter().find(|r| r.path == "mods/sodium.jar").unwrap();
        assert_eq!(sodium.source, "modrinth");
        assert_eq!(sodium.name, "Sodium");
        assert_eq!(sodium.version_label.as_deref(), Some("mc1.21.1-0.6.0"));
        assert_eq!(sodium.project_id.as_deref(), Some("AANobbMI"));
        assert_eq!(sodium.side, "client");
        assert!(
            sodium
                .download_url
                .as_deref()
                .unwrap()
                .starts_with("https://cdn.modrinth.com/")
        );
        let custom = rows.iter().find(|r| r.path == "mods/custom.jar").unwrap();
        assert_eq!(custom.source, "local");
        assert!(custom.resolved_at.is_some(), "asked about, not found");

        // Neither is asked about again this week.
        assert!(needs_lookup(&rows).is_empty());
    }

    #[tokio::test]
    async fn a_file_that_changed_meanwhile_is_not_overwritten() {
        let (pool, id) = rows_with(&[(
            "mods/sodium.jar",
            "8f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e",
        )])
        .await;
        let rows = db::content::list(&pool, &id).await.unwrap();
        let asked = needs_lookup(&rows);
        let versions: HashMap<String, modrinth::Version> = serde_json::from_str(VERSIONS).unwrap();
        let projects = serde_json::from_str::<Vec<modrinth::Project>>(PROJECTS)
            .unwrap()
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();
        let matches = match_rows(&asked, &versions, &projects);
        // The scanner saw a new file at that path before the answer came back.
        sqlx::query("UPDATE instance_content SET sha1 = 'new'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(save(&pool, &asked, &matches, true).await.unwrap(), 0);
        assert_eq!(
            db::content::list(&pool, &id).await.unwrap()[0].source,
            "local"
        );
    }

    #[tokio::test]
    async fn offline_changes_nothing() {
        let (pool, id) =
            rows_with(&[("mods/a.jar", "8f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e")]).await;
        let http = reqwest::Client::new();
        // Nothing listens on this port: a connection error, like being offline.
        let saved = resolve_instance(&pool, &http, "http://127.0.0.1:9", None, &id)
            .await
            .unwrap();
        assert_eq!(saved, 0);
        let rows = db::content::list(&pool, &id).await.unwrap();
        assert!(
            rows[0].resolved_at.is_none(),
            "still to be looked up next time"
        );
    }

    #[tokio::test]
    async fn curseforge_matches_need_the_same_sha1() {
        let (pool, id) = rows_with(&[
            (
                "mods/closed.jar",
                "1111111111111111111111111111111111111111",
            ),
            ("mods/clash.jar", "2222222222222222222222222222222222222222"),
            (
                "mods/custom.jar",
                "3333333333333333333333333333333333333333",
            ),
        ])
        .await;
        for (path, fp) in [
            ("mods/closed.jar", 111),
            ("mods/clash.jar", 222),
            ("mods/custom.jar", 333),
        ] {
            sqlx::query("UPDATE instance_content SET murmur2 = ? WHERE path = ?")
                .bind(fp)
                .bind(path)
                .execute(&pool)
                .await
                .unwrap();
        }
        // Shared earlier as an upload, before Atlas looked on CurseForge.
        sqlx::query("UPDATE instance_content SET source = 'upload' WHERE path = 'mods/closed.jar'")
            .execute(&pool)
            .await
            .unwrap();
        let file = |fingerprint: u32, sha1: &str| CurseForgeFile {
            fingerprint,
            project_id: "238222".into(),
            file_id: "5101".into(),
            name: "Closed Mod".into(),
            version_label: "closed-1.0.jar".into(),
            sha1: Some(sha1.into()),
            author: Some("someone".into()),
            icon_url: None,
            download_url: None,
            page_url: "https://www.curseforge.com/minecraft/mc-mods/closed/files/5101".into(),
            side: Side::Both,
        };
        let files = [
            file(111, "1111111111111111111111111111111111111111"),
            // Same fingerprint, different file: not a match.
            file(222, "ffffffffffffffffffffffffffffffffffffffff"),
        ];
        let rows = db::content::list(&pool, &id).await.unwrap();
        let asked = needs_lookup(&rows);
        let matches = match_curseforge(&asked, &files);
        assert_eq!(matches.len(), 1);

        // CurseForge couldn't be asked about the rest: they stay unchecked.
        assert_eq!(save(&pool, &asked, &matches, false).await.unwrap(), 1);
        let rows = db::content::list(&pool, &id).await.unwrap();
        let closed = rows.iter().find(|r| r.path == "mods/closed.jar").unwrap();
        assert_eq!(closed.source, "curseforge");
        assert_eq!(closed.project_id.as_deref(), Some("238222"));
        assert_eq!(closed.version_id.as_deref(), Some("5101"));
        assert_eq!(closed.author.as_deref(), Some("someone"));
        assert_eq!(
            closed.download_url, None,
            "downloaded from its page instead"
        );
        assert!(closed.page_url.as_deref().unwrap().ends_with("/files/5101"));
        let clash = rows.iter().find(|r| r.path == "mods/clash.jar").unwrap();
        assert_eq!(clash.source, "local");
        assert!(clash.resolved_at.is_none(), "asked again next scan");
        assert_eq!(needs_lookup(&rows).len(), 2);
    }

    #[test]
    fn sides() {
        assert_eq!(side("required", "required"), Side::Both);
        assert_eq!(side("optional", "unsupported"), Side::Client);
        assert_eq!(side("unsupported", "required"), Side::Server);
        assert_eq!(side("unknown", "unknown"), Side::Both);
    }
}

#[cfg(test)]
mod real {
    /// Scans this PC's real instances into a throwaway in-memory database, then looks the
    /// files up on Modrinth (sends their sha1 hashes, as the app does). Writes nothing on disk.
    /// `cargo test --release --lib real_lookup -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn real_lookup() {
        use crate::{db, launchers, scan};
        let pool = db::open_in_memory().await.unwrap();
        let http = reqwest::Client::builder()
            .user_agent(concat!("keplerhaloxx/atlas/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap();
        for kind in launchers::KINDS {
            for root in launchers::detect(kind) {
                for found in launchers::list_instances(kind, &root)
                    .await
                    .unwrap_or_default()
                {
                    let row = db::instances::insert(
                        &pool,
                        db::instances::NewInstance {
                            launcher_id: None,
                            kind,
                            launcher_instance_id: None,
                            game_dir: found.game_dir.to_string_lossy().into_owned(),
                            meta: db::instances::Metadata {
                                name: found.name.clone(),
                                icon: None,
                                minecraft_version: None,
                                loader: None,
                                loader_version: None,
                                last_played_at: None,
                                total_playtime_seconds: None,
                            },
                        },
                    )
                    .await
                    .unwrap();
                    scan::files::scan(&pool, &row.id, &found.game_dir)
                        .await
                        .unwrap();
                    let start = std::time::Instant::now();
                    let saved =
                        super::resolve_instance(&pool, &http, super::modrinth::API, None, &row.id)
                            .await
                            .unwrap();
                    let rows = db::content::list(&pool, &row.id).await.unwrap();
                    if rows.is_empty() {
                        continue;
                    }
                    let mods: Vec<_> = rows.iter().filter(|r| r.kind == "mod").collect();
                    let named = mods.iter().filter(|r| r.mod_id.is_some()).count();
                    let sample: Vec<String> = rows
                        .iter()
                        .filter(|r| r.source == "modrinth")
                        .take(2)
                        .map(|r| format!("{} {}", r.name, r.version_label.as_deref().unwrap_or("")))
                        .collect();
                    let unknown: Vec<&str> = rows
                        .iter()
                        .filter(|r| r.source == "local")
                        .take(2)
                        .map(|r| r.name.as_str())
                        .collect();
                    println!(
                        "{:36} {:3} files | jar metadata {:3}/{:3} mods | Modrinth {:3} ({:>5.2?}) | e.g. {:?} | unknown e.g. {:?}",
                        found.name,
                        rows.len(),
                        named,
                        mods.len(),
                        saved,
                        start.elapsed(),
                        sample,
                        unknown
                    );
                }
            }
        }
    }
}
