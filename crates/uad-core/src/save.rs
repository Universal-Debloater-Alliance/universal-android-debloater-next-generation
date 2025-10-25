use crate::config::{Config, DeviceSettings};
use crate::sync::{CorePackage, Phone, User, apply_pkg_state_commands};
use crate::utils::DisplayablePath;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

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

/// Backup all `Uninstalled` and `Disabled` packages
pub fn backup_phone(
    users: Vec<User>,
    device_id: String,
    phone_packages: &[Vec<CorePackage>],
) -> Result<bool, String> {
    let mut backup = PhoneBackup {
        device_id: device_id.clone(),
        ..PhoneBackup::default()
    };

    for u in users {
        let mut user_backup = UserBackup {
            id: u.id,
            ..UserBackup::default()
        };

        for p in phone_packages[u.index].iter().cloned() {
            user_backup.packages.push(p);
        }
        backup.users.push(user_backup);
    }

    match serde_json::to_string_pretty(&backup) {
        Ok(json) => {
            let backup_dir: PathBuf = Config::load_configuration_file().general.backup_folder;
            let backup_path = &*backup_dir.join(device_id);

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
    match fs::read_to_string(backup.path) {
        Ok(data) => serde_json::from_str::<PhoneBackup>(&data)
            .expect("Unable to parse backup file")
            .users
            .into_iter()
            .map(|u| User {
                id: u.id,
                index: 0,
                protected: false,
            })
            .collect(),
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
            let phone_backup: PhoneBackup =
                serde_json::from_str(&data).expect("Unable to parse backup file");

            let mut commands = vec![];
            let mut skipped_packages = 0;
            for u in phone_backup.users {
                let i_user = match selected_device.user_list.iter().find(|x| x.id == u.id) {
                    Some(i) => i.index,
                    None => return Err(format!("user {} doesn't exist", u.id)),
                };

                for (i, backup_package) in u.packages.iter().enumerate() {
                    let package: CorePackage = if let Some(p) = packages[i_user]
                        .iter()
                        .find(|x| x.name == backup_package.name)
                    {
                        p.clone()
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
