use crate::core::{
    adb::{ACommand as AdbCommand, PM_CLEAR_PACK},
    uad_lists::PackageState,
};
use crate::gui::{views::list::PackageInfo, widgets::package_row::PackageRow};
use retry::{OperationResult, delay::Fixed, retry};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::Command;

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

/// `UserInfo` but relevant to UAD
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

/// ADB error type (GUI expects `Generic(String)` for errors)
#[derive(Debug, Clone)]
pub enum AdbError {
    Generic(String),
}

/// Runs an **arbitrary command** on the device's default shell.
/// Be cautious when calling this to avoid introducing security issues.
///
/// Run an ADB shell command for a specific device.
/// Returns Ok(p_info) on success, or Err(AdbError::Generic(msg)) on failure.
/// NOTE: `async` because it's awaited via `Command::perform(...)` in the GUI.
#[deprecated = "Use [`adb::ACommand::shell`] with `async` blocks instead"]
pub async fn adb_shell_command(
    adb_id: String,
    command: String,
    p_info: PackageInfo,
) -> Result<PackageInfo, AdbError> {
    let mut cmd = Command::new("adb");
    #[cfg(target_os = "windows")]
    {
        // Avoid opening a new console window on older Windows
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // Only pass -s when we actually have an ID; otherwise let ADB pick default device
    if !adb_id.is_empty() {
        cmd.args(["-s", &adb_id]);
    }
    let output = cmd.args(["shell", &command]).output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            return Err(AdbError::Generic(format!(
                "Failed to start ADB: {}\n\
                 • Make sure Android Platform Tools are installed and `adb` is in PATH.\n\
                 • Check that the device is connected and authorized (run `adb devices`).",
                e
            )));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        // Many OEMs print failure info to stdout instead of stderr.
        let mut msg = if !stderr.is_empty() {
            stderr
        } else {
            stdout.clone()
        };

        if let Some(hint) = friendly_hint(&msg) {
            msg.push_str(&format!("\nTip: {}", hint));
        }
        //             Ok(o) => {
        //         let stdout = to_trimmed_utf8(o.stdout);
        //         if o.status.success() {
        //             Ok(stdout)
        //         } else {
        //             let stderr = to_trimmed_utf8(o.stderr);

        //             // ADB does really weird things. Some errors are not redirected to stderr
        //             let err = if stdout.is_empty() { stderr } else { stdout };
        //             Err(err)
        //         }
        //     }
        // } {
        //     Ok(o) => {
        // On old devices, adb commands can return the `0` exit code even if there
        // is an error. On Android 4.4, ADB doesn't check if the package exists.
        // It does not return any error if you try to `pm block` a non-existent package.
        // Some commands are even killed by ADB before finishing and UAD-ng can't catch
        // the output.

        return Err(AdbError::Generic(msg));
    }

    if !stdout.is_empty() {
        info!("[adb ok] {}", stdout);
    }
    if !stderr.is_empty() {
        warn!("[adb warn] {}", stderr);
    }

    Ok(p_info)
}

