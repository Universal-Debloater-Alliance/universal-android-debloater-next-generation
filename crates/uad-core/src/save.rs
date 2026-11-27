use crate::config::DeviceSettings;
use crate::sync::{CorePackage, Phone, User, commands_for_user};
use crate::utils::{DisplayablePath, try_fetch_packages};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
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

/// Folder holding the backups of one device.
#[must_use]
pub fn device_backup_dir(backup_dir: &Path, device_id: &str) -> PathBuf {
    // Wireless serials such as `192.168.1.5:5555` are not valid file names on Windows.
    let invalid: &[char] = if cfg!(windows) {
        &['<', '>', ':', '"', '/', '\\', '|', '?', '*']
    } else {
        &['/']
    };
    backup_dir.join(device_id.replace(invalid, "_"))
}

/// Backup all `Uninstalled` and `Disabled` packages
pub fn backup_phone(
    backup_dir: &Path,
    users: Vec<User>,
    device_id: String,
    phone_packages: &[Vec<CorePackage>],
) -> Result<PathBuf, String> {
    let backup_path = device_backup_dir(backup_dir, &device_id);
    let mut backup = PhoneBackup {
        device_id,
        ..PhoneBackup::default()
    };

    // The users could not be listed: a backup without them could not be restored.
    if users.is_empty() {
        return Err("No user was found on the device".to_string());
    }
    for u in users {
        backup.users.push(UserBackup {
            id: u.id,
            packages: u.packages(phone_packages)?.to_vec(),
        });
    }

    let json = serde_json::to_string_pretty(&backup).map_err(|error| error.to_string())?;
    fs::create_dir_all(&backup_path).map_err(|error| error.to_string())?;
    let filename = format!("{}.json", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));
    let path = backup_path.join(filename);
    fs::write(&path, json).map_err(|error| error.to_string())?;
    Ok(path)
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

#[derive(Debug, Clone)]
pub struct BackupPackage {
    pub name: String,
    pub state: crate::uad_lists::PackageState,
    pub user: User,
    pub index: usize,
    pub commands: Vec<String>,
    pub target: crate::uad_lists::PackageState,
}

#[derive(Debug, Clone)]
pub struct RestoreResult {
    pub packages: Vec<BackupPackage>,
    pub skipped_count: usize,
}

pub fn restore_backup(
    selected_device: &Phone,
    packages: &[Vec<CorePackage>],
    settings: &DeviceSettings,
) -> Result<RestoreResult, String> {
    restore_backup_with_errors(
        selected_device,
        packages,
        settings,
        &HashMap::<usize, String>::new(),
    )
}

pub fn restore_backup_with_errors(
    selected_device: &Phone,
    packages: &[Vec<CorePackage>],
    settings: &DeviceSettings,
    inventory_errors: &HashMap<usize, String, impl std::hash::BuildHasher>,
) -> Result<RestoreResult, String> {
    let backup = PhoneBackup::read(
        settings
            .backup
            .selected
            .as_ref()
            .ok_or("No backup selected")?,
    )?;
    backup.plan_restore(selected_device, |user, _| {
        if let Some(error) = inventory_errors.get(&user.index) {
            return Err(error.clone());
        }
        packages
            .get(user.index)
            .ok_or_else(|| format!("No package list loaded for {user}"))
    })
}

impl PhoneBackup {
    pub fn read(file: &DisplayablePath) -> Result<Self, String> {
        let data = fs::read(&file.path).map_err(|e| e.to_string())?;
        let backup: Self = serde_json::from_slice(&data)
            .map_err(|e| format!("The selected backup file is not valid: {e}"))?;
        if backup.users.is_empty() {
            return Err("The backup contains no users.".into());
        }
        Ok(backup)
    }

    /// Read every saved user's current inventory before any restore command is dispatched.
    pub fn plan_device_restore(self, phone: &Phone) -> Result<RestoreResult, String> {
        let mut phone = phone.clone();
        phone.refresh_users()?;
        let definitions = HashMap::new();
        self.plan_restore(&phone, |_, user_id| {
            try_fetch_packages(&definitions, &phone.adb_id, user_id)
        })
    }

