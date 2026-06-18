#![deny(clippy::unwrap_used)]

//! This module is intended to group everything that's "intrinsic" of ADB.
//!
//! Following the design philosophy of most of Rust `std`,
//! `*Command` are intended to be "thin wrappers" (low-overhead abstractions)
//! around the optional `adb_client` backend or the system ADB CLI,
//! which implies:
//! - no "magic"
//! - no custom commands
//! - no chaining ("piping") of existing commands
//!
//! This guarantees a 1-to-1 mapping between methods and cmds,
//! thereby reducing surprises such as:
//! - Non-atomic operations: consider what happens if a pack changes state
//!   in the middle of listing enabled and disabled packs!
//! - Non-standard semantics: what would happen if a new ADB version
//!   supports a feature we already defined,
//!   but has _slightly_ different behavior?
//!
//! Despite being "low-level", we can still "have cake and eat it too";
//! After all, what's the point of an abstraction if it doesn't come with goodies?:
//! We can reserve some artistic license, such as:
//! - pre-parsing or validanting output, to provide types with invariants
//! - strongly-typed rather than "stringly-typed" APIs
//! - nicer IDE support
//! - compile-time prevention of malformed cmds
//! - implicit enforcement of a narrow set of operations
//!
//! About that last point, if there's ever a need for an ADB feature
//! which these APIs don't expose,
//! please, **PLEASE** refrain from falling-back to any `Command`-like API.
//! Rather, please extend these APIs in a consistent way.
//!
//! ## Backend Selection
//!
//! This module supports two ADB backends:
//! - **Builtin** (`adb_client`): Pure Rust implementation, no external dependencies
//! - **System**: Uses the system-installed `adb` binary
//!
//! Use [`ACommand::with_backend`] to select a specific backend,
//! or [`ACommand::new`] to use the default (System backend).
//!
//! Thank you! ❤️
//!
//! For comprehensive info about ADB,
//! [see this](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/master/docs/)

#[cfg(feature = "builtin-adb")]
use adb_client::{ADBDeviceExt, server::ADBServer};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
#[cfg(feature = "builtin-adb")]
use std::io::Cursor;
use std::rc::Rc;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::utils::is_all_w_c;
use log::{error, info};

/// Convert ADB output bytes to a trimmed UTF-8 string.
/// Uses lossy conversion to prevent panics on non-UTF8 output from certain OEMs.
#[must_use]
pub fn to_trimmed_utf8(v: &[u8]) -> String {
    String::from_utf8_lossy(v).trim_end().to_string()
}

/// ADB backend selection.
///
/// - **Builtin**: Uses the `adb_client` crate (pure Rust, no external dependencies)
/// - **System**: Uses the system-installed `adb` binary
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AdbBackend {
    /// Built-in ADB implementation via `adb_client` crate.
    /// The application can communicate with devices without needing `adb` installed.
    #[cfg(feature = "builtin-adb")]
    Builtin,
    /// Uses the system-installed `adb` binary.
    /// This is the default to preserve existing behavior.
    /// Requires `adb` to be available in PATH.
    /// Useful if you prefer using your own ADB installation or need specific ADB features.
    #[default]
    System,
}

impl AdbBackend {
    /// Returns all available backend variants for UI enumeration
    #[cfg(feature = "builtin-adb")]
    pub const ALL: [Self; 2] = [Self::Builtin, Self::System];

    /// Returns all available backend variants for UI enumeration
    #[cfg(not(feature = "builtin-adb"))]
    pub const ALL: [Self; 1] = [Self::System];
}

impl std::fmt::Display for AdbBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "builtin-adb")]
            Self::Builtin => write!(f, "Builtin"),
            Self::System => write!(f, "System (adb)"),
        }
    }
}

#[cfg(debug_assertions)]
#[must_use]
fn is_version_triple(s: &str) -> bool {
    let mut parts = s.split('.');
    let is_digits = |c: &str| !c.is_empty() && c.bytes().all(|b| b.is_ascii_digit());
    parts.next().is_some_and(is_digits)
        && parts.next().is_some_and(is_digits)
        && parts.next().is_some_and(is_digits)
        && parts.next().is_none()
}

