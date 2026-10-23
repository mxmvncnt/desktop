//! "3 minutes ago"-style relative time strings.

use std::time::{SystemTime, UNIX_EPOCH};

/// Largest unit used before the count just keeps growing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Coarsest {
    /// "45 days ago" (the Fetch caption).
    Day,
    /// "2 months ago", "3 years ago" (commit/branch lists).
    Year,
}

/// Relative time from unix seconds.
pub fn relative_time(unix: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_ago(now - unix, Coarsest::Year)
}

/// Relative time for a `SystemTime` (future times count as "just now").
pub fn relative_time_since(time: SystemTime, coarsest: Coarsest) -> String {
    let secs = time.elapsed().map(|d| d.as_secs() as i64).unwrap_or(0);
    format_ago(secs, coarsest)
}

fn format_ago(secs: i64, coarsest: Coarsest) -> String {
    let d = secs.max(0);
    let (n, unit) = match d {
        0..=59 => return "just now".into(),
        60..=3599 => (d / 60, "minute"),
        3600..=86_399 => (d / 3600, "hour"),
        _ if coarsest == Coarsest::Day => (d / 86_400, "day"),
        86_400..=2_591_999 => (d / 86_400, "day"),
        2_592_000..=31_535_999 => (d / 2_592_000, "month"),
        _ => (d / 31_536_000, "year"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format_ago(-5, Coarsest::Year), "just now");
        assert_eq!(format_ago(60, Coarsest::Year), "1 minute ago");
        assert_eq!(format_ago(7200, Coarsest::Year), "2 hours ago");
        assert_eq!(format_ago(86_400 * 45, Coarsest::Day), "45 days ago");
        assert_eq!(format_ago(86_400 * 45, Coarsest::Year), "1 month ago");
        assert_eq!(format_ago(31_536_000 * 3, Coarsest::Year), "3 years ago");
    }
}
