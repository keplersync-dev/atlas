//! Moving file bytes. Uploads go straight to storage with the presigned URL the server
//! hands out; downloads follow the server's redirect to a presigned URL. The server only
//! checks permissions and hashes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use atlas_protocol::api::{
    BlobCheck, BlobCheckResult, CompleteUploads, FileLinks, FileLinksRequest, MAX_FILE_LINKS,
    MAX_UPLOAD_BATCH, NewUpload, UploadBatch, UploadTickets, UploadsDone,
};
use futures_util::TryStreamExt as _;
use reqwest::{Method, StatusCode};

use super::{Cloud, server_error};
use crate::error::{AppError, AppResult, ErrorCode};

/// Uploads go out in pieces this big, so progress moves within a big file.
const UPLOAD_CHUNK: usize = 256 * 1024;
/// How many files upload at the same time.
const PARALLEL_UPLOADS: usize = 8;
/// Most bytes in one batch of uploads: its tickets expire 10 minutes after they're handed
/// out, and the server reads every file back in the one request that completes them.
const BATCH_BYTES: u64 = 64 * 1024 * 1024;
/// Most files in one "which are stored?" question.
const MAX_CHECK: usize = 5000;

/// A file to upload. `name` is shown in errors.
pub struct UploadFile {
    pub path: PathBuf,
    pub sha256: String,
    pub size: u64,
    pub name: String,
    /// The uploader confirmed they may share it (the share dialog's checkbox).
    pub rights_confirmed: bool,
}

/// The files in batches of at most `MAX_UPLOAD_BATCH` files and `BATCH_BYTES` (or one
/// bigger file alone).
fn batches(files: &[UploadFile]) -> Vec<&[UploadFile]> {
    let mut out = Vec::new();
    let (mut start, mut bytes) = (0, 0);
    for (i, f) in files.iter().enumerate() {
        let full = i - start == MAX_UPLOAD_BATCH || bytes + f.size > BATCH_BYTES;
        if i > start && full {
            out.push(&files[start..i]);
            (start, bytes) = (i, 0);
        }
        bytes += f.size;
    }
    if start < files.len() {
        out.push(&files[start..]);
    }
    out
}

fn mismatch() -> AppError {
    AppError::unknown("Atlas and its server don't match. Update Atlas, or try again later.")
}
/// How many times a file link waits out "slow down" before giving up.
const RATE_LIMIT_WAITS: u32 = 3;

impl Cloud {
    /// Which of these files the server already stores.
    pub async fn stored(&self, sha256: Vec<String>) -> AppResult<Vec<String>> {
        let mut existing = Vec::new();
        for chunk in sha256.chunks(MAX_CHECK) {
            let found: BlobCheckResult = self
                .call(
                    Method::POST,
                    "/blobs/check",
                    Some(&BlobCheck {
                        sha256: chunk.to_vec(),
                    }),
                )
                .await?;
            existing.extend(found.existing);
        }
        Ok(existing)
    }

    /// Upload one file unless the server has it already. See `upload_all`.
    pub async fn upload(&self, file: UploadFile) -> AppResult<()> {
        self.upload_all(&[file], |_| {}).await
    }

