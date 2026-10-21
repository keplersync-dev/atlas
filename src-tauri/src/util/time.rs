use chrono::{DateTime, SecondsFormat, Utc};

/// Now as ISO-8601 UTC with millis: "2026-10-19T16:39:00.000Z".
pub fn now_iso() -> String {
    iso(Utc::now())
}

pub fn iso(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Milliseconds since the Unix epoch (Prism's `lastLaunchTime`, file mtimes) as an ISO string.
pub fn iso_from_millis(ms: i64) -> Option<String> {
    DateTime::from_timestamp_millis(ms).map(iso)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_with_millis_and_z() {
        assert_eq!(
            iso_from_millis(1_792_368_000_000).unwrap(),
            "2026-10-19T00:00:00.000Z"
        );
        let now = now_iso();
        assert_eq!(now.len(), 24);
        assert!(now.ends_with('Z'));
    }
}
