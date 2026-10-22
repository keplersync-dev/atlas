//! Moving file bytes. Uploads go straight to storage with the presigned URL the server
//! hands out; downloads follow the server's redirect to a presigned URL. The server only
//! checks permissions and hashes.

use std::path::Path;

use atlas_protocol::api::{BlobCheck, BlobCheckResult, NewUpload, UploadDone, UploadTicket};
use reqwest::{Method, StatusCode};

use super::{Cloud, server_error};
use crate::error::{AppError, AppResult, ErrorCode};

impl Cloud {
    /// Which of these files the server already stores.
    pub async fn stored(&self, sha256: Vec<String>) -> AppResult<Vec<String>> {
        if sha256.is_empty() {
            return Ok(Vec::new());
        }
        let found: BlobCheckResult = self
            .call(Method::POST, "/blobs/check", Some(&BlobCheck { sha256 }))
            .await?;
        Ok(found.existing)
    }

    /// Upload one file unless the server has it already. `name` is shown in errors.
    pub async fn upload(&self, file: &Path, sha256: &str, size: u64, name: &str) -> AppResult<()> {
        let request = NewUpload {
            sha256: sha256.to_owned(),
            size_bytes: size,
            file_name: name.to_owned(),
        };
        let ticket: UploadTicket = self.call(Method::POST, "/uploads", Some(&request)).await?;
        if ticket.already_exists {
            return Ok(());
        }
        let (Some(id), Some(url)) = (ticket.upload_id, ticket.url) else {
            return Err(AppError::unknown(
                "The Atlas server sent something unexpected",
            ));
        };
        let bytes = tokio::fs::read(file).await.map_err(|e| {
            tracing::warn!(file = %file.display(), error = %e, "couldn't read a file to upload");
            AppError::conflict(format!("Couldn't read {name}"))
        })?;
        let put = self.blob_http.put(&url).body(bytes).send().await;
        match put {
            Ok(res) if res.status().is_success() => {}
            Ok(res) => {
                tracing::warn!(status = %res.status(), "storage refused an upload");
                return Err(AppError::unknown(format!("Uploading {name} didn't work")));
            }
            Err(e) => {
                tracing::warn!(error = %e, "upload failed");
                return Err(AppError::offline());
            }
        }
        let path = format!(
            "/uploads/{}/complete",
            crate::services::shared::segment(&id)?
        );
        let _: UploadDone = self.call(Method::POST, &path, None::<&()>).await?;
        Ok(())
    }

    /// A short-lived link to download one uploaded file of a shared instance.
    pub async fn file_url(&self, shared_instance_id: &str, sha256: &str) -> AppResult<String> {
        let path = format!(
            "/shared-instances/{}/files/{sha256}",
            crate::services::shared::segment(shared_instance_id)?
        );
        let url = self.url(&path)?;
        for force_refresh in [false, true] {
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
            let bytes = res.bytes().await.unwrap_or_default();
            let err = server_error(status, &bytes);
            if status == StatusCode::UNAUTHORIZED
                && err.code == ErrorCode::NotSignedIn
                && !force_refresh
            {
                continue;
            }
            return Err(err);
        }
        Err(AppError::not_signed_in())
    }

    /// The client for downloading from presigned URLs.
    pub fn file_http(&self) -> &reqwest::Client {
        &self.blob_http
    }
}
