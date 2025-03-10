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

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub fn to_trimmed_utf8(v: Vec<u8>) -> String {
    String::from_utf8(v)
        .expect("ADB should always output valid ASCII (or UTF-8, at least)")
        .trim_end()
        .to_string()
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
        Ok(self
            .run()?
            .lines()
            .skip(1) // header
            .map(|dev_stat| {
                let tab_idx = dev_stat
                    // OS-specific?
                    .find('\t')
                    // True on Linux,
                    // no matter if ADB is piped or connected to terminal
                    .expect("There must be 1 tab after serial");
                (
                    // serial
                    dev_stat[..tab_idx].to_string(),
                    // status
                    dev_stat[(tab_idx + 1)..].to_string(),
                )
            })
            .collect())
    }

    /// `version` sub-command, splitted by lines
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
    pub fn version(mut self) -> Result<Vec<String>, String> {
        #[cfg(debug_assertions)]
        static TRIPLE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"^Android Debug Bridge version \d+.\d+.\d+$")
                .unwrap_or_else(|_| unreachable!())
        });
        #[cfg(debug_assertions)]
        static DISTRO: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"^Version \d+.\d+.\d+-\S+$").unwrap_or_else(|_| unreachable!())
        });

        self.0.arg("version");
        Ok(self
            .run()?
            .lines()
            .enumerate()
            // typically 5 allocs.
            // ideally 0, if we didn't use `lines`.
            .map(|(i, ln)| {
                // DO NOT "REFACTOR" TO `debug_assert`!
                // it's not the same!
                #[cfg(debug_assertions)]
                assert!(match i {
                    0 => TRIPLE.is_match(ln),
                    1 => DISTRO.is_match(ln),
                    2 =>
                    // missing test for valid path
                        ln.starts_with("Installed as ")
                            && (ln.ends_with("adb") || ln.ends_with("adb.exe")),
                    // missing test for x86/ARM (both 64b)
                    3 => ln.starts_with("Running on "),
                    _ => unreachable!("Expected < 5 lines"),
                });
                ln.to_string()
            })
            .collect())
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
                error!("ADB: {}", e);
                Err("Cannot run ADB, likely not found".to_string())
            }
            Ok(o) => {
                let stdout = to_trimmed_utf8(o.stdout);
                if o.status.success() {
                    Ok(stdout)
                } else {
                    let stderr = to_trimmed_utf8(o.stderr);
                    // ADB does really weird things:
                    // Some errors are not redirected to `stderr`
                    let err = if stdout.is_empty() { stderr } else { stdout };
                    Err(err)
                }
            }
        }
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
}

/// String with the invariant of being a valid package-name.
/// See its `new` constructor for more info.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
pub struct PackageId(Box<str>);
impl PackageId {
    /// Creates a package-ID if it's valid according to
    /// [this](https://developer.android.com/build/configure-app-module#set-application-id)
    pub fn new(p_id: Box<str>) -> Option<Self> {
        static RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"^[a-zA-Z][a-zA-Z0-9_]*(?:\.[a-zA-Z][a-zA-Z0-9_]*)+$")
                .unwrap_or_else(|_| unreachable!())
        });

        if RE.is_match(p_id.as_ref()) {
            Some(Self(p_id))
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

pub const PM_CLEAR_PACK: &str = "pm clear";

/// Builder object for an Android Package Manager command.
///
/// [More info](https://developer.android.com/tools/adb#pm)
#[derive(Debug)]
pub struct PmCommand(ShellCommand);
impl PmCommand {
    /// `list packages -s` sub-command, [`PACK_PREFIX`] stripped.
    ///
    /// `Ok` variant:
    /// - isn't guaranteed to contain valid pack-IDs,
    ///   as "android" can be printed but it's invalid
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
        };
        if let Some(u) = user_id {
            cmd.arg("--user");
            cmd.arg(u.to_string());
        };

        self.0.0.run().map(|pack_ls| {
            pack_ls
                .lines()
                .map(|p_ln| {
                    debug_assert!(p_ln.starts_with(PACK_PREFIX));
                    String::from(&p_ln[PACK_PREFIX.len()..])
                })
                .collect()
        })
    }

    /// `list users` sub-command.
    /// Output isn't parsed, because
    /// we don't know if the format is stable across Android versions.
    ///
    /// - <https://source.android.com/docs/devices/admin/multi-user-testing>
    /// - <https://stackoverflow.com/questions/37495126/android-get-list-of-users-and-profile-name>
    pub fn list_users(mut self) -> Result<String, String> {
        self.0.0.0.args(["list", "users"]);
        self.0.0.run()
    }
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
            assert_eq!(PackageId::new(p_id.into()), None);
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
            assert_ne!(PackageId::new(p_id.into()), None);
        }
    }
}
