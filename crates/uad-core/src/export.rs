//! Plain-text and CSV exports of package lists, written into a folder the user chose.

use crate::sync::{CorePackage, User};
use crate::uad_lists::PackageState;
use chrono::DateTime;
use csv::Writer;
use std::fs;
use std::path::{Path, PathBuf};

pub const EXPORT_FILE_NAME: &str = "selection_export.txt";

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

/// Export selected package names, one per line, into `dir`.
/// Returns the full path of the written file.
pub fn export_selection(dir: &Path, package_names: &[String]) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let path = dir.join(EXPORT_FILE_NAME);
    fs::write(&path, package_names.join("\n"))
        .map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(path)
}

/// Export uninstalled packages in a csv file inside `dir`.
/// Exported information will contain package name and description.
/// Returns the full path of the written file.
pub fn export_packages(
    dir: &Path,
    user: User,
    phone_packages: &[Vec<CorePackage>],
) -> Result<PathBuf, String> {
    let user_packages = phone_packages
        .get(user.index)
        .ok_or_else(|| format!("No package list loaded for {user}"))?;
    fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let backup_file = dir.join(generate_backup_name(chrono::Local::now()));

    let file = fs::File::create(&backup_file)
        .map_err(|err| format!("{}: {err}", backup_file.display()))?;
    let mut wtr = Writer::from_writer(file);

    wtr.write_record(["Package Name", "Description"])
        .map_err(|err| err.to_string())?;

    for package in user_packages
        .iter()
        .filter(|p| p.state == PackageState::Uninstalled)
    {
        wtr.write_record([
            neutralise_spreadsheet_formula(&package.name),
            neutralise_spreadsheet_formula(&package.description.replace('\n', " ")),
        ])
        .map_err(|err| err.to_string())?;
    }

    wtr.flush().map_err(|err| err.to_string())?;

    Ok(backup_file)
}

/// Prefixes a cell that a spreadsheet would run as a formula with `'`,
/// following the OWASP guidance on CSV injection (CWE-1236).
/// Package descriptions come from a remote list, so they are untrusted.
fn neutralise_spreadsheet_formula(cell: &str) -> String {
    if cell.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{cell}")
    } else {
        cell.to_string()
    }
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

    #[test]
    fn csv_cells_cannot_start_a_formula() {
        for payload in [
            "=HYPERLINK(\"http://x\")",
            "+1",
            "-2+3",
            "@SUM(A1)",
            "\tcmd",
        ] {
            assert!(neutralise_spreadsheet_formula(payload).starts_with('\''));
        }
        assert_eq!(
            neutralise_spreadsheet_formula("Samsung bloat"),
            "Samsung bloat"
        );
    }

    #[test]
    fn export_rejects_an_unknown_user_instead_of_panicking() {
        let user = User {
            id: 10,
            index: 3,
            protected: false,
        };
        assert!(export_packages(Path::new("unused"), user, &[vec![]]).is_err());
    }
}
