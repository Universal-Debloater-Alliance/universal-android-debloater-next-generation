#![deny(clippy::unwrap_used)]

//! This module is intended to group everything that's "intrinsic" of ADB.
//!
//! Following the design philosophy of most of Rust `std`,
//! `*Command` are intended to be "thin wrappers" (low-overhead abstractions)
//! around the ADB CLI or `adb_client`
//! ([in the future](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/issues/700) ),
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
//! Thank you! ❤️
//!
//! For comprehensive info about ADB,
//! [see this](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/master/docs/)

use std::rc::Rc;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::utils::is_all_w_c;
use log::{error, info, warn};

/// Convert ADB output bytes to a trimmed UTF-8 string.
/// Uses lossy conversion to prevent panics on non-UTF8 output from certain OEMs.
#[must_use]
pub fn to_trimmed_utf8(v: &[u8]) -> String {
    String::from_utf8_lossy(v).trim_end().to_string()
}

pub(crate) fn command_failed(output: &str) -> bool {
    output.lines().any(|line| {
        let line = line.trim();
        if line.eq_ignore_ascii_case("failed") {
            return true;
        }
        [
            "error",
            "failure",
            "exception occurred while executing",
            "security exception",
            "java.lang.SecurityException",
        ]
        .iter()
        .any(|prefix| {
            line.split_at_checked(prefix.len())
                .is_some_and(|(label, rest)| {
                    label.eq_ignore_ascii_case(prefix)
                        && (rest.is_empty() || rest.starts_with([':', '[', ' ', '\t']))
                })
        })
    })
}

/// Android denies the shell access to some secondary users.
#[must_use]
pub fn profile_access_denied(error: &str) -> bool {
    error.contains("Shell does not have permission to access user")
        || (error.contains("SecurityException")
            && error.contains("android.permission.INTERACT_ACROSS_USERS"))
}

#[cfg(debug_assertions)]
#[must_use]
fn is_version_triple(s: &str) -> bool {
    let mut components = s.split('.');
    for _ in 0..3 {
        if !components
            .next()
            .is_some_and(|comp| comp.as_bytes().iter().all(u8::is_ascii_digit))
        {
            return false;
        }
    }
    components.next().is_none()
}

/// Builder object for an Android Debug Bridge CLI command,
/// using the type-state and new-type patterns.
///
/// This is not intended to model the entire ADB API.
/// It only models the subset that concerns UADNG.
///
/// [More info here](https://developer.android.com/tools/adb)
#[derive(Debug)]
pub struct ACommand(std::process::Command);
impl ACommand {
    /// `adb` command builder
    #[must_use]
    pub fn new() -> Self {
        Self(std::process::Command::new("adb"))
    }

