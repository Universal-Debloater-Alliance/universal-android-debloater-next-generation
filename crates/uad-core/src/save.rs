use crate::config::DeviceSettings;
use crate::sync::{CorePackage, Phone, User, apply_pkg_state_commands};
use crate::utils::DisplayablePath;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

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
    backup_dir: &Path,
    users: Vec<User>,
    device_id: String,
    phone_packages: &[Vec<CorePackage>],
) -> Result<bool, String> {
    let backup_path = backup_dir.join(&device_id);
    let mut backup = PhoneBackup {
        device_id,
        ..PhoneBackup::default()
    };

    // The users could not be listed: a backup without them could not be restored.
    if users.is_empty() {
        return Err("No user was found on the device".to_string());
    }
    for u in users {
        let packages = phone_packages
            .get(u.index)
            .ok_or_else(|| format!("No package list loaded for user {}", u.id))?;
        backup.users.push(UserBackup {
            id: u.id,
            packages: packages.clone(),
        });
    }

    match serde_json::to_string_pretty(&backup) {
        Ok(json) => {
            if let Err(e) = fs::create_dir_all(&backup_path) {
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
                let user = match selected_device.user_list.iter().find(|x| x.id == u.id) {
                    Some(user) => *user,
                    None => return Err(format!("user {} doesn't exist", u.id)),
                };

                let user_packages = packages
                    .get(user.index)
                    .ok_or_else(|| format!("No package list loaded for user {}", u.id))?;
                for backup_package in &u.packages {
                    // The GUI updates the live device list using this index.
                    let Some((i, package)) = user_packages
                        .iter()
                        .enumerate()
                        .find(|(_, p)| p.name == backup_package.name)
                    else {
                        skipped_packages += 1;
                        warn!(
                            "{} not found for user {} - skipping package during restore",
                            backup_package.name, u.id
                        );
                        continue;
                    };
                    let p_commands = apply_pkg_state_commands(
                        package,
                        backup_package.state,
                        user,
                        selected_device,
                    );
                    if !p_commands.is_empty() {
                        commands.push(BackupPackage {
                            i_user: user.index,
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
    use crate::{
        config::BackupSettings,
        uad_lists::{PackageState, Removal, UadList},
    };
    use tempfile::tempdir;

    fn package(name: &str, state: PackageState) -> CorePackage {
        CorePackage {
            name: name.to_string(),
            description: String::new(),
            state,
            removal: Removal::Recommended,
            list: UadList::Misc,
        }
    }

    #[test]
    fn restore_uses_device_indexes_and_skips_missing_packages() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("backup.json");
        let owner = User::default();
        let work = User {
            id: 10,
            index: 1,
            protected: false,
        };
        let backup = PhoneBackup {
            device_id: "test-device".to_string(),
            users: vec![
                UserBackup {
                    id: owner.id,
                    packages: vec![package("com.example.owner", PackageState::Disabled)],
                },
                UserBackup {
                    id: work.id,
                    packages: vec![
                        package("com.example.missing", PackageState::Disabled),
                        package("com.example.second", PackageState::Disabled),
                        package("com.example.first", PackageState::Disabled),
                    ],
                },
            ],
        };
        fs::write(&path, serde_json::to_vec(&backup).unwrap()).unwrap();
        let phone = Phone {
            android_sdk: 35,
            user_list: vec![owner, work],
            ..Phone::default()
        };
        let settings = DeviceSettings {
            backup: BackupSettings {
                selected: Some(DisplayablePath { path }),
                ..BackupSettings::default()
            },
            ..DeviceSettings::default()
        };
        let packages = vec![
            vec![package("com.example.owner", PackageState::Enabled)],
            vec![
                package("com.example.first", PackageState::Enabled),
                package("com.example.second", PackageState::Enabled),
            ],
        ];
        let result = restore_backup(&phone, &packages, &settings).unwrap();
        let actions: Vec<_> = result
            .packages
            .iter()
            .filter(|p| !p.commands.is_empty())
            .collect();
        assert_eq!(result.skipped_count, 1);
        assert_eq!(actions.len(), 3);
        assert_eq!((actions[0].i_user, actions[0].index), (0, 0));
        assert!(actions[0].commands[0].ends_with("--user 0 com.example.owner"));
        assert_eq!((actions[1].i_user, actions[1].index), (1, 1));
        assert!(actions[1].commands[0].ends_with("--user 10 com.example.second"));
        assert_eq!((actions[2].i_user, actions[2].index), (1, 0));
        assert!(actions[2].commands[0].ends_with("--user 10 com.example.first"));
        assert!(restore_backup(&phone, &[], &settings).is_err());
    }

    #[test]
    fn backup_is_saved_in_the_device_folder() {
        let dir = tempdir().unwrap();
        let serial = "USB123";
        let users = vec![User::default()];
        let packages = vec![vec![package("com.example.app", PackageState::Enabled)]];

        assert!(backup_phone(dir.path(), users.clone(), serial.to_string(), &[]).is_err());
        assert!(backup_phone(dir.path(), vec![], serial.to_string(), &packages).is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);

        backup_phone(dir.path(), users, serial.to_string(), &packages).unwrap();
        let backups = list_available_backups(&dir.path().join(serial));
        assert_eq!(backups.len(), 1);
        let saved: PhoneBackup =
            serde_json::from_slice(&fs::read(&backups[0].path).unwrap()).unwrap();
        assert_eq!(saved.device_id, serial);
        assert_eq!(saved.users[0].packages, packages[0]);
    }

    #[test]
    fn malformed_backup_and_missing_selection_return_errors() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("backup.json");
        fs::write(&path, "not JSON").unwrap();
        let settings = DeviceSettings {
            backup: BackupSettings {
                selected: Some(DisplayablePath { path }),
                ..BackupSettings::default()
            },
            ..DeviceSettings::default()
        };
        assert!(restore_backup(&Phone::default(), &[], &settings).is_err());
        assert!(restore_backup(&Phone::default(), &[], &DeviceSettings::default()).is_err());
    }
}