    fn plan_restore<P: AsRef<[CorePackage]>>(
        self,
        selected_device: &Phone,
        read_packages: impl Fn(User, Option<u16>) -> Result<P, String>,
    ) -> Result<RestoreResult, String> {
        let mut commands = vec![];
        let mut skipped_packages = 0;
        for u in self.users {
            if u.packages.is_empty() {
                continue;
            }
            let user = match selected_device.user_list.iter().find(|x| x.id == u.id) {
                Some(user) => *user,
                None => return Err(format!("user {} doesn't exist", u.id)),
            };

            let user_id = selected_device.package_user_id(user.id)?;
            let inventory = read_packages(user, user_id).map_err(|error| {
                format!("Could not read {user}: {error}. Refresh the device and retry the restore.")
            })?;
            let user_packages = inventory.as_ref();
            if user_packages.is_empty() && !user.protected {
                return Err(format!(
                    "The packages of {user} could not be read. Refresh the device and retry the restore."
                ));
            }
            let target_user = user_id.map(|_| user);
            for backup_package in &u.packages {
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
                let p_commands = match commands_for_user(
                    package,
                    backup_package.state,
                    target_user,
                    selected_device,
                ) {
                    Ok(actions) => actions,
                    Err(error) => {
                        skipped_packages += 1;
                        warn!(
                            "Skipping {} for {user} during restore: {error}",
                            package.name
                        );
                        continue;
                    }
                };
                if !p_commands.is_empty() {
                    commands.push(BackupPackage {
                        name: package.name.clone(),
                        state: package.state,
                        user,
                        index: i,
                        commands: p_commands,
                        target: backup_package.state,
                    });
                } else if package.state != backup_package.state {
                    // Such as a package saved as disabled that is now uninstalled.
                    skipped_packages += 1;
                    warn!(
                        "{} cannot go from {} to {} for user {} - skipping package during restore",
                        package.name, package.state, backup_package.state, u.id
                    );
                }
            }
        }
        if skipped_packages > 0 {
            info!("Restore planned with {skipped_packages} packages skipped");
        }
        Ok(RestoreResult {
            packages: commands,
            skipped_count: skipped_packages,
        })
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
                        package("com.example.uninstalled", PackageState::Disabled),
                    ],
                },
                // A protected profile saved no packages and has since been removed.
                UserBackup {
                    id: 11,
                    packages: Vec::new(),
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
                // An uninstalled package cannot be disabled.
                package("com.example.uninstalled", PackageState::Uninstalled),
            ],
        ];
        let errors = HashMap::from([(work.index, "device offline".to_owned())]);
        let error = restore_backup_with_errors(&phone, &packages, &settings, &errors).unwrap_err();
        assert!(error.contains("user 10") && error.contains("device offline"));
        let empty_work = [packages[0].clone(), vec![]];
        let empty_error = restore_backup(&phone, &empty_work, &settings).unwrap_err();
        assert!(empty_error.contains("user 10") && empty_error.contains("Refresh"));
        let result = restore_backup(&phone, &packages, &settings).unwrap();
        let actions: Vec<_> = result
            .packages
            .iter()
            .filter(|p| !p.commands.is_empty())
            .collect();
        assert_eq!(result.skipped_count, 2);
        assert_eq!(actions.len(), 3);
        assert_eq!((actions[0].user.index, actions[0].index), (0, 0));
        assert!(actions[0].commands[0].ends_with("--user 0 com.example.owner"));
        assert_eq!((actions[1].user.index, actions[1].index), (1, 1));
        assert!(actions[1].commands[0].ends_with("--user 10 com.example.second"));
        assert_eq!((actions[2].user.index, actions[2].index), (1, 0));
        assert!(actions[2].commands[0].ends_with("--user 10 com.example.first"));
        assert!(restore_backup(&phone, &[], &settings).is_err());
    }

    #[test]
    fn restore_skips_an_unsupported_reinstall_but_requires_a_known_sdk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("backup.json");
        let known = "com.example.disabled";
        let removed = "com.example.removed";
        let backup = PhoneBackup {
            device_id: "phone".into(),
            users: vec![UserBackup {
                id: 0,
                packages: vec![
                    package(removed, PackageState::Enabled),
                    package(known, PackageState::Enabled),
                ],
            }],
        };
        fs::write(&path, serde_json::to_vec(&backup).unwrap()).unwrap();
        let settings = DeviceSettings {
            backup: BackupSettings {
                selected: Some(DisplayablePath { path }),
                ..BackupSettings::default()
            },
            ..DeviceSettings::default()
        };
        let packages = [vec![
            package(known, PackageState::Disabled),
            package(removed, PackageState::Uninstalled),
        ]];
        let mut phone = Phone {
            android_sdk: 18,
            user_list: vec![User::default()],
            ..Phone::default()
        };
        let plan = restore_backup(&phone, &packages, &settings).unwrap();
        assert_eq!(plan.skipped_count, 1);
        assert_eq!(plan.packages.len(), 1);
        assert_eq!(plan.packages[0].commands, [format!("pm enable {known}")]);
        phone.android_sdk = 0;
        assert!(restore_backup(&phone, &packages, &settings).is_err());
    }

    #[test]
    fn backup_is_saved_in_the_device_folder() {
        let dir = tempdir().unwrap();
        let serial = "192.168.1.5:5555";
        let users = vec![User::default()];
        let packages = vec![vec![package("com.example.app", PackageState::Enabled)]];

        assert!(backup_phone(dir.path(), users.clone(), serial.to_string(), &[]).is_err());
        assert!(backup_phone(dir.path(), vec![], serial.to_string(), &packages).is_err());
        // The packages of the user could not be read.
        assert!(backup_phone(dir.path(), users.clone(), serial.to_string(), &[vec![]]).is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);

        let written = backup_phone(dir.path(), users, serial.to_string(), &packages).unwrap();
        let backups = list_available_backups(&device_backup_dir(dir.path(), serial));
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].path, written);
        let saved: PhoneBackup =
            serde_json::from_slice(&fs::read(&backups[0].path).unwrap()).unwrap();
        assert_eq!(saved.device_id, serial);
        assert_eq!(saved.users[0].packages, packages[0]);
    }

    #[test]
    fn restoring_a_blocked_package_preserves_its_data() {
        let backup = PhoneBackup {
            device_id: "test".into(),
            users: vec![UserBackup {
                id: 0,
                packages: vec![package("com.example.app", PackageState::Enabled)],
            }],
        };
        for sdk in [19, 20] {
            let phone = Phone {
                android_sdk: sdk,
                user_list: vec![User::default()],
                ..Phone::default()
            };
            let plan = backup
                .clone()
                .plan_restore(&phone, |_, user_id| {
                    assert!(user_id.is_none());
                    Ok(vec![package("com.example.app", PackageState::Uninstalled)])
                })
                .unwrap();
            assert_eq!(plan.packages[0].commands, ["pm unblock com.example.app"]);
        }
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
        fs::write(
            &settings.backup.selected.as_ref().unwrap().path,
            r#"{"device_id":"old","users":[]}"#,
        )
        .unwrap();
        assert_eq!(
            restore_backup(&Phone::default(), &[], &settings).unwrap_err(),
            "The backup contains no users."
        );
    }
}