    /// Upload the files the server doesn't have yet. They go in batches: one request for
    /// their tickets and one to complete them, however many small files are in it.
    /// `on_bytes` hears how many more bytes went out, as they go.
    pub async fn upload_all(
        &self,
        files: &[UploadFile],
        on_bytes: impl Fn(u64) + Clone + Send + Sync + 'static,
    ) -> AppResult<()> {
        for batch in batches(files) {
            let request = UploadBatch {
                files: batch
                    .iter()
                    .map(|f| NewUpload {
                        sha256: f.sha256.clone(),
                        size_bytes: f.size,
                        file_name: f.name.clone(),
                        rights_confirmed: f.rights_confirmed,
                    })
                    .collect(),
            };
            let UploadTickets { tickets } = self
                .call(Method::POST, "/uploads/batch", Some(&request))
                .await?;
            if tickets.len() != batch.len() {
                return Err(mismatch());
            }
            let mut started = Vec::new();
            for (file, ticket) in batch.iter().zip(tickets) {
                if ticket.already_exists {
                    continue;
                }
                let (Some(id), Some(url)) = (ticket.upload_id, ticket.url) else {
                    return Err(mismatch());
                };
                started.push((file, id, url));
            }
            if started.is_empty() {
                continue;
            }
            futures_util::stream::iter(started.iter().map(Ok::<_, AppError>))
                .try_for_each_concurrent(PARALLEL_UPLOADS, |(file, _, url)| {
                    let on_bytes = on_bytes.clone();
                    async move { self.put(file, url, on_bytes).await }
                })
                .await?;
            let done = CompleteUploads {
                upload_ids: started.into_iter().map(|(_, id, _)| id).collect(),
            };
            let _: UploadsDone = self
                .call(Method::POST, "/uploads/complete", Some(&done))
                .await?;
        }
        Ok(())
    }

    /// Send a file's bytes to storage, with a ticket's presigned URL.
    async fn put(
        &self,
        file: &UploadFile,
        url: &str,
        on_bytes: impl Fn(u64) + Send + Sync + 'static,
    ) -> AppResult<()> {
        let name = &file.name;
        let bytes = tokio::fs::read(&file.path).await.map_err(|e| {
            tracing::warn!(file = %file.path.display(), error = %e, "couldn't read a file to upload");
            AppError::conflict(format!("Couldn't read {name}"))
        })?;
        // Always say the size: without it an empty file goes out with none, and storage
        // refuses it (411 Length Required).
        let length = bytes.len();
        let chunks: Vec<Vec<u8>> = bytes.chunks(UPLOAD_CHUNK).map(<[u8]>::to_vec).collect();
        let body = reqwest::Body::wrap_stream(futures_util::stream::iter(chunks.into_iter().map(
            move |chunk| {
                on_bytes(chunk.len() as u64);
                Ok::<_, std::io::Error>(chunk)
            },
        )));
        let put = self
            .blob_http
            .put(url)
            .header(reqwest::header::CONTENT_LENGTH, length)
            .body(body)
            .send()
            .await;
        match put {
            Ok(res) if res.status().is_success() => Ok(()),
            Ok(res) => {
                tracing::warn!(status = %res.status(), "storage refused an upload");
                Err(AppError::unknown(format!("Uploading {name} didn't work")))
            }
            Err(e) => {
                // The URL is presigned: keep it out of the log.
                tracing::warn!(error = %e.without_url(), "upload failed");
                Err(AppError::offline())
            }
        }
    }

    /// A short-lived link to download one uploaded file of a shared instance.
    pub async fn file_url(&self, shared_instance_id: &str, sha256: &str) -> AppResult<String> {
        let path = format!(
            "/shared-instances/{}/files/{sha256}",
            crate::services::shared::segment(shared_instance_id)?
        );
        let url = self.url(&path)?;
        let (mut force_refresh, mut waits) = (false, 0);
        loop {
            let token = self.access_token(force_refresh).await?;
            let res = self
                .blob_http
                .get(url.clone())
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| {
                    tracing::warn!(error = %e, "couldn't reach the Atlas server");
                    AppError::offline()
                })?;
            if res.status().is_redirection()
                && let Some(location) = res
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|v| v.to_str().ok())
            {
                return Ok(location.to_owned());
            }
            let status = res.status();
            // A big pack asks for many links: when the server says slow down, wait as long
            // as it asks rather than fail the whole install.
            if status == StatusCode::TOO_MANY_REQUESTS && waits < RATE_LIMIT_WAITS {
                waits += 1;
                let secs = res
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(60)
                    .min(60);
                tracing::info!(secs, "the Atlas server asked to slow down; waiting");
                tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
                continue;
            }
            let bytes = res.bytes().await.unwrap_or_default();
            let err = server_error(status, &bytes);
            if status == StatusCode::UNAUTHORIZED
                && err.code == ErrorCode::NotSignedIn
                && !force_refresh
            {
                force_refresh = true;
                continue;
            }
            return Err(err);
        }
    }

    /// The client for downloading from presigned URLs.
    pub fn file_http(&self) -> &reqwest::Client {
        &self.blob_http
    }

    /// Download links for up to `MAX_FILE_LINKS` of a shared instance's files.
    pub async fn file_links(
        &self,
        shared_instance_id: &str,
        sha256: Vec<String>,
    ) -> AppResult<FileLinks> {
        let path = format!(
            "/shared-instances/{}/file-links",
            crate::services::shared::segment(shared_instance_id)?
        );
        self.call(Method::POST, &path, Some(&FileLinksRequest { sha256 }))
            .await
    }
}