/// Map frequent OEM failure strings to actionable hints shown to the user.
fn friendly_hint(err_msg: &str) -> Option<&'static str> {
    let e = err_msg;
    if e.contains("Shell cannot change component state for null to 1") {
        // The "null" target indicates the package/component argument was missing.
        return Some("Package name was empty when enabling. Refresh the package list and retry.");
    }
    if e.contains("DELETE_FAILED_USER_RESTRICTED")
        || e.contains("package is protected")
        || e.contains("Cannot uninstall a protected package")
    {
        Some("Package is protected by the vendor. Try Disable instead.")
    } else if e.contains("NOT_INSTALLED_FOR_USER") || e.contains("Unknown package:") {
        Some("It seems this app isn’t installed for the selected user. Refresh the list.")
    } else if e.contains("permission to access user")
        || e.contains("Shell does not have permission to access user")
    {
        Some("Wrong user/profile. Select the primary user or a permitted profile.")
    } else if e.contains("Failure [") && e.contains(']') {
        Some(
            "Android Package Manager rejected the operation. Try Disable, or re-check Expert Mode.",
        )
    } else {
        None
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
    phone: &Phone,
) -> Vec<String> {
    // If the package name is empty, bail early (avoid "null" target)
    if package.name.trim().is_empty() {
        error!("apply_pkg_state_commands: empty package name, skipping command build");
        return vec![];
    }

    // https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki/ADB-reference
    // ALWAYS PUT THE COMMAND THAT CHANGES THE PACKAGE STATE FIRST!
    let commands = match wanted_state {
        PackageState::Enabled => match package.state {
            PackageState::Disabled => vec!["pm enable"],
            PackageState::Uninstalled => match phone.android_sdk {
                i if i >= 23 => vec!["cmd package install-existing"],
                21 | 22 => vec!["pm unhide"],
                19 | 20 => vec!["pm unblock", PM_CLEAR_PACK],
                _ => unreachable!("already prevented by the GUI"),
            },
            _ => vec![],
        },
        PackageState::Disabled => match package.state {
            PackageState::Uninstalled | PackageState::Enabled => match phone.android_sdk {
                sdk if sdk >= 23 => vec!["pm disable-user", "am force-stop", PM_CLEAR_PACK],
                _ => vec![],
            },
            _ => vec![],
        },
        PackageState::Uninstalled => match package.state {
            PackageState::Enabled | PackageState::Disabled => match phone.android_sdk {
                sdk if sdk >= 23 => vec!["pm uninstall"], // > Android Marshmallow (6.0)
                21 | 22 => vec!["pm hide", PM_CLEAR_PACK], // Android Lollipop (5.x)
                _ => vec!["pm block", PM_CLEAR_PACK],     // very old devices
            },
            _ => vec![],
        },
        PackageState::All => vec![],
    }; // `len <= 4`

    let user = supports_multi_user(phone).then_some(selected_user);
    request_builder(&commands, &package.name, user)
}

/// Build a command request to be sent via ADB to a device.
pub fn request_builder(commands: &[&str], package: &str, user: Option<User>) -> Vec<String> {
    let p = package.trim();
    if p.is_empty() {
        error!("request_builder: empty package name; not issuing adb shell command");
        return vec![]; // no-ops — prevents "null" errors from reaching ADB
    }

    let maybe_user_flag = user_flag(user);
    commands
        .iter()
        .map(|c| format!("{c}{maybe_user_flag} {p}"))
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
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Get Android SDK version by querying `ro.build.version.sdk` or defaulting to 0.
/// If `device_serial` is empty, it lets ADB choose the default device.
pub fn get_android_sdk(device_serial: &str) -> u8 {
    AdbCommand::new()
        .shell(device_serial)
        .getprop("ro.build.version.sdk")
        .map_or(0, |sdk| {
            sdk.parse().expect("SDK version numeral must be valid")
        })
}

/// Minimum inclusive Android SDK version that supports multi-user mode (Lollipop 5.0)
pub const MULTI_USER_SDK: u8 = 21;

/// Check if it might support multi-user mode by simply comparing SDK version.
#[must_use]
pub const fn supports_multi_user(dev: &Phone) -> bool {
    dev.android_sdk >= MULTI_USER_SDK
}

/// Check if a `user_id` is protected on a device by trying to list associated packages.
pub fn is_protected_user<S: AsRef<str>>(user_id: u16, device_serial: S) -> bool {
    AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_packages_sys(None, Some(user_id))
        .is_err()
}

pub fn list_users_idx_prot(device_serial: &str) -> Vec<User> {
    AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_users()
        .map(|out| {
            out.into_iter()
                .enumerate()
                .map(|(i, user)| {
                    let id = user.get_id();
                    User {
                        id,
                        index: i,
                        protected: is_protected_user(id, device_serial),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// This matches serials (`getprop ro.serialno`) that are authorized by the user.
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
                        user_list: list_users_idx_prot(serial),
                        adb_id: serial.to_string(),
                    });
                }
                OperationResult::Ok(device_list)
            }
            Err(err) => {
                error!("get_devices_list() -> {err}");
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