#[cfg(debug_assertions)]
fn assert_adb_version_output_format(out: &str) {
    const ADBV: &str = "Android Debug Bridge version ";
    const V: &str = "Version ";

    let mut lns = out.lines();

    assert!(
        lns.next()
            .is_some_and(|ln| ln.starts_with(ADBV) && is_version_triple(&ln[ADBV.len()..]))
    );
    assert!(
        lns.next().is_some_and(|ln| ln.starts_with(V)
            && is_version_triple(&ln[V.len()..ln.find('-').unwrap_or(ln.len())]))
    );
    // missing test for valid path
    assert!(
        lns.next().is_some_and(|ln| ln.starts_with("Installed as ")
            && (ln.ends_with("adb") || ln.ends_with("adb.exe")))
    );
    // missing test for x86/ARM (both 64b)
    assert!(lns.next().is_some_and(|ln| ln.starts_with("Running on ")));
    if lns.next().is_some() {
        unreachable!("Expected < 5 lines")
    }
}

/// Internal state for `ACommand` - tracks the device serial and backend to use
#[derive(Debug)]
struct ACommandState {
    device_serial: Option<String>,
    backend: AdbBackend,
}

/// Builder object for an Android Debug Bridge command,
/// using the type-state and new-type patterns.
///
/// This is not intended to model the entire ADB API.
/// It only models the subset that concerns UADNG.
///
/// [More info here](https://developer.android.com/tools/adb)
#[derive(Debug)]
pub struct ACommand(ACommandState);

impl ACommand {
    /// `adb` command builder with the default backend.
    #[must_use]
    pub fn new() -> Self {
        Self::with_backend(AdbBackend::default())
    }

    /// `adb` command builder with a specific backend
    #[must_use]
    pub fn with_backend(backend: AdbBackend) -> Self {
        Self(ACommandState {
            device_serial: None,
            backend,
        })
    }

    /// `shell` sub-command builder.
    ///
    /// If `device_serial` is empty, it lets ADB choose the default device.
    #[must_use]
    pub fn shell<S: AsRef<str>>(mut self, device_serial: S) -> ShellCommand {
        let serial = device_serial.as_ref();
        if !serial.is_empty() {
            self.0.device_serial = Some(serial.to_string());
        }
        ShellCommand(self)
    }

    /// Header-less list of attached devices (as serials) and their statuses:
    /// - USB
    /// - TCP/IP: WIFI, Ethernet, etc...
    /// - Local emulators
    ///
    /// Status can be (but not limited to):
    /// - "unauthorized"
    /// - "device"
    pub fn devices(self) -> Result<Vec<(String, String)>, String> {
        match self.0.backend {
            #[cfg(feature = "builtin-adb")]
            AdbBackend::Builtin => Self::devices_builtin(),
            AdbBackend::System => Self::devices_system(),
        }
    }

    /// Returns version information from the ADB server/binary.
    ///
    /// ## Builtin backend
    /// Returns the ADB server protocol version:
    /// ```txt
    /// ADB Server Version: 1.0.41
    /// ```
    ///
    /// ## System backend
    /// Returns the full `adb version` output (format may vary by installation):
    /// ```txt
    /// Android Debug Bridge version 1.0.41
    /// Version 35.0.2-android-tools
    /// Installed as /usr/bin/adb
    /// Running on Linux 6.18 (x86_64)
    /// ```
    pub fn version(self) -> Result<String, String> {
        match self.0.backend {
            #[cfg(feature = "builtin-adb")]
            AdbBackend::Builtin => Self::version_builtin(),
            AdbBackend::System => Self::version_system(),
        }
    }

    // ========== Builtin backend implementation (adb_client) ==========