    /// `shell` sub-command builder.
    ///
    /// If `device_serial` is empty, it lets ADB choose the default device.
    #[must_use]
    pub fn shell<S: AsRef<str>>(mut self, device_serial: S) -> ShellCommand {
        let serial = device_serial.as_ref();
        if !serial.is_empty() {
            self.0.args(["-s", serial]);
        }
        self.0.arg("shell");
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
    pub fn devices(mut self) -> Result<Vec<(String, String)>, String> {
        self.0.arg("devices");
        Ok(parse_device_list(&self.run()?))
    }

    /// `version` sub-command
    ///
    /// ## Format
    /// This is just a sample,
    /// we don't know which guarantees are stable (yet):
    /// ```txt
    /// Android Debug Bridge version 1.0.41
    /// Version 34.0.5-debian
    /// Installed as /usr/lib/android-sdk/platform-tools/adb
    /// Running on Linux 6.12.12-amd64 (x86_64)
    /// ```
    ///
    /// The expected format should be like:
    /// ```txt
    /// Android Debug Bridge version <num>.<num>.<num>
    /// Version <num>.<num>.<num>-<no spaces>
    /// Installed as <ANDROID_SDK_HOME>/platform-tools/adb[.exe]
    /// Running on <OS/kernel version> (<CPU arch>)
    /// ```
    #[expect(clippy::panic_in_result_fn, reason = "Assertions are fine")]
    pub fn version(mut self) -> Result<String, String> {
        self.0.arg("version");
        let out = self.run()?;

        #[cfg(debug_assertions)]
        {
            const ADBV: &str = "Android Debug Bridge version ";
            const V: &str = "Version ";

            let mut lns = out.lines();

            assert!(
                lns.next()
                    .is_some_and(|ln| ln.starts_with(ADBV) && is_version_triple(&ln[ADBV.len()..]))
            );
            assert!(lns.next().is_some_and(|ln| ln.starts_with(V)
                && is_version_triple(&ln[V.len()..ln.find('-').unwrap_or(ln.len())])));
            // missing test for valid path
            assert!(lns.next().is_some_and(|ln| ln.starts_with("Installed as ")
                && (ln.ends_with("adb") || ln.ends_with("adb.exe"))));
            // missing test for x86/ARM (both 64b)
            assert!(lns.next().is_some_and(|ln| ln.starts_with("Running on ")));
            if lns.next().is_some() {
                unreachable!("Expected < 5 lines")
            }
        }

        Ok(out)
    }

    /// General executor
    fn run(self) -> Result<String, String> {
        let mut cmd = self.0;
        #[cfg(target_os = "windows")]
        let cmd = cmd.creation_flags(0x0800_0000); // do not open a cmd window

        info!(
            "Ran command: adb {}",
            cmd.get_args()
                .map(|s| s.to_str().unwrap_or_else(|| unreachable!()))
                .collect::<Vec<_>>()
                .join(" ")
        );
        match cmd.output() {
            Err(e) => {
                error!("ADB: {e}");
                Err("Cannot run ADB, likely not found".to_string())
            }
            Ok(o) => {
                let stdout = to_trimmed_utf8(&o.stdout);
                let stderr = to_trimmed_utf8(&o.stderr);
                if command_failed(&stderr) || profile_access_denied(&stderr) {
                    return Err(stderr);
                }
                if o.status.success() {
                    Ok(stdout)
                } else {
                    // ADB does really weird things:
                    // Some errors are not redirected to `stderr`
                    let err = if stdout.is_empty() { stderr } else { stdout };
                    Err(err)
                }
            }
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
    pub fn pm(mut self) -> PmCommand {
        self.0.0.arg("pm");
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
    pub fn getprop(mut self, key: &str) -> Result<String, String> {
        self.0.0.args(["getprop", key]);
        self.0.run()
    }
    /// Reboots device
    pub fn reboot(mut self) -> Result<String, String> {
        self.0.0.arg("reboot");
        self.0.run()
    }

    /// Execute an arbitrary shell action string on the device's default shell.
    /// The action string is passed as a single argument to `adb shell` and
    /// interpreted by the remote shell (which splits on spaces).
    pub fn raw(mut self, action: &str) -> Result<String, String> {
        self.0.0.arg(action);
        self.0.run()
    }
}

#[must_use]
pub const fn is_pkg_component(s: &[u8]) -> bool {
    if s.is_empty() {
        return false;
    }
    s[0].is_ascii_alphabetic()
        && if s.len() > 1 {
            is_all_w_c(s.split_at(1).1)
        } else {
            true
        }
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
    // is there a trait for this?
    fn to_str(self) -> &'static str {
        match self {
            Self::IncludeUninstalled => "-u",
            Self::OnlyEnabled => "-e",
            Self::OnlyDisabled => "-d",
        }
    }
}
#[expect(clippy::to_string_trait_impl, reason = "This is not user-facing")]
impl ToString for PmListPacksFlag {
    fn to_string(&self) -> String {
        self.to_str().to_string()
    }
}

const PACK_PREFIX: &str = "package:";

fn parse_package_list(pack_ls: &str) -> Result<Vec<String>, String> {
    let mut packages = Vec::new();
    let mut remaining = pack_ls;
    while !remaining.is_empty() {
        let (line, rest) = remaining.split_once('\n').unwrap_or((remaining, ""));
        if line.trim().is_empty() {
            remaining = rest;
            continue;
        }
        let Some(name) = line.strip_prefix(PACK_PREFIX) else {
            return Err(remaining.trim_end_matches(['\r', '\n']).to_owned());
        };
        remaining = rest;
        let name = name.trim_end_matches('\r');
        if PackageId::new(name).is_some() {
            packages.push(name.to_owned());
        } else {
            warn!("skipping nonstandard package name: {name:?}");
        }
    }
    Ok(packages)
}

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
        mut self,
        f: Option<PmListPacksFlag>,
        user_id: Option<u16>,
    ) -> Result<Vec<String>, String> {
        let cmd = &mut self.0.0.0;

        cmd.args(["list", "packages", "-s"]);
        if let Some(s) = f {
            cmd.arg(s.to_str());
        }
        if let Some(u) = user_id {
            cmd.arg("--user");
            cmd.arg(u.to_string());
        }

        self.0
            .0
            .run()
            .and_then(|pack_ls| parse_package_list(&pack_ls))
    }

    /// `list users` sub-command, deserialized/parsed.
    ///
    /// - <https://source.android.com/docs/devices/admin/multi-user-testing>
    /// - <https://stackoverflow.com/questions/37495126/android-get-list-of-users-and-profile-name>
    pub fn list_users(mut self) -> Result<Box<[UserInfo]>, String> {
        self.0.0.0.args(["list", "users"]);
        parse_user_list(&self.0.0.run()?)
    }
}

fn parse_device_list(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|line| {
            // ADB separates serial and status with a tab, including when piped.
            // Statuses such as "no permissions" contain spaces.
            let (serial, status) = line.split_once('\t')?;
            let (serial, status) = (serial.trim(), status.trim());
            (!serial.is_empty() && !status.is_empty())
                .then(|| (serial.to_string(), status.to_string()))
        })
        .collect()
}