/// Links for the files an install or update downloads, asked for a batch at a time just
/// before they're needed: links expire after a few minutes, and a slow download of a big
/// pack would outlast them if they were all asked for at the start.
pub struct LinkBatches<'a> {
    cloud: &'a Cloud,
    shared_instance_id: String,
    /// Every file that may need a link, in the order they download.
    wanted: Vec<String>,
    /// sha256 → (link, use it until).
    links: tokio::sync::Mutex<HashMap<String, (String, Instant)>>,
}

impl<'a> LinkBatches<'a> {
    pub fn new(cloud: &'a Cloud, shared_instance_id: &str, wanted: Vec<String>) -> Self {
        Self {
            cloud,
            shared_instance_id: shared_instance_id.to_owned(),
            wanted,
            links: Default::default(),
        }
    }

    /// The file's link: from the batch it came in, or with the next batch, which starts at
    /// it. Not found when the file isn't in the instance or was taken down.
    pub async fn get(&self, sha256: &str) -> AppResult<String> {
        // Held while asking, so files downloading at the same time share one request.
        let mut links = self.links.lock().await;
        let now = Instant::now();
        let fresh = |sha: &str, links: &HashMap<String, (String, Instant)>| {
            links.get(sha).is_some_and(|(_, until)| now < *until)
        };
        if !fresh(sha256, &links) {
            let from = self.wanted.iter().position(|s| s == sha256).unwrap_or(0);
            let batch: Vec<String> = std::iter::once(sha256.to_owned())
                .chain(
                    self.wanted[from..]
                        .iter()
                        .filter(|s| *s != sha256 && !fresh(s, &links))
                        .cloned(),
                )
                .take(MAX_FILE_LINKS)
                .collect();
            let got = self
                .cloud
                .file_links(&self.shared_instance_id, batch)
                .await?;
            // Half their life, so a link is never used just as it runs out.
            let until = now + Duration::from_secs(got.expires_in_secs / 2);
            for (sha, url) in got.links {
                links.insert(sha, (url, until));
            }
        }
        match links.get(sha256) {
            Some((url, until)) if now < *until => Ok(url.clone()),
            _ => Err(AppError::not_found("That file isn't available")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(size: u64) -> UploadFile {
        UploadFile {
            path: PathBuf::new(),
            sha256: String::new(),
            size,
            name: String::new(),
            rights_confirmed: false,
        }
    }

    #[test]
    fn uploads_go_in_batches() {
        let configs: Vec<UploadFile> = (0..450).map(|_| file(1024)).collect();
        let sizes: Vec<usize> = batches(&configs).iter().map(|b| b.len()).collect();
        assert_eq!(sizes, [200, 200, 50]);

        // Bytes count too, and a file bigger than a batch goes alone.
        let mb = 1024 * 1024;
        let mods = [file(40 * mb), file(30 * mb), file(90 * mb), file(mb)];
        let sizes: Vec<usize> = batches(&mods).iter().map(|b| b.len()).collect();
        assert_eq!(sizes, [1, 1, 1, 1]);
        let mods = [file(30 * mb), file(30 * mb), file(5 * mb)];
        let sizes: Vec<usize> = batches(&mods).iter().map(|b| b.len()).collect();
        assert_eq!(sizes, [2, 1]);
        assert!(batches(&[]).is_empty());
    }
}
