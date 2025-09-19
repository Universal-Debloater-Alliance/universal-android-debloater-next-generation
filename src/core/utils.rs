#![warn(clippy::unwrap_used)]

use crate::core::{
    adb::{ACommand as AdbCommand, PmListPacksFlag},
    sync::User,
    theme::Theme,
    uad_lists::{PackageHashMap, PackageState, Removal, UadList},
};
use crate::gui::widgets::package_row::PackageRow;
use chrono::{DateTime, offset::Utc};
use csv::Writer;
use std::{
    collections::HashSet,
    fmt, fs,
    path::{Path, PathBuf},
};

/// Canonical shortened name of the application
pub const NAME: &str = "UAD-ng";
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

pub fn fetch_packages(
    uad_lists: &PackageHashMap,
    device_serial: &str,
    user_id: Option<u16>,
) -> Vec<PackageRow> {
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
    let mut uad_list;
    let mut state;
    let mut removal;
    let mut user_package: Vec<PackageRow> = Vec::new();

    for pack_name in all_sys_packs {
        let p_name = &pack_name;
        state = PackageState::Uninstalled;
        description = "[No description]: CONTRIBUTION WELCOMED";
        uad_list = UadList::Unlisted;
        removal = Removal::Unlisted;

        if let Some(package) = uad_lists.get(p_name) {
            if !package.description.is_empty() {
                description = &package.description;
            }
            uad_list = package.list;
            removal = package.removal;
        }

        if enabled_sys_packs.contains(p_name) {
            state = PackageState::Enabled;
        } else if disabled_sys_packs.contains(p_name) {
            state = PackageState::Disabled;
        }

        let package_row =
            PackageRow::new(p_name, state, description, uad_list, removal, false, false);
        user_package.push(package_row);
    }
    user_package.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    user_package
}

pub fn string_to_theme(theme: &str) -> Theme {
    match theme {
        "Dark" => Theme::Dark,
        "Light" => Theme::Light,
        "Lupin" => Theme::Lupin,
        // Auto uses `Display`, so it doesn't have a canonical repr
        t if t.starts_with("Auto") => Theme::Auto,
        _ => Theme::default(),
    }
}

pub fn setup_uad_dir(dir: &Path) -> PathBuf {
    let dir = dir.join("uad");
    if let Err(e) = fs::create_dir_all(&dir) {
        error!("Can't create directory: {dir:?}");
        panic!("{e}");
    }
    dir
}

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
                // Use lossy decoding to avoid panic on non-UTF8 output
                let stderr = String::from_utf8_lossy(&o.stderr).trim_end().to_string();
                error!("Can't open the following URL: {stderr}");
            }
        }
        Err(e) => error!("Failed to run command to open the file explorer: {e}"),
    }
}


#[rustfmt::skip]
pub fn last_modified_date(file: PathBuf) -> DateTime<Utc> {
    fs::metadata(file).map_or_else(|_| Utc::now(), |metadata| match metadata.modified() {
        Ok(time) => time.into(),
        Err(_) => Utc::now(),
    })
}

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

/// Export selected packages.
/// File will be saved in same directory where UAD-ng is located.
pub async fn export_selection(packages: Vec<PackageRow>) -> Result<bool, String> {
    let selected = packages
        .iter()
        .filter(|p| p.selected)
        .map(|p| p.name.clone())
        .collect::<Vec<String>>()
        .join("\n");

    match fs::write(EXPORT_FILE_NAME, selected) {
        Ok(()) => Ok(true),
        Err(err) => Err(err.to_string()),
    }
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
            |p| match p.to_os_string().into_string() {
                Ok(stem) => stem,
                Err(e) => {
                    error!("[PATH ENCODING]: {e:?}");
                    "[PATH ENCODING ERROR]".to_string()
                }
            },
        );

        write!(f, "{stem}")
    }
}

/// Can be used to choose any folder.
pub async fn open_folder() -> Result<PathBuf, Error> {
    let picked_folder = rfd::AsyncFileDialog::new()
        .pick_folder()
        .await
        .ok_or(Error::DialogClosed)?;

    Ok(picked_folder.path().to_owned())
}

/// Export uninstalled packages in a csv file.
/// Exported information will contain package name and description.
pub async fn export_packages(
    user: User,
    phone_packages: Vec<Vec<PackageRow>>,
) -> Result<bool, String> {
    let backup_file = generate_backup_name(chrono::Local::now());

    let file = fs::File::create(backup_file).map_err(|err| err.to_string())?;
    let mut wtr = Writer::from_writer(file);

    wtr.write_record(["Package Name", "Description"])
        .map_err(|err| err.to_string())?;

    let uninstalled_packages: Vec<&PackageRow> = phone_packages[user.index]
        .iter()
        .filter(|p| p.state == PackageState::Uninstalled)
        .collect();

    for package in uninstalled_packages {
        wtr.write_record([&package.name, &package.description.replace('\n', " ")])
            .map_err(|err| err.to_string())?;
    }

    wtr.flush().map_err(|err| err.to_string())?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn backup_name() {
        assert_eq!(
            generate_backup_name(chrono::Utc.timestamp_millis_opt(0).unwrap()),
            "uninstalled_packages_19700101.csv".to_string()
        );
    }
}
