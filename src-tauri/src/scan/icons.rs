//! Mod and pack icons, copied out of the files into Atlas's cache so the UI can show them.
//!
//!   cache/icons/<sha1>.png    the icon of the file with that sha1
//!   cache/icons/<sha1>.none   checked: that file has no icon (don't open it again)
//!   cache/icons/instance-<sha1 of the image>.png   instance pictures (cache_instance_icon)
//!
//! Rows store `icon_url = "file:<absolute path>"`; the DTO turns that into an asset-protocol
//! URL (util::asset). A Modrinth icon, when there is one, replaces it (it's usually nicer)
//! and a row that already has an icon is left alone.

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use base64::Engine;
use sha1::{Digest, Sha1};

use super::metadata::extract_icon;
use crate::db::content::{self, ContentRow};
use crate::error::{AppError, AppResult};

/// An instance picture from a launcher, made showable: local files and embedded images
/// are copied into the cache (the asset protocol only serves the cache), URLs pass
/// through. Same picture = same file name, so comparing results tells if it changed.
/// Blocking (small files).
pub fn cache_instance_icon(icons_dir: &Path, icon: &str) -> Option<String> {
    if icon.starts_with("https://") {
        return Some(icon.to_string());
    }
    let bytes = if let Some(path) = icon.strip_prefix("file:") {
        let meta = std::fs::metadata(path).ok()?;
        if meta.len() > 2_000_000 {
            return None;
        }
        std::fs::read(path).ok()?
    } else {
        // Embedded picture ("data:image/png;base64,…"); anything else isn't an image.
        let (_, encoded) = icon.strip_prefix("data:image/")?.split_once(";base64,")?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .ok()?
    };
    let is_image = bytes.starts_with(b"\x89PNG")
        || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"GIF8")
        || (bytes.len() > 12 && &bytes[8..12] == b"WEBP");
    if !is_image {
        return None;
    }
    let file = icons_dir.join(format!(
        "instance-{}.png",
        hex::encode(Sha1::digest(&bytes))
    ));
    if !file.is_file() {
        std::fs::create_dir_all(icons_dir).ok()?;
        let tmp = file.with_extension("part");
        std::fs::write(&tmp, &bytes).ok()?;
        std::fs::rename(&tmp, &file).ok()?;
    }
    Some(format!("file:{}", file.display()))
}

fn icon_file(dir: &Path, sha1: &str) -> PathBuf {
    dir.join(format!("{sha1}.png"))
}

fn none_marker(dir: &Path, sha1: &str) -> PathBuf {
    dir.join(format!("{sha1}.none"))
}

/// Give every row without an icon the one from its file, if it has one. Returns how many
/// rows got an icon. Each file is opened at most once ever (the result is cached by sha1).
pub async fn fill(
    db: &SqlitePool,
    icons_dir: &Path,
    instance_id: &str,
    game_dir: &Path,
) -> AppResult<usize> {
    let rows: Vec<ContentRow> = content::list(db, instance_id)
        .await?
        .into_iter()
        .filter(|r| r.installed && r.icon_url.is_none() && r.kind != "shaderpack")
        .collect();
    if rows.is_empty() {
        return Ok(0);
    }
    let (dir, game) = (icons_dir.to_path_buf(), game_dir.to_path_buf());
    let found: Vec<(String, String, PathBuf)> = tokio::task::spawn_blocking(move || {
        rows.iter()
            .filter_map(|r| {
                let png = icon_file(&dir, &r.sha1);
                if !png.is_file() {
                    if none_marker(&dir, &r.sha1).exists() {
                        return None;
                    }
                    let on_disk = game.join(crate::sync::plan::disk_path(r));
                    if !extract_icon(&on_disk, &png) {
                        let _ = std::fs::create_dir_all(&dir)
                            .and_then(|_| std::fs::write(none_marker(&dir, &r.sha1), b""));
                        return None;
                    }
                }
                Some((r.id.clone(), r.sha1.clone(), png))
            })
            .collect()
    })
    .await
    .map_err(|e| AppError::unknown(e.to_string()))?;

    let mut tx = db.begin().await?;
    let mut set = 0;
    for (id, sha1, png) in &found {
        // Only if the row still has that file and still has no icon.
        let done = sqlx::query("UPDATE instance_content SET icon_url = ? WHERE id = ? AND sha1 = ? AND icon_url IS NULL")
            .bind(format!("file:{}", png.display()))
            .bind(id)
            .bind(sha1)
            .execute(&mut *tx)
            .await?;
        set += done.rows_affected() as usize;
    }
    tx.commit().await?;
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake png body";

    #[test]
    fn instance_icons_are_cached_by_content() {
        let dir =
            std::env::temp_dir().join(format!("atlas-insticons-{}", crate::util::ids::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("launcher-icon.png");
        std::fs::write(&src, PNG).unwrap();
        let cache = dir.join("cache");

        let from_file = cache_instance_icon(&cache, &format!("file:{}", src.display())).unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(PNG);
        let from_data =
            cache_instance_icon(&cache, &format!("data:image/png;base64,{encoded}")).unwrap();
        assert_eq!(from_file, from_data, "same picture, same cached file");
        let cached = PathBuf::from(from_file.strip_prefix("file:").unwrap());
        assert!(cached.starts_with(&cache));
        assert_eq!(std::fs::read(&cached).unwrap(), PNG);

        let url = "https://media.forgecdn.net/avatars/thumbnails/1/2/256/256/3";
        assert_eq!(cache_instance_icon(&cache, url).as_deref(), Some(url));
        assert_eq!(cache_instance_icon(&cache, "Grass"), None);
        assert_eq!(
            cache_instance_icon(&cache, "file:C:/does/not/exist.png"),
            None
        );
        std::fs::write(&src, b"not an image").unwrap();
        assert_eq!(
            cache_instance_icon(&cache, &format!("file:{}", src.display())),
            None
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
