use crate::config::DeviceSettings;
use crate::sync::{CorePackage, Phone, User, apply_pkg_state_commands};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::{
    fs,
    path::{Path, PathBuf},
};

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

/// Folder holding the backups of one device: `backup_folder/<device_id>`.
///
/// `device_id` is an ADB serial. A wireless serial (`192.168.1.5:5555`) has a `:`,
/// which Windows forbids in file names, and a serial must never add path
/// components (CWE-22). Such characters become `_`. USB serials and every
/// serial on Linux and macOS keep their existing folder name.
#[must_use]
pub fn device_backup_dir(backup_folder: &Path, device_id: &str) -> PathBuf {
    const WINDOWS_RESERVED: [char; 7] = [':', '*', '?', '"', '<', '>', '|'];
    let folder_name: String = device_id
        .chars()
        .map(|c| {
            let is_unsafe = matches!(c, '/' | '\\' | '\0')
                || (cfg!(target_os = "windows") && WINDOWS_RESERVED.contains(&c));
            if is_unsafe { '_' } else { c }
        })
        .collect();
    match folder_name.as_str() {
        "" | "." | ".." => backup_folder.join("_"),
        name => backup_folder.join(name),
    }
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct PhoneBackup {
    pub device_id: String,
    pub users: Vec<UserBackup>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct UserBackup {
    pub id: u16,
    pub packages: Vec<CorePackage>,
}

/// Backup the state of every package of `users` into `backup_folder`.
pub fn backup_phone(
    backup_folder: &Path,
    users: Vec<User>,
    device_id: &str,
    phone_packages: &[Vec<CorePackage>],
) -> Result<bool, String> {
    let mut backup = PhoneBackup {
        device_id: device_id.to_string(),
        ..PhoneBackup::default()
    };

    for u in users {
        let mut user_backup = UserBackup {
            id: u.id,
            ..UserBackup::default()
        };

        let user_packages = phone_packages
            .get(u.index)
            .ok_or_else(|| format!("No package list loaded for {u}"))?;
        user_backup.packages.extend(user_packages.iter().cloned());
        backup.users.push(user_backup);
    }

    match serde_json::to_string_pretty(&backup) {
        Ok(json) => {
            let backup_path = &*device_backup_dir(backup_folder, device_id);

            if let Err(e) = fs::create_dir_all(backup_path) {
                error!("BACKUP: could not create backup dir: {e}");
                return Err(e.to_string());
            }

            let backup_filename =
                format!("{}.json", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));

            match fs::write(backup_path.join(backup_filename), json) {
                Ok(()) => Ok(true),
                Err(err) => Err(err.to_string()),
            }
        }
        Err(err) => Err(err.to_string()),
    }
}

pub fn list_available_backups(dir: &Path) -> Vec<DisplayablePath> {
    match fs::read_dir(dir) {
        Ok(files) => files
            .filter_map(Result::ok)
            .map(|e| DisplayablePath { path: e.path() })
            .collect::<Vec<_>>(),
        Err(_) => vec![],
    }
}

#[must_use]
pub fn list_available_backup_user(backup: DisplayablePath) -> Vec<User> {
    let path = backup.path;
    match fs::read_to_string(&path) {
        Ok(data) => match serde_json::from_str::<PhoneBackup>(&data) {
            Ok(phone_backup) => phone_backup
                .users
                .into_iter()
                .map(|u| User {
                    id: u.id,
                    index: 0,
                    protected: false,
                })
                .collect(),
            Err(e) => {
                error!(
                    "[BACKUP]: {} is not a valid backup file: {e}",
                    path.display()
                );
                vec![]
            }
        },
        Err(e) => {
            error!("[BACKUP]: Selected backup file not found: {e}");
            vec![]
        }
    }
}

#[derive(Debug)]
pub struct BackupPackage {
    pub i_user: usize,
    pub index: usize,
    pub commands: Vec<String>,
}

#[derive(Debug)]
pub struct RestoreResult {
    pub packages: Vec<BackupPackage>,
    pub skipped_count: usize,
}

pub fn restore_backup(
    selected_device: &Phone,
    packages: &[Vec<CorePackage>],
    settings: &DeviceSettings,
) -> Result<RestoreResult, String> {
    match fs::read_to_string(
        settings
            .backup
            .selected
            .as_ref()
            .ok_or("field should be Some type")?
            .path
            .clone(),
    ) {
        Ok(data) => {
            let phone_backup: PhoneBackup = serde_json::from_str(&data)
                .map_err(|e| format!("The selected backup file is not valid: {e}"))?;

            let mut commands = vec![];
            let mut skipped_packages = 0;
            for u in phone_backup.users {
                let i_user = match selected_device.user_list.iter().find(|x| x.id == u.id) {
                    Some(i) => i.index,
                    None => return Err(format!("user {} doesn't exist", u.id)),
                };

                for backup_package in &u.packages {
                    // `index` must point into the device package list, which the
                    // GUI uses to show and update the package, not into the backup.
                    let (i, package): (usize, CorePackage) = if let Some((i, p)) = packages
                        .get(i_user)
                        .ok_or_else(|| format!("No package list loaded for user {}", u.id))?
                        .iter()
                        .enumerate()
                        .find(|(_, x)| x.name == backup_package.name)
                    {
                        (i, p.clone())
                    } else {
                        skipped_packages += 1;
                        warn!(
                            "{} not found for user {} - skipping package during restore",
                            backup_package.name, u.id
                        );
                        continue;
                    };
                    let p_commands = apply_pkg_state_commands(
                        &package,
                        backup_package.state,
                        settings
                            .backup
                            .selected_user
                            .ok_or("field should be Some type")?,
                        selected_device,
                    );
                    if !p_commands.is_empty() {
                        commands.push(BackupPackage {
                            i_user,
                            index: i,
                            commands: p_commands,
                        });
                    }
                }
            }
            if skipped_packages > 0 {
                info!(
                    "Restore completed with {skipped_packages} packages skipped (not found on device)"
                );
            }
            if !commands.is_empty() {
                commands.push(BackupPackage {
                    i_user: 0,
                    index: 0,
                    commands: vec![],
                });
            }
            Ok(RestoreResult {
                packages: commands,
                skipped_count: skipped_packages,
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_backup_dir_never_leaves_the_backup_folder() {
        let root = Path::new("backups");
        for serial in ["../../etc", "..", ".", "", "a/b", "a\\b", "x\0y"] {
            let dir = device_backup_dir(root, serial);
            assert_eq!(
                dir.parent(),
                Some(root),
                "{serial:?} gave {}",
                dir.display()
            );
            assert!(!dir.ends_with(".."), "{serial:?} gave {}", dir.display());
        }
    }

    #[test]
    fn usb_serials_keep_their_folder_name() {
        assert_eq!(
            device_backup_dir(Path::new("backups"), "R58M123ABC"),
            Path::new("backups").join("R58M123ABC")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn wireless_serials_are_valid_windows_folder_names() {
        assert_eq!(
            device_backup_dir(Path::new("backups"), "192.168.1.5:5555"),
            Path::new("backups").join("192.168.1.5_5555")
        );
    }

    #[test]
    fn backup_rejects_an_unknown_user_instead_of_panicking() {
        let user = User {
            id: 10,
            index: 2,
            protected: false,
        };
        assert!(backup_phone(Path::new("unused"), vec![user], "serial", &[]).is_err());
    }
}
