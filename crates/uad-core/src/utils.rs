#![warn(clippy::unwrap_used)]

use crate::{
    adb::{ACommand as AdbCommand, PmListPacksFlag},
    sync::{CorePackage, User},
    uad_lists::{PackageHashMap, PackageState, Removal, UadList},
};
use chrono::{DateTime, offset::Utc};
use csv::Writer;
use log::error;
use std::{
    collections::HashSet,
    fmt, fs,
    path::{Path, PathBuf},
};

/// Canonical shortened name of the application
pub const NAME: &str = "UAD-ng";
/// Full name of the application
pub const FULL_NAME: &str = "Universal Android Debloater Next Generation";
pub const EXPORT_FILE_NAME: &str = "selection_export.txt";

/// Returns `true` if `c` matches the regex `\w`
#[inline]
#[must_use]
pub const fn is_w(c: u8) -> bool {
    // https://github.com/rust-lang/rust/issues/93279
    // https://github.com/rust-lang/rust/issues/83623
    (c == b'_') | c.is_ascii_alphanumeric()
}

/// Returns `true` if `s` matches the regex `^\w+$`
#[must_use]
pub const fn is_all_w_c(s: &[u8]) -> bool {
    let mut i = 0;
    while i < s.len() {
        if !is_w(s[i]) {
            return false;
        }
        i += 1;
    }
    true
}

// Takes a time-stamp parameter,
// for purity and testability.
//
// The TZ is generic, because testing requires UTC,
// while users get the local-aware version.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Timestamps should be fresh, no need to borrow"
)]
#[must_use]
pub fn generate_backup_name<T>(t: DateTime<T>) -> String
where
    T: chrono::TimeZone,
    T::Offset: std::fmt::Display,
{
    t.format("uninstalled_packages_%Y%m%d.csv").to_string()
}

#[derive(Debug, Clone, Copy)]
pub enum Error {
    DialogClosed,
}

#[must_use]
pub fn fetch_packages(
    uad_lists: &PackageHashMap,
    device_serial: &str,
    user_id: Option<u16>,
) -> Vec<CorePackage> {
    let all_sys_packs = AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_packages_sys(Some(PmListPacksFlag::IncludeUninstalled), user_id)
        .unwrap_or_default();
    let enabled_sys_packs: HashSet<String> = AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_packages_sys(Some(PmListPacksFlag::OnlyEnabled), user_id)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let disabled_sys_packs: HashSet<String> = AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_packages_sys(Some(PmListPacksFlag::OnlyDisabled), user_id)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut description;
    let mut removal;
    let mut state;
    let mut list;
    let mut user_package: Vec<CorePackage> = Vec::new();

    for pack_name in all_sys_packs {
        let p_name = &pack_name;
        description = String::from("[No description]: CONTRIBUTION WELCOMED");
        removal = Removal::Unlisted;
        state = PackageState::Uninstalled;
        list = UadList::Unlisted;

        if let Some(package) = uad_lists.get(p_name) {
            if !package.description.is_empty() {
                description = package.description.clone();
            }
            removal = package.removal;
            list = package.list;
        }

        if enabled_sys_packs.contains(p_name) {
            state = PackageState::Enabled;
        } else if disabled_sys_packs.contains(p_name) {
            state = PackageState::Disabled;
        }

        let package = CorePackage {
            name: p_name.clone(),
            description,
            removal,
            state,
            list,
        };
        user_package.push(package);
    }
    user_package.sort_by_key(|package| package.name.to_lowercase());
    user_package
}

#[must_use]
pub fn setup_uad_dir(dir: &Path) -> PathBuf {
    let dir = dir.join("uad");
    if let Err(e) = fs::create_dir_all(&dir) {
        error!("Can't create directory: {}", dir.display());
        panic!("{e}");
    }
    dir
}

/// Open a directory or file with the system's default file manager.
pub fn open_url(dir: PathBuf) {
    const OPENER: &str = match std::env::consts::OS.as_bytes() {
        b"windows" => "explorer",
        b"macos" => "open",
        // "linux"
        _ => "xdg-open",
    };
    match std::process::Command::new(OPENER).arg(dir).output() {
        Ok(o) => {
            if !o.status.success() {
                // Use lossy conversion for stderr - some systems (like Windows)
                // may output non-UTF8 characters in error messages
                let stderr = String::from_utf8_lossy(&o.stderr);
                let stderr_trimmed = stderr.trim_end();
                if stderr_trimmed.is_empty() {
                    error!("Can't open URL: command failed with no error message");
                } else {
                    error!("Can't open the following URL: {stderr_trimmed}");
                }
            }
        }
        Err(e) => error!("Failed to run command to open the file explorer: {e}"),
    }
}

#[rustfmt::skip]
#[must_use]
pub fn last_modified_date(file: PathBuf) -> DateTime<Utc> {
    fs::metadata(file).map_or_else(|_| Utc::now(), |metadata| match metadata.modified() {
        Ok(time) => time.into(),
        Err(_) => Utc::now(),
    })
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

/// Export selected package names into `dir`.
/// Returns the path of the exported file.
pub fn export_selection(dir: &Path, package_names: &[String]) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let export_file = std::path::absolute(dir)
        .map_err(|err| err.to_string())?
        .join(EXPORT_FILE_NAME);
    fs::write(&export_file, package_names.join("\n")).map_err(|err| err.to_string())?;
    Ok(export_file)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayablePath {
    pub path: PathBuf,
}

impl fmt::Display for DisplayablePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stem = self.path.file_stem().map_or_else(
            || {
                error!("[PATH STEM]: No file stem found");
                "[File steam not found]".to_string()
            },
            |p| {
                if let Ok(stem) = p.to_os_string().into_string() {
                    stem
                } else {
                    error!("[PATH ENCODING]: {}", self.path.display());
                    "[PATH ENCODING ERROR]".to_string()
                }
            },
        );

        write!(f, "{stem}")
    }
}