    /// Get ADB server version using the builtin `adb_client`
    #[cfg(feature = "builtin-adb")]
    fn version_builtin() -> Result<String, String> {
        let mut server = ADBServer::default();
        match server.version() {
            Ok(version) => Ok(format!("ADB Server Version: {version}")),
            Err(e) => {
                error!("Failed to get ADB server version: {e}");
                Err(format!("Cannot get ADB server version: {e}"))
            }
        }
    }

    /// List devices using the builtin `adb_client`
    #[cfg(feature = "builtin-adb")]
    fn devices_builtin() -> Result<Vec<(String, String)>, String> {
        let mut server = ADBServer::default();
        server
            .devices()
            .map(|device_list| {
                device_list
                    .into_iter()
                    .map(|dev| (dev.identifier, dev.state.to_string()))
                    .collect()
            })
            .map_err(|e| {
                error!("ADB: {e}");
                format!("Cannot connect to ADB server: {e}")
            })
    }

    /// Execute a shell command via `adb_client` (builtin backend)
    #[cfg(feature = "builtin-adb")]
    fn run_shell_command_builtin(&self, shell_command: &str) -> Result<String, String> {
        let mut server = ADBServer::default();

        // Validate device availability and serial
        if let Some(ref serial) = self.0.device_serial {
            let device_list = server
                .devices()
                .map_err(|e| format!("Cannot get device list: {e}"))?;

            if !device_list.iter().any(|d| d.identifier == *serial) {
                let available = device_list
                    .iter()
                    .map(|d| d.identifier.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!(
                    "Device '{serial}' not found. Available: {available}"
                ));
            }
        }

        let mut device = server
            .get_device()
            .map_err(|e| format!("Cannot connect to device: {e}"))?;

        if shell_command.trim().is_empty() {
            return Err("Empty shell command".into());
        }

        info!("Ran command: adb shell {shell_command}");

        let mut buffer = Vec::new();
        let _exit_code = device
            .shell_command(&shell_command, Some(&mut Cursor::new(&mut buffer)), None)
            .map_err(|e| {
                error!("ADB shell command failed: {e}");
                format!("Shell command failed: {e}")
            })?;

        Ok(String::from_utf8_lossy(&buffer).trim_end().to_string())
    }

    // ========== System backend implementation (adb binary) ==========

    /// Get ADB version using the system `adb` binary
    fn version_system() -> Result<String, String> {
        let mut cmd = std::process::Command::new("adb");
        cmd.arg("version");
        let out = Self::run_system_command(cmd)?;

        #[cfg(debug_assertions)]
        assert_adb_version_output_format(&out);

        Ok(out)
    }

    /// List devices using the system `adb` binary
    fn devices_system() -> Result<Vec<(String, String)>, String> {
        let mut cmd = std::process::Command::new("adb");
        cmd.arg("devices");
        Ok(Self::run_system_command(cmd)?
            .lines()
            .skip(1) // header
            .filter_map(|line| {
                let (serial, status) = line.split_once('\t')?;
                Some((serial.to_string(), status.to_string()))
            })
            .collect())
    }

    /// Execute a shell command via system `adb` binary
    fn run_shell_command_system(&self, shell_command: &str) -> Result<String, String> {
        let mut cmd = std::process::Command::new("adb");

        if let Some(ref serial) = self.0.device_serial {
            cmd.args(["-s", serial]);
        }

        cmd.arg("shell");
        cmd.arg(shell_command);

        info!("Ran command: adb shell {}", shell_command);
        Self::run_system_command(cmd)
    }

    /// General system command executor for adb binary
    fn run_system_command(mut cmd: std::process::Command) -> Result<String, String> {
        #[cfg(target_os = "windows")]
        let cmd = cmd.creation_flags(0x0800_0000); // do not open a cmd window

        info!(
            "Ran command: adb {}",
            cmd.get_args()
                .filter_map(|s| s.to_str())
                .collect::<Vec<_>>()
                .join(" ")
        );

        let output = cmd.output().map_err(|e| {
            error!("ADB: {e}");
            "Cannot run ADB, likely not found".to_string()
        })?;

        let stdout = to_trimmed_utf8(&output.stdout);
        if output.status.success() {
            Ok(stdout)
        } else {
            // ADB sometimes outputs errors to stdout instead of stderr
            Err(if stdout.is_empty() {
                to_trimmed_utf8(&output.stderr)
            } else {
                stdout
            })
        }
    }

