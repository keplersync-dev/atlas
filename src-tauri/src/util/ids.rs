/// A new id: UUIDv7 as a string. Sorts by creation time, safe to create offline.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
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
}
