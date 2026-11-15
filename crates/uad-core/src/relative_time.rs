//! File ages shown to the user, such as "3 days ago".

use chrono::{DateTime, TimeDelta, offset::Utc};
use std::fs;
use std::path::PathBuf;

/// Modification time of `file`, or now when it cannot be read.
#[must_use]
pub fn last_modified_date(file: PathBuf) -> DateTime<Utc> {
    fs::metadata(file)
        .and_then(|metadata| metadata.modified())
        .map_or_else(|_| Utc::now(), DateTime::from)
}

/// How long ago `date` was, in the largest whole unit: "just now", "1 minute ago", "3 days ago".
#[must_use]
pub fn format_diff_time_from_now(date: DateTime<Utc>) -> String {
    format_elapsed(Utc::now() - date)
}

fn format_elapsed(elapsed: TimeDelta) -> String {
    let (count, unit) = if elapsed.num_days() > 0 {
        (elapsed.num_days(), "day")
    } else if elapsed.num_hours() > 0 {
        (elapsed.num_hours(), "hour")
    } else if elapsed.num_minutes() > 0 {
        (elapsed.num_minutes(), "minute")
    } else {
        return "just now".to_string();
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_time_uses_the_largest_unit_and_plurals() {
        assert_eq!(format_elapsed(TimeDelta::seconds(20)), "just now");
        assert_eq!(format_elapsed(TimeDelta::minutes(1)), "1 minute ago");
        assert_eq!(format_elapsed(TimeDelta::minutes(59)), "59 minutes ago");
        assert_eq!(format_elapsed(TimeDelta::hours(2)), "2 hours ago");
        assert_eq!(format_elapsed(TimeDelta::days(1)), "1 day ago");
    }
}
