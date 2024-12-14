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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackId(String);
impl PackId {
    /// Creates a package-ID if it's valid according to
    /// [this](https://developer.android.com/build/configure-app-module#set-application-id)
    pub fn new<S: AsRef<str>>(pid: S) -> Option<Self> {
        #[dynamic]
        static RE: Regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_]*(?:\.[a-zA-Z][a-zA-Z0-9_]*)+$")
            .unwrap_or_else(|_| unreachable!());

        let pid = pid.as_ref();

        if RE.is_match(pid) {
            Some(Self(pid.to_string()))
        } else {
            None
        }
    }
}

#[dynamic]
static DEV_RE: Regex = Regex::new(r"\n(\S+)\s+device").unwrap_or_else(|_| unreachable!());

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

/// Builder for an ADB CLI command,
/// using the type-state and new-type patterns.
///
/// This is not intended to model the entire ADB API.
/// It only models the subset that concerns UADNG.
///
/// [More info here](https://developer.android.com/tools/adb)
#[derive(Debug)]
pub struct AdbCmd(Command);
impl AdbCmd {
    pub fn new() -> Self {
        Self(Command::new("adb"))
    }
    /// If `device_serial` is empty, it lets ADB choose the default device.
    pub fn shell<S: AsRef<str>>(mut self, device_serial: S) -> AdbShCmd {
        let serial = device_serial.as_ref();
        if !serial.is_empty() {
            self.0.args(["-s", serial]);
        }
        self.0.arg("shell");
        AdbShCmd(self)
    }
    /// List detected devices:
    /// - USB
    /// - TCP/IP: WIFI, Ethernet, etc...
    /// - Local emulators
    pub fn devices(mut self) -> Result<String, String> {
        self.0.arg("devices");
        self.run()
    }
    /// Reboots default device
    pub fn reboot(mut self) -> Result<String, String> {
        self.0.arg("reboot");
        self.run()
    }
    pub fn run(self) -> Result<String, String> {
        let mut cmd = self.0;
        #[cfg(target_os = "windows")]
        let cmd = cmd.creation_flags(0x0800_0000); // do not open a cmd window

        match cmd.output() {
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
}

/// Builder for a command that runs on the device's default `sh` implementation.
/// Typically MKSH, but could be Ash.
///
/// [More info](https://chromium.googlesource.com/aosp/platform/system/core/+/refs/heads/upstream/shell_and_utilities).
#[derive(Debug)]
pub struct AdbShCmd(AdbCmd);
impl AdbShCmd {
    pub fn pm(mut self) -> ApmCmd {
        self.0 .0.arg("pm");
        ApmCmd(self)
    }
    /// Reboots device
    pub fn reboot(mut self) -> Result<String, String> {
        self.0 .0.arg("reboot");
        self.0.run()
    }
}

/// `pm list packages` flag/state/type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmLsPackFlag {
    /// All: Include uninstalled
    U,
    /// Only enabled
    E,
    /// Only disabled
    D,
}
impl PmLsPackFlag {
    // is there a trait for this?
    fn to_str(self) -> &'static str {
        match self {
            PmLsPackFlag::U => "-u",
            PmLsPackFlag::E => "-e",
            PmLsPackFlag::D => "-d",
        }
    }
}
#[expect(clippy::to_string_trait_impl, reason = "This is not user-facing")]
impl ToString for PmLsPackFlag {
    fn to_string(&self) -> String {
        self.to_str().to_string()
    }
}

/// Builder for an Android Package Manager command.
///
/// [More info](https://developer.android.com/tools/adb#pm)
#[derive(Debug)]
pub struct ApmCmd(AdbShCmd);
impl ApmCmd {
    pub fn list_packs(
        mut self,
        f: Option<PmLsPackFlag>,
        u: Option<User>,
    ) -> Result<Vec<PackId>, String> {
        let cmd = &mut self.0 .0 .0;
        cmd.args(["list", "packages", "-s"]);
        if let Some(s) = f {
            cmd.arg(s.to_str());
        };
        if let Some(u) = u {
            cmd.arg("--user");
            cmd.arg(u.id.to_string());
        };
        self.0 .0.run().map(|pack_ls| {
            pack_ls
                .lines()
                .map(|p_ln| {
                    debug_assert!(p_ln.starts_with(PACK_URI_SCHEME));
                    PackId::new(&p_ln[PACK_URI_LEN as usize..]).expect("One of these is wrong: `PackId` regex, ADB implementation. Or the spec now allows a wider char-set")
                })
                .collect()
        })
    }
    pub fn list_users(mut self) -> Result<String, String> {
        self.0 .0 .0.args(["list", "users"]);
        self.0 .0.run()
    }
}

