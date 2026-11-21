use crate::CACHE_DIR;
use crate::CONFIG_DIR;
use crate::sync::User;
use crate::utils::DisplayablePath;
use log::error;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const DEFAULT_THEME: &str = "Auto (follow system theme)";

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub general: GeneralSettings,
    #[serde(skip_serializing_if = "Vec::is_empty", default = "Vec::new")]
    pub devices: Vec<DeviceSettings>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneralSettings {
    pub theme: String,
    pub expert_mode: bool,
    pub backup_folder: PathBuf,
}

#[derive(Default, Debug, Clone)]
pub struct BackupSettings {
    pub backups: Vec<DisplayablePath>,
    pub selected: Option<DisplayablePath>,
    pub users: Vec<User>,
    pub selected_user: Option<User>,
    pub backup_state: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DeviceSettings {
    /// Unique serial identifier
    pub device_id: String,
    pub disable_mode: bool, // should be `enum RemovalMode`
    pub multi_user_mode: bool,
    #[serde(skip)]
    pub backup: BackupSettings,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME.to_string(),
            expert_mode: false,
            backup_folder: CACHE_DIR.join("backups"),
        }
    }
}

static CONFIG_FILE: LazyLock<PathBuf> = LazyLock::new(|| CONFIG_DIR.join("config.toml"));

impl Config {
    pub fn save_device_settings(
        &mut self,
        device_settings: DeviceSettings,
        general: GeneralSettings,
    ) {
        self.save_device_settings_to_path(device_settings, general, &CONFIG_FILE);
    }

    fn save_device_settings_to_path(
        &mut self,
        device_settings: DeviceSettings,
        general: GeneralSettings,
        path: &Path,
    ) {
        if let Some(device) = self
            .devices
            .iter_mut()
            .find(|x| x.device_id == device_settings.device_id)
        {
            *device = device_settings;
        } else {
            self.devices.push(device_settings);
        }
        self.general = general;
        let toml = toml::to_string(&self).unwrap();
        fs::write(path, toml).expect("Could not write config file to disk!");
    }

    #[must_use]
    pub fn load_configuration_file() -> Self {
        Self::load_from_path(&CONFIG_FILE)
    }

    fn load_from_path(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(s) => match toml::from_str(&s) {
                Ok(config) => return config,
                Err(e) => error!("Invalid config file: `{e}`"),
            },
            Err(e) => error!("Failed to read config file: `{e}`"),
        }
        error!("Restoring default config file");
        let toml = toml::to_string(&Self::default()).unwrap();
        fs::write(path, toml).expect("Could not write config file to disk!");
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_config_creates_defaults_in_the_requested_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = Config::load_from_path(&path);
        let saved: Config = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(config.general.theme, DEFAULT_THEME);
        assert_eq!(saved.general.theme, DEFAULT_THEME);
        assert!(saved.devices.is_empty());
    }

    #[test]
    fn valid_config_preserves_settings_and_file_contents() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let contents = r#"
[general]
theme = "Light"
expert_mode = true
backup_folder = "custom-backups"

[[devices]]
device_id = "test-device"
disable_mode = true
multi_user_mode = false
"#;
        fs::write(&path, contents).unwrap();
        let config = Config::load_from_path(&path);
        assert_eq!(config.general.theme, "Light");
        assert!(config.general.expert_mode);
        assert_eq!(config.general.backup_folder, Path::new("custom-backups"));
        assert_eq!(config.devices.len(), 1);
        assert!(config.devices[0].disable_mode);
        assert_eq!(fs::read_to_string(path).unwrap(), contents);
    }

    #[test]
    fn invalid_config_is_replaced_with_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "invalid = [").unwrap();
        let config = Config::load_from_path(&path);
        assert_eq!(config.general.theme, DEFAULT_THEME);
        let saved: Config = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(saved.general.theme, DEFAULT_THEME);
    }

    #[test]
    fn saving_settings_persists_a_new_device_and_general_settings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = Config::default();
        let device = DeviceSettings {
            device_id: "test-device".to_string(),
            disable_mode: true,
            multi_user_mode: true,
            ..DeviceSettings::default()
        };
        let general = GeneralSettings {
            theme: "Light".to_string(),
            expert_mode: true,
            backup_folder: dir.path().join("backups"),
        };
        config.save_device_settings_to_path(device, general, &path);

        let saved = Config::load_from_path(&path);
        assert_eq!(saved.devices.len(), 1);
        assert_eq!(saved.devices[0].device_id, "test-device");
        assert!(saved.devices[0].disable_mode);
        assert!(saved.devices[0].multi_user_mode);
        assert_eq!(saved.general.theme, "Light");
        assert!(saved.general.expert_mode);
        assert_eq!(saved.general.backup_folder, dir.path().join("backups"));
    }

    #[test]
    fn saving_settings_updates_the_matching_device_and_preserves_other_devices() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut config = Config {
            devices: vec![
                DeviceSettings {
                    device_id: "other-device".to_string(),
                    multi_user_mode: true,
                    ..DeviceSettings::default()
                },
                DeviceSettings {
                    device_id: "test-device".to_string(),
                    ..DeviceSettings::default()
                },
            ],
            ..Config::default()
        };
        let device = DeviceSettings {
            device_id: "test-device".to_string(),
            disable_mode: true,
            ..DeviceSettings::default()
        };
        config.save_device_settings_to_path(device, GeneralSettings::default(), &path);

        let saved = Config::load_from_path(&path);
        assert_eq!(saved.devices.len(), 2);
        assert_eq!(saved.devices[0].device_id, "other-device");
        assert!(saved.devices[0].multi_user_mode);
        assert!(!saved.devices[0].disable_mode);
        assert_eq!(saved.devices[1].device_id, "test-device");
        assert!(saved.devices[1].disable_mode);
        assert!(!saved.devices[1].multi_user_mode);
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.devices.len(), 0);
        assert_eq!(config.general.theme, DEFAULT_THEME);
        assert!(!config.general.expert_mode);
        assert_eq!(config.general.backup_folder, CACHE_DIR.join("backups"));
    }

    #[test]
    fn test_config_file_path() {
        assert_eq!(&*CONFIG_FILE, Path::new(&*CONFIG_DIR.join("config.toml")));
    }
}