    /// Execute a shell command using the configured backend
    fn run_shell_command(&self, shell_command: &str) -> Result<String, String> {
        match self.0.backend {
            #[cfg(feature = "builtin-adb")]
            AdbBackend::Builtin => self.run_shell_command_builtin(shell_command),
            AdbBackend::System => self.run_shell_command_system(shell_command),
        }
    }
}

impl Default for ACommand {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder object for a command that runs on the device's default `sh` implementation.
/// Typically MKSH, but could be Ash.
///
/// [More info](https://chromium.googlesource.com/aosp/platform/system/core/+/refs/heads/upstream/shell_and_utilities).
#[derive(Debug)]
pub struct ShellCommand(ACommand);

impl ShellCommand {
    /// `pm` command builder
    pub fn pm(self) -> PmCommand {
        PmCommand(self)
    }

    /// Query a device property value, by its key.
    /// These can be of any type:
    /// - `boolean`
    /// - `int`
    /// - chars
    /// - etc...
    ///
    /// So to avoid lossy conversions, we return strs
    pub fn getprop(self, key: &str) -> Result<String, String> {
        self.0.run_shell_command(&format!("getprop {key}"))
    }

    /// Reboots device
    pub fn reboot(self) -> Result<String, String> {
        self.0.run_shell_command("reboot")
    }

    /// Execute an arbitrary shell action string on the device's default shell.
    /// The action string is passed as a single argument to `adb shell` and
    /// interpreted by the remote shell (which splits on spaces).
    pub fn raw(self, action: &str) -> Result<String, String> {
        self.0.run_shell_command(action)
    }
}

#[must_use]
pub const fn is_pkg_component(s: &[u8]) -> bool {
    !s.is_empty() && s[0].is_ascii_alphabetic() && (s.len() == 1 || is_all_w_c(s.split_at(1).1))
}

/// String with the invariant of being a valid package-name.
/// See [`PackageId::new`] for validation details.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageId(Rc<str>);
impl PackageId {
    /// Creates a package-ID if it's `"android"` or valid according to:
    /// - <https://developer.android.com/guide/topics/manifest/manifest-element.html#package>
    /// - <https://developer.android.com/build/configure-app-module#set-application-id>
    #[must_use]
    pub fn new(p_id: &str) -> Option<Self> {
        if p_id == "android" {
            return Some(Self(p_id.into()));
        }
        let mut components = p_id.split('.');
        for _ in 0..2 {
            if !components
                .next()
                .is_some_and(|comp| is_pkg_component(comp.as_bytes()))
            {
                return None;
            }
        }
        if components.all(|comp| is_pkg_component(comp.as_bytes())) {
            Some(Self(p_id.into()))
        } else {
            None
        }
    }
}

/// `pm list packages` flag/state/type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmListPacksFlag {
    /// `-u`, not to be confused with `-a`
    IncludeUninstalled,
    /// `-e`
    OnlyEnabled,
    /// `-d`
    OnlyDisabled,
}
impl PmListPacksFlag {
    const fn as_str(self) -> &'static str {
        match self {
            Self::IncludeUninstalled => "-u",
            Self::OnlyEnabled => "-e",
            Self::OnlyDisabled => "-d",
        }
    }
}

impl std::fmt::Display for PmListPacksFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

const PACK_PREFIX: &str = "package:";

pub const PM_CLEAR_PACK: &str = "pm clear";

