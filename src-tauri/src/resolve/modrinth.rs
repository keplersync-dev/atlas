//! Modrinth API calls used to identify files by hash. No key needed; Modrinth asks for
//! an identifying User-Agent (set on the client in AppState). Limit: 300 requests/min.
//! Docs: https://docs.modrinth.com/api/

use std::collections::HashMap;

use serde::Deserialize;

pub const API: &str = "https://api.modrinth.com/v2";

/// How many hashes / ids per request (keeps bodies and URLs a sane size).
const BATCH: usize = 200;

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    pub files: Vec<VersionFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub hashes: FileHashes,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileHashes {
    pub sha1: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub project_type: String,
    /// "required" | "optional" | "unsupported" | "unknown"
    pub client_side: String,
    pub server_side: String,
}

/// The Modrinth version each sha1 belongs to. Hashes Modrinth doesn't know are absent.
pub async fn versions_by_sha1(
    http: &reqwest::Client,
    api: &str,
    hashes: &[String],
) -> reqwest::Result<HashMap<String, Version>> {
    let mut out = HashMap::new();
    for chunk in hashes.chunks(BATCH) {
        let found: HashMap<String, Version> = http
            .post(format!("{api}/version_files"))
            .json(&serde_json::json!({ "hashes": chunk, "algorithm": "sha1" }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        out.extend(found);
    }
    Ok(out)
}

pub async fn projects(
    http: &reqwest::Client,
    api: &str,
    ids: &[String],
) -> reqwest::Result<HashMap<String, Project>> {
    let mut out = HashMap::new();
    for chunk in ids.chunks(BATCH) {
        let ids = serde_json::to_string(chunk).expect("strings serialize");
        let found: Vec<Project> = http
            .get(format!("{api}/projects"))
            .query(&[("ids", ids)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        out.extend(found.into_iter().map(|p| (p.id.clone(), p)));
    }
    Ok(out)
}