/// Export uninstalled packages in a csv file.
/// Exported information will contain package name and description.
pub fn export_packages(
    dir: &Path,
    user: User,
    phone_packages: &[Vec<CorePackage>],
) -> Result<PathBuf, String> {
    let user_packages = phone_packages
        .get(user.index)
        .ok_or_else(|| format!("No package list loaded for user {}", user.id))?;
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let backup_file = std::path::absolute(dir)
        .map_err(|err| err.to_string())?
        .join(generate_backup_name(chrono::Local::now()));

    let file = fs::File::create(&backup_file).map_err(|err| err.to_string())?;
    let mut wtr = Writer::from_writer(file);

    wtr.write_record(["Package Name", "Description"])
        .map_err(|err| err.to_string())?;

    for package in user_packages
        .iter()
        .filter(|p| p.state == PackageState::Uninstalled)
    {
        wtr.write_record([csv_text(&package.name), csv_text(&package.description)])
            .map_err(|err| err.to_string())?;
    }

    wtr.flush().map_err(|err| err.to_string())?;

    Ok(backup_file)
}

// CSV quoting alone does not prevent spreadsheets from evaluating formulas.
fn csv_text(value: &str) -> String {
    let text = value.replace(['\r', '\n'], " ");
    if value.starts_with(['\t', '\r', '\n']) || text.trim_start().starts_with(['=', '+', '-', '@'])
    {
        format!("'{text}")
    } else {
        text
    }
}

/// Truncate the first line to at most `max_len` Unicode characters.
#[must_use]
pub fn truncate_description(desc: &str, max_len: usize) -> String {
    let first_line = desc.split('\n').next().unwrap_or("").trim_end();

    if first_line.chars().count() <= max_len {
        first_line.to_string()
    } else if max_len < 3 {
        ".".repeat(max_len)
    } else {
        let prefix: String = first_line.chars().take(max_len - 3).collect();
        format!("{prefix}...")
    }
}

/// Check if package matches search term (checks name and description)
#[must_use]
pub fn matches_search(pkg_name: &str, search_term: &str, pkg_description: Option<&str>) -> bool {
    let search_lower = search_term.to_lowercase();

    if pkg_name.to_lowercase().contains(&search_lower) {
        return true;
    }

    if let Some(description) = pkg_description {
        if description.to_lowercase().contains(&search_lower) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn description_truncation_respects_unicode_and_short_limits() {
        for (description, limit, expected) in [
            ("é漢🙂abc", 5, "é漢..."),
            ("你好世界", 4, "你好世界"),
            ("abcdef", 0, ""),
            ("abcdef", 1, "."),
            ("abcdef", 2, ".."),
            ("abcdef", 3, "..."),
            ("abc\r\nsecond line", 10, "abc"),
            ("", 0, ""),
        ] {
            assert_eq!(truncate_description(description, limit), expected);
        }
    }

    #[test]
    fn selection_export_uses_the_requested_directory() {
        let dir = tempfile::tempdir().expect("temporary export directory");
        let export_dir = dir.path().join("backups");
        let names = ["com.example.one".to_string(), "com.example.two".to_string()];
        let path = export_selection(&export_dir, &names).expect("export selection");
        assert_eq!(
            path,
            std::path::absolute(&export_dir)
                .expect("absolute export directory")
                .join(EXPORT_FILE_NAME)
        );
        assert_eq!(
            fs::read_to_string(path).expect("read exported selection"),
            "com.example.one\ncom.example.two"
        );
    }

    #[test]
    fn csv_export_uses_the_requested_directory_and_quotes_text() {
        let dir = tempfile::tempdir().expect("temporary export directory");
        let user = User {
            id: 0,
            index: 0,
            protected: false,
        };
        let package = CorePackage {
            name: "com.example.app".into(),
            description: " =HYPERLINK(\"x\",\"y\")\r\nnext line".into(),
            removal: Removal::Recommended,
            state: PackageState::Uninstalled,
            list: UadList::Misc,
        };
        let enabled = CorePackage {
            state: PackageState::Enabled,
            ..package.clone()
        };
        let path =
            export_packages(dir.path(), user, &[vec![package, enabled]]).expect("export packages");
        assert_eq!(
            path.parent().expect("export directory"),
            std::path::absolute(dir.path()).expect("absolute export directory")
        );
        let records = csv::Reader::from_path(path)
            .expect("open exported CSV")
            .records()
            .collect::<Result<Vec<_>, _>>()
            .expect("valid CSV records");
        assert_eq!(records.len(), 1);
        assert_eq!(&records[0][0], "com.example.app");
        assert_eq!(&records[0][1], "' =HYPERLINK(\"x\",\"y\")  next line");
        for value in [
            "=1+1", "+1", "-2", "@SUM(A1)", " \t=1+1", "\tvalue", "\rvalue",
        ] {
            assert!(csv_text(value).starts_with('\''));
        }
        assert_eq!(
            csv_text("normal, \"quoted\" text"),
            "normal, \"quoted\" text"
        );
    }

    #[test]
    fn csv_export_rejects_unknown_users_before_creating_a_file() {
        let dir = tempfile::tempdir().expect("temporary export directory");
        assert!(export_packages(dir.path(), User::default(), &[]).is_err());
        assert_eq!(
            fs::read_dir(dir.path())
                .expect("read export directory")
                .count(),
            0
        );
    }

    #[test]
    fn backup_name() {
        assert_eq!(
            generate_backup_name(chrono::Utc.timestamp_millis_opt(0).unwrap()),
            "uninstalled_packages_19700101.csv".to_string()
        );
    }
}