/// Builder object for an Android Package Manager command.
/// <https://developer.android.com/tools/adb#pm>
#[derive(Debug)]
#[must_use]
pub struct PmCommand(ShellCommand);
impl PmCommand {
    /// `list packages -s` sub-command, [`PACK_PREFIX`] stripped from each element.
    ///
    /// `Ok` variant:
    /// - isn't 100% guaranteed to contain valid pack-IDs
    ///   but you can assume it does (except in `unsafe {}` blocks)
    /// - isn't sorted
    /// - duplicates never _seem_ to happen, but don't assume uniqueness
    pub fn list_packages_sys(
        self,
        flag: Option<PmListPacksFlag>,
        user_id: Option<u16>,
    ) -> Result<Vec<String>, String> {
        let mut command = String::from("pm list packages -s");
        if let Some(f) = flag {
            write!(&mut command, " {f}").ok();
        }
        if let Some(uid) = user_id {
            write!(&mut command, " --user {uid}").ok();
        }

        self.0.raw(&command).map(|output| {
            output
                .lines()
                .filter_map(|line| {
                    let pkg = line.strip_prefix(PACK_PREFIX)?;
                    debug_assert!(PackageId::new(pkg).is_some());
                    Some(pkg.to_string())
                })
                .collect()
        })
    }

    /// `list users` sub-command, deserialized/parsed.
    ///
    /// - <https://source.android.com/docs/devices/admin/multi-user-testing>
    /// - <https://stackoverflow.com/questions/37495126/android-get-list-of-users-and-profile-name>
    pub fn list_users(self) -> Result<Box<[UserInfo]>, String> {
        // Expected shape: "UserInfo{<id>:<name>:<flags>}[ running]"
        // https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/core/java/android/content/pm/UserInfo.java
        Ok(self
            .0
            .raw("pm list users")?
            .lines()
            .skip(1) // omit header
            .filter_map(|line| {
                // Defensive parsing: strip known prefixes/suffixes, extract ID
                let s = line.trim_ascii();
                let s = s.strip_prefix("UserInfo{").unwrap_or(s);
                let s = s.strip_suffix("running").unwrap_or(s).trim_ascii_end();
                let s = s.strip_suffix('}').unwrap_or(s);
                let id = s.split(':').next()?.parse().ok()?;
                Some(UserInfo { id })
            })
            .collect())
    }
}

/// Mirror of AOSP `UserInfo` Java Class, with an extra field
#[derive(Debug, Clone)]
pub struct UserInfo {
    id: u16,
    //name: Box<str>,
    //flags: u32,
    //running: bool,
}
impl UserInfo {
    #[must_use]
    pub const fn get_id(&self) -> u16 {
        self.id
    }
    /*
    /// Check if the user was logged-in at the time `pm list users` was invoked
    #[must_use]
    #[allow(dead_code, reason = "Currently unused by UI; kept for future features")]
    pub const fn was_running(&self) -> bool {
        self.running
    }
    */
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_pack_ids() {
        for p_id in [
            "",
            "   ",
            ".",
            "nodots",
            "com..example",
            "net.hello.",
            "org.0example",
            "org._foobar",
            "the.🎂.is.a.lie",
            "EXCLAMATION!!!!",
        ] {
            assert_eq!(PackageId::new(p_id), None);
        }
    }

    #[test]
    fn valid_pack_ids() {
        for p_id in [
            "A.a",
            "x.X",
            "org.example",
            "net.hello",
            "uwu.owo",
            "Am0Gu5.Zuz",
            "net.net.net.net.net.net.net.net.net.net.net",
            "com.github.w1nst0n",
            "this_.String_.is_.not_.real_",
        ] {
            assert_ne!(PackageId::new(p_id), None);
        }
    }

    #[test]
    fn backend_display() {
        #[cfg(feature = "builtin-adb")]
        assert_eq!(AdbBackend::Builtin.to_string(), "Builtin");
        assert_eq!(AdbBackend::System.to_string(), "System (adb)");
    }

    #[test]
    fn backend_default_is_system() {
        assert_eq!(AdbBackend::default(), AdbBackend::System);
    }
}
