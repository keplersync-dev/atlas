//! Where files come from: look unknown files up by hash (docs/tauri-plan.md §9.3).
//!
//! Runs in the background after a scan, for rows still `source = 'local'` that were never
//! looked up or were last looked up over a week ago. A match fills in the Modrinth project,
//! version, title, icon, download and page links. Offline or rate-limited: nothing is
//! marked, so the next scan simply tries again.
//!
//! CurseForge comes later (T5): its API key must stay on the Atlas server, so those lookups
//! go through `POST /v1/resolve/curseforge` with the murmur2 fingerprints.

pub mod modrinth;

use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::db::content::ContentRow;
use crate::dto::Side;
use crate::util::time::{iso, now_iso};

/// Look files up again after this long (a mod may have been uploaded since).
const RETRY_AFTER_DAYS: i64 = 7;

/// What a Modrinth match tells us about one row.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub content_id: String,
    pub sha1: String,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub version_label: String,
    pub icon_url: Option<String>,
    pub download_url: String,
    pub page_url: String,
    pub side: Side,
}

/// Rows worth asking about.
pub fn needs_lookup(rows: &[ContentRow]) -> Vec<&ContentRow> {
    let cutoff = iso(chrono::Utc::now() - chrono::Duration::days(RETRY_AFTER_DAYS));
    rows.iter()
        .filter(|r| r.installed && r.source == "local")
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
                project_id: p.id.clone(),
                version_id: v.id.clone(),
                name: p.title.clone(),
                version_label: v.version_number.clone(),
                icon_url: p.icon_url.clone(),
                download_url: file.url.clone(),
                page_url: format!("https://modrinth.com/{}/{}", p.project_type, p.slug),
                side: side(&p.client_side, &p.server_side),
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

/// Save matches, and note that the other rows were asked about. Each update checks the
/// row still has the same file (sha1), in case it changed while we were online.
pub async fn save(db: &SqlitePool, asked: &[&ContentRow], matches: &[Match]) -> sqlx::Result<usize> {
    let now = now_iso();
    let mut tx = db.begin().await?;
    let mut saved = 0;
    for m in matches {
        let done = sqlx::query(
            "UPDATE instance_content SET source = 'modrinth', project_id = ?, version_id = ?, name = ?,
                version_label = ?, icon_url = ?, download_url = ?, page_url = ?, side = ?, resolved_at = ?
             WHERE id = ? AND sha1 = ?",
        )
        .bind(&m.project_id)
        .bind(&m.version_id)
        .bind(&m.name)
        .bind(&m.version_label)
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
    for r in asked.iter().filter(|r| !matches.iter().any(|m| m.content_id == r.id)) {
        sqlx::query("UPDATE instance_content SET resolved_at = ? WHERE id = ? AND sha1 = ?")
            .bind(&now)
            .bind(&r.id)
            .bind(&r.sha1)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(saved)
}

/// Look up an instance's unknown files on Modrinth and save what's found.
/// Returns how many rows changed. Network problems are logged and give Ok(0).
pub async fn resolve_instance(
    db: &SqlitePool,
    http: &reqwest::Client,
    api: &str,
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
    match lookup.await {
        Ok((versions, projects)) => {
            let matches = match_rows(&asked, &versions, &projects);
            let saved = save(db, &asked, &matches).await?;
            tracing::info!(instance = %instance_id, asked = asked.len(), found = saved, "looked files up on Modrinth");
            Ok(saved)
        }
        Err(e) => {
            tracing::debug!(error = %e, "Modrinth lookup failed; will retry on the next scan");
            Ok(0)
        }
    }
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
            ("mods/sodium.jar", "8f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e"),
            ("mods/custom.jar", "0000000000000000000000000000000000000000"),
        ])
        .await;
        let rows = db::content::list(&pool, &id).await.unwrap();
        let asked = needs_lookup(&rows);
        assert_eq!(asked.len(), 2);

        let versions: HashMap<String, modrinth::Version> = serde_json::from_str(VERSIONS).unwrap();
        let projects: HashMap<String, modrinth::Project> = serde_json::from_str::<Vec<modrinth::Project>>(PROJECTS)
            .unwrap()
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();
        let matches = match_rows(&asked, &versions, &projects);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].page_url, "https://modrinth.com/mod/sodium");
        assert_eq!(matches[0].side, Side::Client, "server unsupported = client only");

        assert_eq!(save(&pool, &asked, &matches).await.unwrap(), 1);
        let rows = db::content::list(&pool, &id).await.unwrap();
        let sodium = rows.iter().find(|r| r.path == "mods/sodium.jar").unwrap();
        assert_eq!(sodium.source, "modrinth");
        assert_eq!(sodium.name, "Sodium");
        assert_eq!(sodium.version_label.as_deref(), Some("mc1.21.1-0.6.0"));
        assert_eq!(sodium.project_id.as_deref(), Some("AANobbMI"));
        assert_eq!(sodium.side, "client");
        assert!(sodium.download_url.as_deref().unwrap().starts_with("https://cdn.modrinth.com/"));
        let custom = rows.iter().find(|r| r.path == "mods/custom.jar").unwrap();
        assert_eq!(custom.source, "local");
        assert!(custom.resolved_at.is_some(), "asked about, not found");

        // Neither is asked about again this week.
        assert!(needs_lookup(&rows).is_empty());
    }

    #[tokio::test]
    async fn a_file_that_changed_meanwhile_is_not_overwritten() {
        let (pool, id) = rows_with(&[("mods/sodium.jar", "8f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e")]).await;
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
        sqlx::query("UPDATE instance_content SET sha1 = 'new'").execute(&pool).await.unwrap();
        assert_eq!(save(&pool, &asked, &matches).await.unwrap(), 0);
        assert_eq!(db::content::list(&pool, &id).await.unwrap()[0].source, "local");
    }

    #[tokio::test]
    async fn offline_changes_nothing() {
        let (pool, id) = rows_with(&[("mods/a.jar", "8f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e")]).await;
        let http = reqwest::Client::new();
        // Nothing listens on this port: a connection error, like being offline.
        let saved = resolve_instance(&pool, &http, "http://127.0.0.1:9", &id).await.unwrap();
        assert_eq!(saved, 0);
        let rows = db::content::list(&pool, &id).await.unwrap();
        assert!(rows[0].resolved_at.is_none(), "still to be looked up next time");
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
                for found in launchers::list_instances(kind, &root).await.unwrap_or_default() {
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
                    scan::files::scan(&pool, &row.id, &found.game_dir).await.unwrap();
                    let start = std::time::Instant::now();
                    let saved = super::resolve_instance(&pool, &http, super::modrinth::API, &row.id).await.unwrap();
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
                    let unknown: Vec<&str> =
                        rows.iter().filter(|r| r.source == "local").take(2).map(|r| r.name.as_str()).collect();
                    println!(
                        "{:36} {:3} files | jar metadata {:3}/{:3} mods | Modrinth {:3} ({:>5.2?}) | e.g. {:?} | unknown e.g. {:?}",
                        found.name, rows.len(), named, mods.len(), saved, start.elapsed(), sample, unknown
                    );
                }
            }
        }
    }
}
