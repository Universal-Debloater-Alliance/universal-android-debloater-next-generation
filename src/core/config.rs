use crate::core::utils::DisplayablePath;
use crate::core::{
    sync::{get_android_sdk, User},
    theme::Theme,
};
use crate::gui::views::settings::Settings;
use crate::CACHE_DIR;
use crate::CONFIG_DIR;
use serde::{Deserialize, Serialize};
use static_init::dynamic;
use std::fs;
use std::path::PathBuf;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceSettings {
    pub device_id: String,
    pub disable_mode: bool,
    pub multi_user_mode: bool,
    #[serde(skip)]
    pub backup: BackupSettings,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            theme: Theme::default().to_string(),
            expert_mode: false,
            backup_folder: CACHE_DIR.join("backups"),
        }
    }
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            device_id: String::default(),
            multi_user_mode: get_android_sdk() > 21,
            disable_mode: false,
            backup: BackupSettings::default(),
        }
    }
}

#[dynamic]
static CONFIG_FILE: PathBuf = CONFIG_DIR.join("config.toml");

impl Config {
    pub fn save_changes(settings: &Settings, device_id: &String) {
        let mut config = Self::load_configuration_file();
        if let Some(device) = config
            .devices
            .iter_mut()
            .find(|x| x.device_id == *device_id)
        {
            device.clone_from(&settings.device);
        } else {
            debug!("config: New device settings saved");
            config.devices.push(settings.device.clone());
        }
        config.general.clone_from(&settings.general);
        let toml = toml::to_string(&config).unwrap();
        fs::write(&*CONFIG_FILE, toml).expect("Could not write config file to disk!");
    }

    pub fn load_configuration_file() -> Self {
        match fs::read_to_string(&*CONFIG_FILE) {
            Ok(s) => match toml::from_str(&s) {
                Ok(config) => return config,
                Err(e) => error!("Invalid config file: `{}`", e),
            },
            Err(e) => error!("Failed to read config file: `{}`", e),
        }
        error!("Restoring default config file");
        let toml = toml::to_string(&Self::default()).unwrap();
        fs::write(&*CONFIG_FILE, toml).expect("Could not write config file to disk!");
        Self::default()
    }
}

//write unit tests:
#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // create a clean default config file for testing
    fn create_default_config_file() {
        let toml = toml::to_string(&Config::default()).unwrap();
        fs::write(&*CONFIG_FILE, toml).expect("Could not write config file to disk!");
    }

    #[test]
    fn test_create_default_config_file() {
        create_default_config_file();
        assert!(CONFIG_FILE.exists());
    }

    #[test]
    fn test_load_configuration_file() {
        create_default_config_file();
        let config = Config::load_configuration_file();
        assert_eq!(config.devices.len(), 0);
        assert_eq!(config.general.theme, Theme::default().to_string());
        assert!(!config.general.expert_mode);
        assert_eq!(config.general.backup_folder, CACHE_DIR.join("backups"));
    }

    #[test]
    fn test_save_changes() {
        let mut settings = Settings::default();
        let device_id = "test_device".to_string();
        settings.device.device_id = device_id.clone();
        Config::save_changes(&settings, &device_id);
        let config = Config::load_configuration_file();
        assert!(!config.devices.is_empty(), "Devices list is empty after saving changes!");
        assert_eq!(config.devices[0].device_id, device_id);
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.devices.len(), 0);
        assert_eq!(config.general.theme, Theme::default().to_string());
        assert!(!config.general.expert_mode);
        assert_eq!(config.general.backup_folder, CACHE_DIR.join("backups"));
    }

    #[test]
    fn test_config_file_path() {
        assert_eq!(&*CONFIG_FILE, Path::new(&*CONFIG_DIR.join("config.toml")));
    }
}