// https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/core/java/android/content/pm/UserInfo.java
// Expected shape: "UserInfo{<id>:<name>:<flags>}[ running]".
// The format may differ on old Android versions or change in future versions.
fn parse_user_list(output: &str) -> Result<Box<[UserInfo]>, String> {
    output
        .lines()
        .map(str::trim_ascii)
        .filter(|line| !line.is_empty() && *line != "Users:")
        .map(|line| {
            // this could be optimized by making more API-stability assumptions
            let info = line
                .strip_prefix("UserInfo")
                .unwrap_or(line)
                .trim_ascii_start();
            let info = info.strip_prefix('{').unwrap_or(info).trim_ascii();
            let info = info
                .strip_suffix("running")
                .unwrap_or(info)
                .trim_ascii_end();
            let info = info.strip_suffix('}').unwrap_or(info).trim_ascii_end();
            let id = info
                .split(':')
                .next()
                .unwrap_or_default()
                .parse()
                .map_err(|e| {
                    if line.starts_with("UserInfo") || line.starts_with('{') {
                        format!("Invalid user ID in ADB output {line:?}: {e}")
                    } else {
                        // pm can print a multiline diagnostic on stdout and exit successfully.
                        output.to_owned()
                    }
                })?;
            Ok(UserInfo { id })
        })
        .collect()
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
    fn device_list_tolerates_notices_and_malformed_lines() {
        let output = "* daemon started successfully\r\nList of devices attached\r\nUSB123\tdevice\r\n192.168.1.5:5555\tunauthorized\r\nserial\tno permissions (missing udev rules)\ninvalid\n\tdevice\nmissing-status\t\n";
        assert_eq!(
            parse_device_list(output),
            vec![
                ("USB123".to_string(), "device".to_string()),
                ("192.168.1.5:5555".to_string(), "unauthorized".to_string()),
                (
                    "serial".to_string(),
                    "no permissions (missing udev rules)".to_string()
                ),
            ]
        );
        assert_eq!(parse_device_list(""), Vec::<(String, String)>::new());
    }

    #[test]
    fn user_list_preserves_format_tolerance() {
        let users = parse_user_list(
            "Users:\r\n\tUserInfo{0:Owner:c13} running\r\n\r\n\tUserInfo{10:Work: profile:30}\r\n\tUserInfo {11:Vendor:unknown-flags} vendor-status\r\n\tUserInfo{12} running\r\n \r\n",
        )
        .expect("valid user list");
        assert_eq!(
            users.iter().map(UserInfo::get_id).collect::<Vec<_>>(),
            [0, 10, 11, 12]
        );
    }

    #[test]
    fn headerless_user_list_keeps_the_owner() {
        let users = parse_user_list("UserInfo{0:Owner:13} running\nUserInfo{10:Work:31}")
            .expect("valid user list");
        assert_eq!(
            users.iter().map(UserInfo::get_id).collect::<Vec<_>>(),
            [0, 10]
        );
    }

    #[test]
    fn malformed_user_lists_return_an_error_instead_of_a_partial_list() {
        for line in [
            "UserInfo{bad:Owner:13}",
            "UserInfo{65536:Owner:13}",
            "unexpected output",
        ] {
            assert!(parse_user_list(&format!("Users:\nUserInfo{{0:Owner:13}}\n{line}")).is_err());
        }
    }

    #[test]
    fn user_list_keeps_multiline_diagnostics() {
        let diagnostic = "ERROR: user service offline\n  detail: binder unavailable";
        assert_eq!(
            parse_user_list(diagnostic).expect_err("ADB diagnostic"),
            diagnostic
        );
    }

    #[test]
    fn package_list_lf_and_crlf() {
        for newline in ["\n", "\r\n"] {
            let output =
                format!("package:android{newline}package:com.android.MtpApplication{newline}");
            assert_eq!(
                parse_package_list(&output).expect("valid package list"),
                ["android", "com.android.MtpApplication"]
            );
        }
    }

    #[test]
    fn package_list_extra_carriage_returns() {
        let output = "package:android\r\r\npackage:com.android.MtpApplication\r\r\n";
        assert_eq!(
            parse_package_list(&to_trimmed_utf8(output.as_bytes())).expect("valid package list"),
            ["android", "com.android.MtpApplication"]
        );
    }

    #[test]
    fn package_list_without_final_newline() {
        for output in ["package:com.example", "package:com.example\r"] {
            assert_eq!(
                parse_package_list(output).expect("valid package list"),
                ["com.example"]
            );
        }
        assert_eq!(
            parse_package_list("").expect("empty package list"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn package_list_rejects_invalid_ids() {
        let output = concat!(
            "package:com..example\n",
            "package:com.example withspace\n",
            "package:com.example \n",
            "package:com.example\t\n",
            "package:com.example\u{a0}\n",
            "package:com.example\n",
        );
        assert_eq!(
            parse_package_list(output).expect("package lines"),
            ["com.example"]
        );
    }

    #[test]
    fn package_list_propagates_a_diagnostic_after_valid_rows() {
        let diagnostic = "Exception occurred while executing 'list':\r\n  java.lang.SecurityException: Shell does not have permission to access user 10";
        let output = format!("package:android\r\n\r\n{diagnostic}\r\n");
        assert_eq!(
            parse_package_list(&output).expect_err("ADB diagnostic"),
            diagnostic
        );
    }

    #[test]
    fn command_failures_are_recognized_by_their_label() {
        for reply in [
            "ERROR: refused",
            "some output\n  error: refused",
            "Failure [restricted]",
            "Error type 3",
            "  Failed\r\n",
            "FAILED",
        ] {
            assert!(command_failed(reply), "{reply}");
        }
        assert!(!command_failed(
            "Package com.sec.app.RilErrorNotifier new state: disabled-user"
        ));
        assert!(!command_failed("FailureNotifier started"));
        assert!(!command_failed("Failed to match that name"));
    }

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
}
