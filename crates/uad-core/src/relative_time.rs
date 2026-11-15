//! File ages shown to the user as "N day(s) ago".

use chrono::{DateTime, offset::Utc};
use std::fs;
use std::path::PathBuf;

/// Modification time of `file`, or now when it cannot be read.
#[must_use]
pub fn last_modified_date(file: PathBuf) -> DateTime<Utc> {
    fs::metadata(file)
        .and_then(|metadata| metadata.modified())
        .map_or_else(|_| Utc::now(), DateTime::from)
}

#[must_use]
pub fn format_diff_time_from_now(date: DateTime<Utc>) -> String {
    let now: DateTime<Utc> = Utc::now();
    let last_update = now - date;
    if last_update.num_days() == 0 {
        if last_update.num_hours() == 0 {
            last_update.num_minutes().to_string() + " min(s) ago"
        } else {
            last_update.num_hours().to_string() + " hour(s) ago"
        }
    } else {
        last_update.num_days().to_string() + " day(s) ago"
    }
}
