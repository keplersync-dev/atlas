use crate::error::{AppError, AppResult};

/// A new id: UUIDv7 as a string. Sorts by creation time, safe to create offline.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// An id as a path segment. Ids are UUIDs; anything else is refused with `refused` before
/// it can change the path.
pub fn segment(id: &str, refused: impl FnOnce() -> AppError) -> AppResult<&str> {
    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        Ok(id)
    } else {
        Err(refused())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_uuids_that_sort_by_time() {
        let a = super::new_id();
        let b = super::new_id();
        assert_eq!(a.len(), 36);
        assert!(a < b, "later ids sort after earlier ones");
    }

    #[test]
    fn only_ids_become_path_segments() {
        let refused = || crate::error::AppError::not_found("gone");
        let id = super::new_id();
        assert_eq!(super::segment(&id, refused).unwrap(), id);
        for bad in ["", "..", "a/b", "x?y", "%2e%2e"] {
            assert!(super::segment(bad, refused).is_err(), "{bad}");
        }
    }
}
