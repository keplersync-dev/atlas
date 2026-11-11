//! Make the database say "this folder is on revision N": after an update is applied,
//! when an instance is linked, or after a restore.
//!
//! For every file in the manifest, the row at that path becomes managed, with the share
//! setting it was published with. Optional items the user doesn't have get a row with
//! `installed = 0` (no file), so the Content tab can offer them. Rows that were managed
//! but aren't in this revision go back to being the user's own. The revision's choice of
//! other files to share becomes this copy's.

use std::collections::{HashMap, HashSet};

use sqlx::SqlitePool;

use super::manifest::{self, Manifest};
use crate::db::content::{self, ContentRow};
use crate::db::instances;
use crate::dto::{ShareMode, to_wire};
use crate::error::AppResult;
use crate::services::share_files;
use crate::util::ids::new_id;

pub async fn adopt(
    db: &SqlitePool,
    instance_id: &str,
    revision: u32,
    m: &Manifest,
) -> AppResult<()> {
    let rows = content::list(db, instance_id).await?;
    let by_path: HashMap<String, &ContentRow> =
        rows.iter().map(|r| (r.path.to_lowercase(), r)).collect();
    let in_manifest: HashSet<String> = m.content.iter().map(|e| e.path.to_lowercase()).collect();

    // Which other files go out comes with the version, unless this copy changed its own
    // choices since the last one and hasn't sent them yet.
    let files = match &m.files {
        Some(files) => {
            let mine = share_files::saved_rules(db, instance_id).await?;
            let before = instances::require(db, instance_id).await?.synced_manifest();
            let unsent = before.and_then(|b| b.files).is_some_and(|b| b != mine);
            (!unsent && mine != *files).then_some(files)
        }
        None => None,
    };

    let mut tx = db.begin().await?;
    if let Some(files) = files {
        share_files::save_rules(&mut *tx, instance_id, files).await?;
    }
    for e in &m.content {
        let mode = to_wire(if e.required {
            ShareMode::Required
        } else {
            ShareMode::Optional
        });
        match by_path.get(&e.path.to_lowercase()) {
            Some(r) => {
                sqlx::query(
                    "UPDATE instance_content SET is_managed = 1, share_mode = ?, published_share_mode = ?,
                        source = ?, project_id = ?, version_id = ?, name = ?, version_label = ?,
                        download_url = ?, page_url = ?, side = ?
                     WHERE id = ?",
                )
                .bind(&mode)
                .bind(&mode)
                .bind(to_wire(e.source))
                .bind(&e.project_id)
                .bind(&e.version_id)
                .bind(&e.name)
                .bind(&e.version_label)
                .bind(&e.download_url)
                .bind(&e.page_url)
                .bind(to_wire(e.side))
                .bind(&r.id)
                .execute(&mut *tx)
                .await?;
            }
            // Not downloaded: an optional item they skipped. (A missing *required* file is a
            // local change, found by comparing with the synced manifest; no row for it.)
            None if !e.required => {
                sqlx::query(
                    "INSERT INTO instance_content (id, instance_id, kind, path, enabled, size_bytes, mtime_ms,
                        sha1, sha256, source, project_id, version_id, name, version_label, download_url,
                        page_url, side, is_managed, share_mode, published_share_mode, installed)
                     VALUES (?, ?, ?, ?, 1, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 'optional', 'optional', 0)",
                )
                .bind(new_id())
                .bind(instance_id)
                .bind(to_wire(e.kind))
                .bind(&e.path)
                .bind(e.size_bytes as i64)
                .bind(&e.sha1)
                .bind(&e.sha256)
                .bind(to_wire(e.source))
                .bind(&e.project_id)
                .bind(&e.version_id)
                .bind(&e.name)
                .bind(&e.version_label)
                .bind(&e.download_url)
                .bind(&e.page_url)
                .bind(to_wire(e.side))
                .execute(&mut *tx)
                .await?;
            }
            None => {}
        }
    }
    for r in rows
        .iter()
        .filter(|r| !in_manifest.contains(&r.path.to_lowercase()))
    {
        if !r.installed {
            content::delete(&mut tx, &r.id).await?; // no file and no longer offered
        } else if r.is_managed {
            sqlx::query("UPDATE instance_content SET is_managed = 0, published_share_mode = NULL WHERE id = ?")
                .bind(&r.id)
                .execute(&mut *tx)
                .await?;
        }
    }
    let json = serde_json::to_string(m)?;
    sqlx::query(
        "UPDATE instance SET synced_revision = ?, synced_manifest = ?, synced_manifest_hash = ?, sync_report_due = 1
         WHERE id = ?",
    )
    .bind(revision as i64)
    .bind(json)
    .bind(manifest::hash(m))
    .bind(instance_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::FileRules;

    fn version(files: Option<FileRules>) -> Manifest {
        let mut m: Manifest = serde_json::from_value(serde_json::json!({
            "schemaVersion": 2,
            "name": "Pack",
            "minecraft": { "version": "1.21.1", "loader": "fabric" },
            "content": [],
        }))
        .unwrap();
        m.files = files;
        m
    }

    fn rules(include: &str) -> FileRules {
        FileRules {
            include: vec![include.into()],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn a_copy_takes_on_the_file_choices_unless_it_changed_its_own() {
        let db = crate::db::open_in_memory().await.unwrap();
        sqlx::query(
            "INSERT INTO instance (id, launcher_kind, game_dir, name, shared_instance_id, shared_role, tracked_at)
             VALUES ('i', 'folder', 'C:/i', 'Pack', 's', 'editor', '')",
        )
        .execute(&db)
        .await
        .unwrap();
        let mine = || share_files::saved_rules(&db, "i");

        adopt(&db, "i", 1, &version(Some(rules("kubejs/"))))
            .await
            .unwrap();
        assert_eq!(mine().await.unwrap(), rules("kubejs/"), "taken on");

        adopt(&db, "i", 2, &version(None)).await.unwrap();
        assert_eq!(
            mine().await.unwrap(),
            rules("kubejs/"),
            "an older version says nothing"
        );

        adopt(&db, "i", 3, &version(Some(rules("scripts/"))))
            .await
            .unwrap();
        assert_eq!(mine().await.unwrap(), rules("scripts/"), "the next choices");

        share_files::save_rules(&db, "i", &rules("options.txt"))
            .await
            .unwrap();
        adopt(&db, "i", 4, &version(Some(rules("resources/"))))
            .await
            .unwrap();
        assert_eq!(
            mine().await.unwrap(),
            rules("options.txt"),
            "unsent choices are kept"
        );
    }
}