/// If `shell`, it'll run a command on the device's default `sh` implementation.
/// Typically MKSH, but could be Ash.
/// [More info](https://chromium.googlesource.com/aosp/platform/system/core/+/refs/heads/upstream/shell_and_utilities).
///
/// If `serial` is empty, it lets ADB choose the default device.
///
/// If `shell`, it's likely you want `serial` to _not_ be empty.
fn adb_cmd(shell: bool, serial: &str, args: &str) -> Result<String, String> {
    let mut cmd = Command::new("adb");
    if !serial.is_empty() {
        cmd.args(["-s", serial]);
    };
    if shell {
        cmd.arg("shell");
    }
    // this works even without "shell"?
    cmd.arg(args);

    #[cfg(target_os = "windows")]
    let cmd = cmd.creation_flags(0x0800_0000); // do not open a cmd window

    match cmd.output() {
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
/// See [`adb_cmd`] for details.
pub async fn adb_sh_cmd<S: AsRef<str>>(
    device_serial: S,
    action: String,
    command_type: CommandType,
) -> Result<CommandType, AdbError> {
    let label = match &command_type {
        CommandType::PackageManager(p) => &p.removal,
        CommandType::Shell => "Shell",
    };

    match adb_cmd(true, device_serial.as_ref(), &action) {
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

pub const PACK_URI_SCHEME: &str = "package:";
#[expect(clippy::cast_possible_truncation, reason = "")]
pub const PACK_URI_LEN: u8 = PACK_URI_SCHEME.len() as u8;

/// If `device_serial` is empty, it lets ADB choose the default device.
pub fn hashset_system_packages(
    state: PackageState,
    device_serial: &str,
    user_id: Option<User>,
) -> HashSet<String> {
    let user = user_flag(user_id);
    let action = match state {
        PackageState::Enabled => format!("{PM_LIST_PACKS} -s -e{user}"),
        PackageState::Disabled => format!("{PM_LIST_PACKS} -s -d{user}"),
        _ => return HashSet::default(), // You probably don't need to use this function for anything else
    };

    match adb_cmd(true, device_serial, &action) {
        Ok(s) => s
            .lines()
            // Assume every line has the same prefix
            .map(|ln| String::from(&ln[PACK_URI_LEN as usize..]))
            .collect(),
        _ => HashSet::default(),
    }
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
    adb_cmd(true, serial, "getprop ro.product.model").unwrap_or_else(|err| {
        println!("ERROR: {err}");
        error!("ERROR: {err}");
        if err.contains("adb: no devices/emulators found") {
            "no devices/emulators found".to_string()
        } else {
            err
        }
    })
}

/// Get Android SDK version by querying the
// `ro.build.version.sdk` property or defaulting to 0.
///
/// If `device_serial` is empty, it lets ADB choose the default device.
pub fn get_android_sdk(device_serial: &str) -> u8 {
    adb_cmd(true, device_serial, "getprop ro.build.version.sdk").map_or(0, |sdk| {
        sdk.parse().expect("SDK version numeral must be valid")
    })
}

/// Get the brand by querying the `ro.product.brand` property.
///
/// If `serial` is empty, it lets ADB choose the default device.
pub fn get_device_brand(serial: &str) -> String {
    format!(
        "{} {}",
        adb_cmd(true, serial, "getprop ro.product.brand")
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
///
/// If `device_serial` is empty, it lets ADB choose the default device.
pub fn is_protected_user(user_id: &str, device_serial: &str) -> bool {
    adb_cmd(
        true,
        device_serial,
        &format!("{PM_LIST_PACKS} -s --user {user_id}"),
    )
    .is_err()
}

/// `pm list users` parsed into a vec with extra info.
pub fn list_users_parsed(device_serial: &str) -> Vec<User> {
    #[dynamic]
    static RE: Regex = Regex::new(r"\{([0-9]+)").unwrap_or_else(|_| unreachable!());
    AdbCmd::new()
        .shell(device_serial)
        .pm()
        .list_users()
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
    retry(Fixed::from_millis(500).take(120), || {
        match AdbCmd::new().devices() {
            Ok(devices) => {
                let mut device_list: Vec<Device> = vec![];
                if !DEV_RE.is_match(&devices) {
                    return OperationResult::Retry(vec![]);
                }
                for device in DEV_RE.captures_iter(&devices) {
                    let serial = &device[1];
                    device_list.push(Device {
                        model: get_device_brand(serial),
                        android_sdk: get_android_sdk(serial),
                        user_list: list_users_parsed(serial),
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
        }
    })
    .unwrap_or_default()
}

pub async fn initial_load() -> bool {
    match adb_cmd(false, "", "devices") {
        Ok(_devices) => true,
        Err(_err) => false,
    }
}
