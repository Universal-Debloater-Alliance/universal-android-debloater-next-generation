use crate::core::uad_lists::PackageState;
use crate::gui::views::list::PackageInfo;
use crate::gui::widgets::package_row::PackageRow;
use regex::Regex;
use retry::{delay::Fixed, retry, OperationResult};
use serde::{Deserialize, Serialize};
use static_init::dynamic;
use std::collections::HashSet;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const PM_LIST_PACKS: &str = "pm list packages";
const PM_CLEAR_PACK: &str = "pm clear";

#[dynamic]
static RE: Regex = Regex::new(r"\n(\S+)\s+device").unwrap_or_else(|_| unreachable!());

/// An Android device, typically a phone
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Non-market name
    pub model: String, // could be `Copy`
    /// Android API level version
    pub android_sdk: u8,
    /// In theory, `len < u16::MAX` _should_ always be `true`.
    /// In practice, `len <= u8::MAX`.
    pub user_list: Vec<User>,
    /// Unique serial identifier
    pub adb_id: String, // could be `Copy`
}

impl Default for Device {
    fn default() -> Self {
        Self {
            model: "fetching devices...".to_string(),
            android_sdk: 0,
            user_list: vec![],
            adb_id: String::default(),
        }
    }
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.model)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Copy)]
pub struct User {
    pub id: u16,
    pub index: usize,
    pub protected: bool,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "user {}", self.id)
    }
}

