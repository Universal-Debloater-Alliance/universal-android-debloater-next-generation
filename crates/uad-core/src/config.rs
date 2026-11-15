use crate::CACHE_DIR;
use crate::CONFIG_DIR;
use crate::save::DisplayablePath;
use crate::sync::User;
use log::{error, info};
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
        write_config_file(&CONFIG_FILE, self);
    }

    #[must_use]
    pub fn load_configuration_file() -> Self {
        load_config_file(&CONFIG_FILE)
    }
}

/// Reads the config at `path`. A missing or invalid file is replaced by the defaults.
fn load_config_file(path: &Path) -> Config {
    match fs::read_to_string(path) {
        Ok(s) => match toml::from_str(&s) {
            Ok(config) => return config,
            Err(e) => error!("Invalid config file: `{e}`"),
        },
        // A missing file is normal on the first launch.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("No config file yet, creating the default one");
        }
        Err(e) => error!("Failed to read config file: `{e}`"),
    }
    let config = Config::default();
    write_config_file(path, &config);
    config
}

/// Saves `config` to `path`. A failure is logged instead of crashing the app,
/// because the settings still apply to the current session.
fn write_config_file(path: &Path, config: &Config) {
    match toml::to_string(config) {
        Ok(toml) => {
            if let Err(e) = fs::write(path, toml) {
                error!("Could not write config file {}: {e}", path.display());
            }
        }
        Err(e) => error!("Could not serialize config: {e}"),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests. They use a scratch file, never the user's real config.

    use super::*;

    /// A config path inside a fresh folder that is unique to `test_name`.
    fn scratch_config_file(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "uad-config-test-{}-{test_name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("config.toml")
    }

    #[test]
    fn missing_file_is_created_with_defaults() {
        let path = scratch_config_file("missing");
        let config = load_config_file(&path);
        assert!(path.exists());
        assert_eq!(config.general.theme, DEFAULT_THEME);
        assert!(config.devices.is_empty());
    }

    #[test]
    fn invalid_file_falls_back_to_defaults() {
        let path = scratch_config_file("invalid");
        fs::write(&path, "general = 3").unwrap();
        let config = load_config_file(&path);
        assert_eq!(config.general.theme, DEFAULT_THEME);
    }

    #[test]
    fn saved_device_settings_load_back() {
        let path = scratch_config_file("round-trip");
        let mut config = Config::default();
        config.devices.push(DeviceSettings {
            device_id: "test_device".to_string(),
            disable_mode: true,
            ..DeviceSettings::default()
        });
        write_config_file(&path, &config);
        let loaded = load_config_file(&path);
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(loaded.devices[0].device_id, "test_device");
        assert!(loaded.devices[0].disable_mode);
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
