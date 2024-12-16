//! This module is intended to group everything that's "intrinsic" of ADB.
//!
//! Following the design philosophy of `Vec` and `thread`,
//! `*Cmd` are intended to be thin wrappers ("low-level" abstractions)
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
//! - shorter names, complemented by context
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
use static_init::dynamic;
use std::{collections::HashSet, process::Command};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub fn to_trimmed_utf8(v: Vec<u8>) -> String {
    String::from_utf8(v)
        .expect("ADB should always output valid ASCII (or UTF-8, at least)")
        .trim_end()
        .to_string()
}

/// Builder for an ADB CLI command,
/// using the type-state and new-type patterns.
///
/// This is not intended to model the entire ADB API.
/// It only models the subset that concerns UADNG.
///
/// [More info here](https://developer.android.com/tools/adb)
#[derive(Debug)]
pub struct Cmd(Command);
impl Cmd {
    pub fn new() -> Self {
        Command::new("adb").arg("");
        Self(Command::new("adb"))
    }
    /// `shell` sub-command.
    ///
    /// If `device_serial` is empty, it lets ADB choose the default device.
    pub fn sh<S: AsRef<str>>(mut self, device_serial: S) -> ShCmd {
        let serial = device_serial.as_ref();
        if !serial.is_empty() {
            self.0.args(["-s", serial]);
        }
        self.0.arg("shell");
        ShCmd(self)
    }
    /// List attached devices (as serials) and their status:
    /// - USB
    /// - TCP/IP: WIFI, Ethernet, etc...
    /// - Local emulators
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
                    .find('\t')
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
    /// Reboots default device
    pub fn reboot(mut self) -> Result<String, String> {
        self.0.arg("reboot");
        self.run()
    }
    fn run(self) -> Result<String, String> {
        let mut cmd = self.0;
        #[cfg(target_os = "windows")]
        let cmd = cmd.creation_flags(0x0800_0000); // do not open a cmd window

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

/// Builder for a command that runs on the device's default `sh` implementation.
/// Typically MKSH, but could be Ash.
///
/// [More info](https://chromium.googlesource.com/aosp/platform/system/core/+/refs/heads/upstream/shell_and_utilities).
#[derive(Debug)]
pub struct ShCmd(Cmd);
impl ShCmd {
    pub fn pm(mut self) -> PmCmd {
        self.0 .0.arg("pm");
        PmCmd(self)
    }
    /// Reboots device
    pub fn reboot(mut self) -> Result<String, String> {
        self.0 .0.arg("reboot");
        self.0.run()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Hash)]
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

const PACK_PREFIX: &str = "package:";

pub const PM_CLEAR_PACK: &str = "pm clear";

/// Builder for an Android Package Manager command.
///
/// [More info](https://developer.android.com/tools/adb#pm)
#[derive(Debug)]
pub struct PmCmd(ShCmd);
impl PmCmd {
    /// `list packages` sub-command, stripped of [`PACK_PREFIX`].
    /// This is "the rawest" version (minimal overhead).
    ///
    /// `Ok` variant:
    /// - isn't sorted
    /// - duplicates never _seem_ to happen, but don't assume uniqueness
    ///
    /// See also [`ls_packs_valid`].
    pub fn ls_packs(
        mut self,
        f: Option<PmLsPackFlag>,
        user_id: Option<u16>,
    ) -> Result<Vec<String>, String> {
        let cmd = &mut self.0 .0 .0;
        cmd.args(["list", "packages", "-s"]);
        if let Some(s) = f {
            cmd.arg(s.to_str());
        };
        if let Some(u) = user_id {
            cmd.arg("--user");
            cmd.arg(u.to_string());
        };
        self.0 .0.run().map(|pack_ls| {
            pack_ls
                .lines()
                .map(|p_ln| {
                    debug_assert!(p_ln.starts_with(PACK_PREFIX));
                    String::from(&p_ln[PACK_PREFIX.len()..])
                })
                .collect()
        })
    }
    /// `list packages` sub-command, pre-validated.
    /// This is strongly-typed, at the cost of regex & hash overhead.
    ///
    /// See also [`ls_packs`].
    pub fn ls_packs_valid(
        self,
        f: Option<PmLsPackFlag>,
        user_id: Option<u16>,
    ) -> Result<HashSet<PackId>, String> {
        Ok(self.ls_packs(f, user_id)?
            .into_iter()
            .map(|p| PackId::new(p).expect("One of these is wrong: `PackId` regex, ADB implementation. Or the spec now allows a wider char-set")).collect())
    }
    #[allow(clippy::doc_markdown, reason = "Multi URL")]
    /// `list users` sub-command.
    /// - https://source.android.com/docs/devices/admin/multi-user-testing
    /// - https://stackoverflow.com/questions/37495126/android-get-list-of-users-and-profile-name
    pub fn ls_users(mut self) -> Result<Vec<String>, String> {
        self.0 .0 .0.args(["list", "users"]);
        Ok(self.0 .0.run()?.lines().skip(1).map(String::from).collect())
    }
}