pub fn adb_shell_command(shell: bool, serial: Option<&str>, args: &str) -> Result<String, String> {
    // this could be a `tinyvec` or `arrayvec`
    let mut adb_args = Vec::with_capacity(4);
    if let Some(s) = serial {
        adb_args.extend(["-s", s]);
    };
    if shell {
        adb_args.push("shell");
    }
    // the rest
    adb_args.push(args);

    let mut command = Command::new("adb");
    command.args(adb_args);

    #[cfg(target_os = "windows")]
    let command = command.creation_flags(0x0800_0000); // do not open a cmd window

    match command.output() {
        Err(e) => {
            error!("ADB: {}", e);
            Err("Cannot run ADB, likely not found".to_string())
        }
        Ok(o) => {
            let stdout = String::from_utf8(o.stdout)
                .map_err(|e| e.to_string())?
                .trim_end()
                .to_string();
            if o.status.success() {
                Ok(stdout)
            } else {
                let stderr = String::from_utf8(o.stderr)
                    .map_err(|e| e.to_string())?
                    .trim_end()
                    .to_string();

                // ADB does really weird things. Some errors are not redirected to stderr
                let err = if stdout.is_empty() { stderr } else { stdout };
                Err(err)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum CommandType {
    PackageManager(PackageInfo),
    Shell,
}

/// An enum to contain different variants for errors yielded by ADB.
#[derive(Debug, Clone)]
pub enum AdbError {
    Generic(String),
}

/// Runs a shell command on the device.
pub async fn android_sh_cmd<S: AsRef<str>>(
    device_serial: S,
    action: String,
    command_type: CommandType,
) -> Result<CommandType, AdbError> {
    let label = match &command_type {
        CommandType::PackageManager(p) => &p.removal,
        CommandType::Shell => "Shell",
    };

    match adb_shell_command(true, Some(device_serial.as_ref()), &action) {
        Ok(o) => {
            // On old devices, adb commands can return the `0` exit code even if there
            // is an error. On Android 4.4, ADB doesn't check if the package exists.
            // It does not return any error if you try to `pm block` a non-existent package.
            // Some commands are even killed by ADB before finishing and UAD-ng can't catch
            // the output.
            if ["Error", "Failure"].iter().any(|&e| o.contains(e)) {
                return Err(AdbError::Generic(format!("[{label}] {action} -> {o}")));
            }

            info!("[{label}] {action} -> {o}");
            Ok(command_type)
        }
        Err(err) => {
            if !err.contains("[not installed for") {
                return Err(AdbError::Generic(format!("[{label}] {action} -> {err}")));
            }
            Err(AdbError::Generic(err))
        }
    }
}

/// If `None`, returns an empty String, not " --user 0"
pub fn user_flag(user_id: Option<&User>) -> String {
    user_id
        .map(|user| format!(" --user {}", user.id))
        .unwrap_or_default()
}

/// installed and uninstalled packages
pub fn list_all_system_packages(device_serial: &str, user_id: Option<&User>) -> String {
    let action = format!("{PM_LIST_PACKS} -s -u{}", user_flag(user_id));

    adb_shell_command(true, Some(device_serial), &action)
        .unwrap_or_default()
        .replace("package:", "")
}

pub fn hashset_system_packages(
    state: PackageState,
    device_serial: &str,
    user_id: Option<&User>,
) -> HashSet<String> {
    let user = user_flag(user_id);
    let action = match state {
        PackageState::Enabled => format!("{PM_LIST_PACKS} -s -e{user}"),
        PackageState::Disabled => format!("{PM_LIST_PACKS} -s -d{user}"),
        _ => String::default(), // You probably don't need to use this function for anything else
    };

    adb_shell_command(true, Some(device_serial), &action)
        .unwrap_or_default()
        .replace("package:", "")
        .lines()
        .map(String::from)
        .collect()
}

// Minimum information for processing adb commands
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct CorePackage {
    pub name: String,
    pub state: PackageState,
}

impl From<&mut PackageRow> for CorePackage {
    fn from(pr: &mut PackageRow) -> Self {
        Self {
            name: pr.name.clone(),
            state: pr.state,
        }
    }
}
impl From<PackageRow> for CorePackage {
    fn from(pr: PackageRow) -> Self {
        Self {
            name: pr.name.clone(),
            state: pr.state,
        }
    }
}

impl From<&PackageRow> for CorePackage {
    fn from(pr: &PackageRow) -> Self {
        Self {
            name: pr.name.clone(),
            state: pr.state,
        }
    }
}

pub fn apply_pkg_state_commands(
    package: &CorePackage,
    wanted_state: PackageState,
    selected_user: &User,
    dev: &Device,
) -> Vec<String> {
    // https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki/ADB-reference
    // ALWAYS PUT THE COMMAND THAT CHANGES THE PACKAGE STATE FIRST!
    let commands = match wanted_state {
        PackageState::Enabled => match package.state {
            PackageState::Disabled => match dev.android_sdk {
                i if i >= 23 => vec!["pm enable"],
                _ => vec!["pm enable"],
            },
            PackageState::Uninstalled => match dev.android_sdk {
                i if i >= 23 => vec!["cmd package install-existing"],
                21 | 22 => vec!["pm unhide"],
                19 | 20 => vec!["pm unblock", PM_CLEAR_PACK],
                _ => unreachable!("already prevented by the GUI"),
            },
            _ => vec![],
        },
        PackageState::Disabled => match package.state {
            PackageState::Uninstalled | PackageState::Enabled => match dev.android_sdk {
                sdk if sdk >= 23 => vec!["pm disable-user", "am force-stop", PM_CLEAR_PACK],
                _ => vec![],
            },
            _ => vec![],
        },
        PackageState::Uninstalled => match package.state {
            PackageState::Enabled | PackageState::Disabled => match dev.android_sdk {
                sdk if sdk >= 23 => vec!["pm uninstall"], // > Android Marshmallow (6.0)
                21 | 22 => vec!["pm hide", PM_CLEAR_PACK], // Android Lollipop (5.x)
                19 | 20 => vec!["pm block", PM_CLEAR_PACK], // Android KitKat (4.4/4.4W) and older
                _ => vec!["pm block", PM_CLEAR_PACK], // Disable mode is unavailable on older devices because the specific ADB commands need root
            },
            _ => vec![],
        },
        PackageState::All => vec![],
    }; // this should be a `tinyvec`, as `len <= 4`

    let user = supports_multi_user(dev).then_some(selected_user);
    request_builder(&commands, &package.name, user)
}

/// Build a command request to be sent via ADB to a device.
/// `commands` accepts one or more ADB shell commands
/// which act on a common `package` and `user`.
pub fn request_builder(commands: &[&str], package: &str, user: Option<&User>) -> Vec<String> {
    let maybe_user_flag = user_flag(user);
    commands
        .iter()
        .map(|c| format!("{}{} {}", c, maybe_user_flag, package))
        .collect()
}

/// Get the model by querying the `ro.product.model` property.
pub fn get_device_model(serial: &str) -> String {
    adb_shell_command(true, Some(serial), "getprop ro.product.model").unwrap_or_else(|err| {
        println!("ERROR: {err}");
        if err.contains("adb: no devices/emulators found") {
            "no devices/emulators found".to_string()
        } else {
            err
        }
    })
}

/// Get Android SDK version by querying the
// `ro.build.version.sdk` property or defaulting to 0.
pub fn get_android_sdk(device_serial: &str) -> u8 {
    adb_shell_command(true, Some(device_serial), "getprop ro.build.version.sdk").map_or(0, |sdk| {
        sdk.parse().expect("SDK version numeral must be valid")
    })
}

/// Get the brand by querying the `ro.product.brand` property.
pub fn get_device_brand(serial: &str) -> String {
    format!(
        "{} {}",
        adb_shell_command(true, Some(serial), "getprop ro.product.brand")
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
        get_device_model(serial)
    )
}

/// Minimum inclusive Android SDK version
/// that supports multi-user mode.
/// Lollipop 5.0
pub const MULTI_USER_SDK: u8 = 21;

/// Check if it supports multi-user mode, by comparing SDK version.
#[must_use]
pub const fn supports_multi_user(dev: &Device) -> bool {
    dev.android_sdk >= MULTI_USER_SDK
}

/// Check if a `user_id` is protected on a device by trying
/// to list associated packages.
pub fn is_protected_user(user_id: &str, device_serial: &str) -> bool {
    adb_shell_command(
        true,
        Some(device_serial),
        &format!("{PM_LIST_PACKS} -s --user {user_id}"),
    )
    .is_err()
}

pub fn get_user_list(device_serial: &str) -> Vec<User> {
    #[dynamic]
    static RE: Regex = Regex::new(r"\{([0-9]+)").unwrap_or_else(|_| unreachable!());
    adb_shell_command(true, Some(device_serial), "pm list users")
        .map(|users| {
            RE.find_iter(&users)
                .enumerate()
                .map(|(i, u)| User {
                    id: u.as_str()[1..].parse().unwrap(),
                    index: i,
                    protected: is_protected_user(&u.as_str()[1..], device_serial),
                })
                .collect()
        })
        .unwrap_or_default()
}

// getprop ro.serialno
pub async fn get_devices_list() -> Vec<Device> {
    retry(
        Fixed::from_millis(500).take(120),
        || match adb_shell_command(false, None, "devices") {
            Ok(devices) => {
                let mut device_list: Vec<Device> = vec![];
                if !RE.is_match(&devices) {
                    return OperationResult::Retry(vec![]);
                }
                for device in RE.captures_iter(&devices) {
                    let serial = &device[1];
                    device_list.push(Device {
                        model: get_device_brand(serial),
                        android_sdk: get_android_sdk(serial),
                        user_list: get_user_list(serial),
                        adb_id: serial.to_string(),
                    });
                }
                OperationResult::Ok(device_list)
            }
            Err(err) => {
                error!("get_device_list() -> {}", err);
                let test: Vec<Device> = vec![];
                OperationResult::Retry(test)
            }
        },
    )
    .unwrap_or_default()
}

pub async fn initial_load() -> bool {
    match adb_shell_command(false, None, "devices") {
        Ok(_devices) => true,
        Err(_err) => false,
    }
}
