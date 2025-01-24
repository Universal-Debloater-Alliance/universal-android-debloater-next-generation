use crate::core::{
    adb::{to_trimmed_utf8, ACommand as AdbCommand, PM_CLEAR_PACK},
    uad_lists::PackageState,
};
use crate::gui::{views::list::PackageInfo, widgets::package_row::PackageRow};
use regex::Regex;
use retry::{delay::Fixed, retry, OperationResult};
use serde::{Deserialize, Serialize};
use static_init::dynamic;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// An Android device, typically a phone
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phone {
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

impl Default for Phone {
    fn default() -> Self {
        Self {
            model: "fetching devices...".to_string(),
            android_sdk: 0,
            user_list: vec![],
            adb_id: String::default(),
        }
    }
}

impl std::fmt::Display for Phone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.model)
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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

/// Runs an **arbitrary command** on the device's default `sh` implementation.
/// Typically MKSH, but could be Ash.
/// [More info](https://chromium.googlesource.com/aosp/platform/system/core/+/refs/heads/upstream/shell_and_utilities).
///
/// If `serial` is empty, it lets ADB choose the default device.
#[deprecated = "Use [`adb::ACommand::shell`] with `async` blocks instead"]
pub async fn adb_shell_command<S: AsRef<str>>(
    device_serial: S,
    action: String,
    command_type: CommandType,
) -> Result<CommandType, AdbError> {
    let serial = device_serial.as_ref();

    let label = match &command_type {
        CommandType::PackageManager(p) => &p.removal,
        CommandType::Shell => "Shell",
    };

    let mut cmd = Command::new("adb");
    if !serial.is_empty() {
        cmd.args(["-s", serial]);
    };
    cmd.arg("shell");
    // this works because `sh` splits spaces
    cmd.arg(&action);

    #[cfg(target_os = "windows")]
    let cmd = cmd.creation_flags(0x0800_0000); // do not open a cmd window

    match match cmd.output() {
        Err(e) => {
            error!("ADB: {}", e);
            Err("Cannot run ADB, likely not found".to_string())
        }
        Ok(o) => {
            let stdout = to_trimmed_utf8(o.stdout);
            if o.status.success() {
                Ok(stdout)
            } else {
                let stderr = to_trimmed_utf8(o.stderr);

                // ADB does really weird things. Some errors are not redirected to stderr
                let err = if stdout.is_empty() { stderr } else { stdout };
                Err(err)
            }
        }
    } {
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
pub fn user_flag(user_id: Option<User>) -> String {
    user_id
        .map(|user| format!(" --user {}", user.id))
        .unwrap_or_default()
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
    selected_user: User,
    dev: &Phone,
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
pub fn request_builder(commands: &[&str], package: &str, user: Option<User>) -> Vec<String> {
    let maybe_user_flag = user_flag(user);
    commands
        .iter()
        .map(|c| format!("{c}{maybe_user_flag} {package}"))
        .collect()
}

/// Get the model by querying the `ro.product.model` property.
///
/// If `serial` is empty, it lets ADB choose the default device.
pub fn get_device_model(serial: &str) -> String {
    AdbCommand::new()
        .shell(serial)
        .getprop("ro.product.model")
        .unwrap_or_else(|err| {
            eprintln!("ERROR: {err}");
            error!("{err}");
            if err.contains("adb: no devices/emulators found") {
                "no devices/emulators found".to_string()
            } else {
                err
            }
        })
}

/// Get the brand by querying the `ro.product.brand` property.
///
/// If `serial` is empty, it lets ADB choose the default device.
pub fn get_device_brand(serial: &str) -> String {
    AdbCommand::new()
        .shell(serial)
        .getprop("ro.product.brand")
        // `trim` is just-in-case
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Get Android SDK version by querying the
// `ro.build.version.sdk` property or defaulting to 0.
///
/// If `device_serial` is empty, it lets ADB choose the default device.
pub fn get_android_sdk(device_serial: &str) -> u8 {
    AdbCommand::new()
        .shell(device_serial)
        .getprop("ro.build.version.sdk")
        .map_or(0, |sdk| {
            sdk.parse().expect("SDK version numeral must be valid")
        })
}

/// Minimum inclusive Android SDK version
/// that supports multi-user mode.
/// Lollipop 5.0
pub const MULTI_USER_SDK: u8 = 21;

/// Check if it supports multi-user mode, by comparing SDK version.
#[must_use]
pub const fn supports_multi_user(dev: &Phone) -> bool {
    dev.android_sdk >= MULTI_USER_SDK
}

/// Check if a `user_id` is protected on a device by trying
/// to list associated packages.
///
/// If `device_serial` is empty, it lets ADB choose the default device.
pub fn is_protected_user<S: AsRef<str>>(user_id: u16, device_serial: S) -> bool {
    AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_packages_sys(None, Some(user_id))
        .is_err()
}

/// `pm list users` parsed into a vec with extra info.
pub fn ls_users_parsed(device_serial: &str) -> Vec<User> {
    #[dynamic]
    static RE: Regex = Regex::new(r"\{([0-9]+)").unwrap_or_else(|_| unreachable!());

    AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_users()
        // if default, then empty iter, which becomes empty vec (again)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(i, user)| {
            // It seems each line is a user,
            // optionally associated with a work-profile.
            // This will ignore the work-profiles!
            let u = RE.captures(&user).expect("Each user should have an ID")[1]
                .parse()
                .unwrap_or_else(|_| unreachable!("User ID must be valid `u16`"));
            User {
                id: u,
                index: i,
                protected: is_protected_user(u, device_serial),
            }
        })
        .collect()
}

/// This matches serials (`getprop ro.serialno`)
/// that are authorized by the user.
pub async fn get_devices_list() -> Vec<Phone> {
    retry(
        Fixed::from_millis(500).take(if cfg!(debug_assertions) { 3 } else { 120 }),
        || match AdbCommand::new().devices() {
            Ok(devices) => {
                let mut device_list: Vec<Phone> = vec![];
                if devices.iter().all(|(_, stat)| stat != "device") {
                    return OperationResult::Retry(vec![]);
                }
                for device in devices {
                    let serial = &device.0;
                    device_list.push(Phone {
                        model: format!("{} {}", get_device_brand(serial), get_device_model(serial)),
                        android_sdk: get_android_sdk(serial),
                        user_list: ls_users_parsed(serial),
                        adb_id: serial.to_string(),
                    });
                }
                OperationResult::Ok(device_list)
            }
            Err(err) => {
                error!("get_device_list() -> {}", err);
                let test: Vec<Phone> = vec![];
                OperationResult::Retry(test)
            }
        },
    )
    .unwrap_or_default()
}

pub async fn initial_load() -> bool {
    match AdbCommand::new().devices() {
        Ok(_devices) => true,
        Err(_err) => false,
    }
}
