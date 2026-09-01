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
//! - **Builtin** (`adb_client`): Embedded ADB over USB
//! - **System**: Uses the system-installed `adb` binary
//!
//! Select the process-wide backend with [`AdbBackend::set_current`], then use
//! [`ACommand::new`].
//!
//! Thank you! ❤️
//!
//! For comprehensive info about ADB,
//! [see this](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/master/docs/)

#[cfg(feature = "builtin-adb")]
use adb_client::{
    ADBDeviceExt, RebootType,
    usb::{ADBUSBDevice, USBTransport},
};
#[cfg(feature = "builtin-adb")]
use rsa::{
    RsaPrivateKey,
    pkcs1::DecodeRsaPrivateKey,
    pkcs8::{DecodePrivateKey, EncodePrivateKey, LineEnding},
    rand_core::OsRng,
};
#[cfg(feature = "builtin-adb")]
use rusb::{Context, Device, DeviceDescriptor, UsbContext, constants::LIBUSB_CLASS_VENDOR_SPEC};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
#[cfg(feature = "builtin-adb")]
use std::io::Write as _;
#[cfg(feature = "builtin-adb")]
use std::path::PathBuf;
use std::rc::Rc;
#[cfg(feature = "builtin-adb")]
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
#[cfg(feature = "builtin-adb")]
use tempfile::NamedTempFile;

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

/// ADB backend selection.
///
/// - **Builtin**: Connects directly to physical USB devices via `adb_client`
/// - **System**: Uses the system-installed `adb` binary
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AdbBackend {
    /// Built-in direct-USB ADB implementation via `adb_client`.
    /// The application can communicate with USB devices without an ADB server or binary.
    #[cfg(feature = "builtin-adb")]
    Builtin,
    /// Uses the system-installed `adb` binary.
    /// This is the default to preserve existing behavior.
    /// Requires `adb` to be available in PATH.
    /// Useful if you prefer using your own ADB installation or need specific ADB features.
    #[default]
    System,
}

/// Connection state reported for an ADB device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdbDeviceStatus {
    Device,
    Busy,
    Unauthorized,
    NoPermissions,
    Offline,
    BackendError(String),
    Other(String),
}

impl AdbDeviceStatus {
    fn from_system(status: &str) -> Self {
        let status = status.trim();
        if status == "device" {
            Self::Device
        } else if status == "unauthorized" {
            Self::Unauthorized
        } else if status == "offline" {
            Self::Offline
        } else if status == "no-permissions" || status.starts_with("no permissions") {
            Self::NoPermissions
        } else {
            Self::Other(status.to_string())
        }
    }
}

static CURRENT_ADB_BACKEND: AtomicU8 = AtomicU8::new(0);
#[cfg(feature = "builtin-adb")]
static BUILTIN_USB_DEVICES: Mutex<Vec<CachedBuiltinDevice>> = Mutex::new(Vec::new());

#[cfg(feature = "builtin-adb")]
#[derive(Debug)]
struct CachedBuiltinDevice {
    identifier: String,
    location: UsbDeviceLocation,
    device: ADBUSBDevice,
}

#[cfg(feature = "builtin-adb")]
fn with_builtin_usb_devices<T>(
    operation: impl FnOnce(&mut Vec<CachedBuiltinDevice>) -> Result<T, String>,
) -> Result<T, String> {
    let mut devices = BUILTIN_USB_DEVICES.lock().unwrap_or_else(|poisoned| {
        warn!("Recovering poisoned Builtin ADB USB lock");
        poisoned.into_inner()
    });
    operation(&mut devices)
}

#[cfg(feature = "builtin-adb")]
fn clear_builtin_usb_devices() {
    if let Err(poisoned) = BUILTIN_USB_DEVICES
        .lock()
        .map(|mut devices| devices.clear())
    {
        warn!("Recovering poisoned Builtin ADB USB lock while clearing cached devices");
        poisoned.into_inner().clear();
    }
}

impl AdbBackend {
    /// Returns all available backend variants for UI enumeration
    #[cfg(feature = "builtin-adb")]
    pub const ALL: [Self; 2] = [Self::Builtin, Self::System];

    /// Returns all available backend variants for UI enumeration
    #[cfg(not(feature = "builtin-adb"))]
    pub const ALL: [Self; 1] = [Self::System];

    /// Select the process-wide ADB backend.
    pub fn set_current(self) {
        let value = match self {
            Self::System => 0,
            #[cfg(feature = "builtin-adb")]
            Self::Builtin => 1,
        };
        #[cfg(feature = "builtin-adb")]
        {
            let previous = CURRENT_ADB_BACKEND.swap(value, Ordering::Relaxed);
            if previous == 1 && value != 1 {
                clear_builtin_usb_devices();
            }
        }
        #[cfg(not(feature = "builtin-adb"))]
        CURRENT_ADB_BACKEND.store(value, Ordering::Relaxed);
    }

    /// Return the process-wide ADB backend.
    #[must_use]
    pub fn current() -> Self {
        match CURRENT_ADB_BACKEND.load(Ordering::Relaxed) {
            #[cfg(feature = "builtin-adb")]
            1 => Self::Builtin,
            _ => Self::System,
        }
    }
}

impl std::fmt::Display for AdbBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "builtin-adb")]
            Self::Builtin => write!(f, "Builtin (direct USB)"),
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

#[cfg(feature = "builtin-adb")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UsbDeviceLocation {
    bus: u8,
    address: u8,
}

#[cfg(feature = "builtin-adb")]
#[derive(Debug)]
struct UsbDeviceCandidate {
    identifier: String,
    location: UsbDeviceLocation,
    device: Device<Context>,
}

#[cfg(feature = "builtin-adb")]
fn is_same_usb_device(
    cached_identifier: &str,
    cached_location: UsbDeviceLocation,
    candidate_identifier: &str,
    candidate_location: UsbDeviceLocation,
) -> bool {
    cached_identifier == candidate_identifier && cached_location == candidate_location
}

#[cfg(feature = "builtin-adb")]
fn retain_cached_device_for_candidate(
    cached_identifier: &str,
    cached_location: UsbDeviceLocation,
    candidate_identifier: &str,
    candidate_location: UsbDeviceLocation,
) -> bool {
    cached_identifier != candidate_identifier
        || is_same_usb_device(
            cached_identifier,
            cached_location,
            candidate_identifier,
            candidate_location,
        )
}

#[cfg(feature = "builtin-adb")]
const fn is_supported_adb_interface(class: u8, subclass: u8, protocol: u8) -> bool {
    const ADB_SUBCLASS: u8 = 0x42;
    const ADB_PROTOCOL: u8 = 0x01;

    class == LIBUSB_CLASS_VENDOR_SPEC && subclass == ADB_SUBCLASS && protocol == ADB_PROTOCOL
}

#[cfg(feature = "builtin-adb")]
fn is_adb_usb_device(device: &Device<Context>, descriptor: &DeviceDescriptor) -> bool {
    (0..descriptor.num_configurations()).any(|index| {
        device.config_descriptor(index).is_ok_and(|configuration| {
            configuration.interfaces().any(|interface| {
                interface.descriptors().any(|interface_descriptor| {
                    is_supported_adb_interface(
                        interface_descriptor.class_code(),
                        interface_descriptor.sub_class_code(),
                        interface_descriptor.protocol_code(),
                    )
                })
            })
        })
    })
}

#[cfg(feature = "builtin-adb")]
fn enumerate_usb_devices() -> Result<Vec<UsbDeviceCandidate>, String> {
    let context = Context::new().map_err(|e| format!("Cannot initialize USB: {e}"))?;
    let devices = context
        .devices()
        .map_err(|e| format!("Cannot enumerate USB devices: {e}"))?;
    let mut candidates = Vec::new();

    for device in devices.iter() {
        let Ok(descriptor) = device.device_descriptor() else {
            continue;
        };
        if !is_adb_usb_device(&device, &descriptor) {
            continue;
        }

        let location = UsbDeviceLocation {
            bus: device.bus_number(),
            address: device.address(),
        };
        let identifier = device
            .open()
            .ok()
            .and_then(|handle| {
                handle
                    .read_serial_number_string_ascii(&descriptor)
                    .ok()
                    .filter(|serial| !serial.is_empty())
            })
            .unwrap_or_else(|| format!("usb:{:03}:{:03}", location.bus, location.address));

        candidates.push(UsbDeviceCandidate {
            identifier,
            location,
            device,
        });
    }

    Ok(candidates)
}

#[cfg(feature = "builtin-adb")]
fn resolve_adb_key_path(
    android_user_home: Option<PathBuf>,
    home_directory: Option<PathBuf>,
) -> Option<PathBuf> {
    android_user_home
        .map(|path| path.join("adbkey"))
        .or_else(|| home_directory.map(|home| home.join(".android").join("adbkey")))
}

#[cfg(feature = "builtin-adb")]
fn write_adb_private_key(key_path: &std::path::Path, pem: &str) -> Result<(), String> {
    let key_directory = key_path
        .parent()
        .ok_or_else(|| format!("Invalid ADB key path: {}", key_path.display()))?;
    let mut temporary_key = NamedTempFile::new_in(key_directory).map_err(|e| {
        format!(
            "Cannot create temporary ADB key in {}: {e}",
            key_directory.display()
        )
    })?;
    temporary_key
        .write_all(pem.as_bytes())
        .and_then(|()| temporary_key.as_file().sync_all())
        .map_err(|e| format!("Cannot save {}: {e}", key_path.display()))?;

    match temporary_key.into_temp_path().persist_noclobber(key_path) {
        Ok(()) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(format!(
            "Cannot create {}: {}",
            key_path.display(),
            error.error
        )),
    }
}

#[cfg(feature = "builtin-adb")]
struct PreparedAdbPrivateKey {
    path: PathBuf,
    _temporary_file: Option<NamedTempFile>,
}

#[cfg(feature = "builtin-adb")]
impl PreparedAdbPrivateKey {
    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(feature = "builtin-adb")]
fn prepare_existing_adb_private_key(
    key_path: &std::path::Path,
) -> Result<PreparedAdbPrivateKey, String> {
    let key_contents = std::fs::read_to_string(key_path)
        .map_err(|e| format!("Cannot read ADB key {}: {e}", key_path.display()))?;

    if RsaPrivateKey::from_pkcs8_pem(&key_contents).is_ok() {
        return Ok(PreparedAdbPrivateKey {
            path: key_path.to_path_buf(),
            _temporary_file: None,
        });
    }

    let key = RsaPrivateKey::from_pkcs1_pem(&key_contents).map_err(|_| {
        format!(
            "ADB key {} is not a valid unencrypted PKCS#8 or PKCS#1 RSA private key",
            key_path.display()
        )
    })?;
    let pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| format!("Cannot convert {} to PKCS#8: {e}", key_path.display()))?;
    let mut temporary_file = NamedTempFile::new()
        .map_err(|e| format!("Cannot create temporary converted ADB key: {e}"))?;
    temporary_file
        .write_all(pem.as_bytes())
        .and_then(|()| temporary_file.as_file().sync_all())
        .map_err(|e| format!("Cannot prepare converted ADB key: {e}"))?;

    Ok(PreparedAdbPrivateKey {
        path: temporary_file.path().to_path_buf(),
        _temporary_file: Some(temporary_file),
    })
}

#[cfg(feature = "builtin-adb")]
fn ensure_adb_private_key() -> Result<PreparedAdbPrivateKey, String> {
    let key_path = resolve_adb_key_path(
        std::env::var_os("ANDROID_USER_HOME").map(PathBuf::from),
        dirs::home_dir(),
    )
    .ok_or_else(|| "Cannot determine home directory for the ADB key".to_string())?;
    if key_path.is_file() {
        return prepare_existing_adb_private_key(&key_path);
    }

    let key_directory = key_path
        .parent()
        .ok_or_else(|| format!("Invalid ADB key path: {}", key_path.display()))?;
    std::fs::create_dir_all(key_directory)
        .map_err(|e| format!("Cannot create {}: {e}", key_directory.display()))?;
    let key = RsaPrivateKey::new(&mut OsRng, 2048)
        .map_err(|e| format!("Cannot generate ADB authentication key: {e}"))?;
    let pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| format!("Cannot encode ADB authentication key: {e}"))?;
    write_adb_private_key(&key_path, pem.as_ref())?;

    // Validate the file that won a possible concurrent persist_noclobber race.
    prepare_existing_adb_private_key(&key_path)
}

#[cfg(feature = "builtin-adb")]
fn select_usb_device_index(available: &[String], requested: Option<&str>) -> Result<usize, String> {
    if let Some(identifier) = requested {
        return available
            .iter()
            .position(|candidate| candidate == identifier)
            .ok_or_else(|| format!("USB device '{identifier}' not found"));
    }

    match available {
        [] => Err("No ADB-capable USB devices found".to_string()),
        [_] => Ok(0),
        _ => Err(format!(
            "Multiple USB devices found; select one explicitly: {}",
            available.join(", ")
        )),
    }
}

#[cfg(feature = "builtin-adb")]
const BUILTIN_EXIT_MARKER: &str = "__UAD_EXIT_STATUS__:";

#[cfg(feature = "builtin-adb")]
fn wrap_builtin_shell_command(command: &str) -> String {
    format!("{command}\n__uad_exit=$?\nprintf '\\n{BUILTIN_EXIT_MARKER}%s\\n' \"$__uad_exit\"")
}

#[cfg(feature = "builtin-adb")]
fn finish_builtin_shell_command(stdout: &[u8], stderr: &[u8]) -> Result<String, String> {
    let stdout = to_trimmed_utf8(stdout);
    let stderr = to_trimmed_utf8(stderr);

    let marker_index = stdout.rfind(BUILTIN_EXIT_MARKER).ok_or_else(|| {
        "Direct USB shell command ended without reporting its exit status".to_string()
    })?;
    let status_text = stdout[marker_index + BUILTIN_EXIT_MARKER.len()..].trim();
    let exit_code = status_text.parse::<u8>().map_err(|_| {
        format!("Direct USB shell command returned an invalid exit status: {status_text:?}")
    })?;
    let command_output = stdout[..marker_index].trim_end();
    let detail = match (command_output.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{command_output}\n{stderr}"),
        (false, true) => command_output.to_string(),
        (true, false) => stderr,
        (true, true) => String::new(),
    };

    if exit_code != 0 {
        return Err(format!(
            "Direct USB shell command exited with status {exit_code}: {detail}"
        ));
    }

    Ok(detail)
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
    /// `adb` command builder using the process-wide selected backend.
    #[must_use]
    pub fn new() -> Self {
        Self(ACommandState {
            device_serial: None,
            backend: AdbBackend::current(),
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
    pub fn devices(self) -> Result<Vec<(String, AdbDeviceStatus)>, String> {
        match self.0.backend {
            #[cfg(feature = "builtin-adb")]
            AdbBackend::Builtin => Self::devices_builtin(),
            AdbBackend::System => Self::devices_system(),
        }
    }

    /// Returns version information from the ADB server/binary.
    ///
    /// ## Builtin backend
    /// Returns the embedded transport implementation:
    /// ```txt
    /// adb_client (direct USB)
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
            AdbBackend::Builtin => Ok(Self::version_builtin()),
            AdbBackend::System => Self::version_system(),
        }
    }

    /// Stop the system ADB server so a direct-USB backend can claim the device.
    pub fn kill_system_server() -> Result<String, String> {
        let mut cmd = std::process::Command::new("adb");
        cmd.arg("kill-server");
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x0800_0000); // do not open a cmd window

        let output = match cmd.output() {
            Ok(output) => output,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok("System ADB is not installed; no server needed stopping".to_string());
            }
            Err(err) => return Err(format!("Cannot run `adb kill-server`: {err}")),
        };
        let stdout = to_trimmed_utf8(&output.stdout);
        let stderr = to_trimmed_utf8(&output.stderr);
        if output.status.success() {
            Ok(stdout)
        } else if stderr.is_empty() {
            Err(stdout)
        } else {
            Err(stderr)
        }
    }

    // ========== Builtin backend implementation (adb_client) ==========

    /// Describe the embedded direct-USB backend.
    #[cfg(feature = "builtin-adb")]
    fn version_builtin() -> String {
        "adb_client (direct USB)".to_string()
    }

    /// List direct USB devices without contacting an ADB server.
    #[cfg(feature = "builtin-adb")]
    fn devices_builtin() -> Result<Vec<(String, AdbDeviceStatus)>, String> {
        with_builtin_usb_devices(|cached_devices| {
            let candidates = enumerate_usb_devices()?;
            cached_devices.retain(|cached| {
                candidates.iter().any(|candidate| {
                    is_same_usb_device(
                        &cached.identifier,
                        cached.location,
                        &candidate.identifier,
                        candidate.location,
                    )
                })
            });

            Ok(candidates
                .into_iter()
                .map(|candidate| {
                    let identifier = candidate.identifier.clone();
                    let location = candidate.location;
                    let status = if cached_devices.iter().any(|cached| {
                        is_same_usb_device(
                            &cached.identifier,
                            cached.location,
                            &candidate.identifier,
                            candidate.location,
                        )
                    }) {
                        AdbDeviceStatus::Device
                    } else {
                        match Self::open_builtin_candidate(candidate) {
                            Ok(device) => {
                                cached_devices.push(CachedBuiltinDevice {
                                    identifier: identifier.clone(),
                                    location,
                                    device,
                                });
                                AdbDeviceStatus::Device
                            }
                            Err(err) => {
                                warn!("Direct USB device {identifier} is not ready: {err}");
                                Self::builtin_connection_status(&err)
                            }
                        }
                    };
                    (identifier, status)
                })
                .collect())
        })
    }

    #[cfg(feature = "builtin-adb")]
    fn builtin_connection_status(error: &str) -> AdbDeviceStatus {
        let normalized = error.to_ascii_lowercase();
        if normalized.contains("busy") {
            AdbDeviceStatus::Busy
        } else if normalized.contains("timeout") || normalized.contains("timed out") {
            AdbDeviceStatus::Unauthorized
        } else if normalized.contains("permission") || normalized.contains("access") {
            AdbDeviceStatus::NoPermissions
        } else if normalized.contains("disconnect")
            || normalized.contains("not connected")
            || normalized.contains("no device")
        {
            AdbDeviceStatus::Offline
        } else {
            AdbDeviceStatus::BackendError(error.to_string())
        }
    }

    #[cfg(feature = "builtin-adb")]
    fn open_builtin_candidate(candidate: UsbDeviceCandidate) -> Result<ADBUSBDevice, String> {
        let private_key = ensure_adb_private_key()?;
        ADBUSBDevice::new_from_transport(
            USBTransport::new_from_device(candidate.device),
            Some(private_key.path().to_path_buf()),
        )
        .map_err(|e| format!("Cannot connect directly to USB device: {e}"))
    }

    #[cfg(feature = "builtin-adb")]
    fn ensure_builtin_device(
        &self,
        cached_devices: &mut Vec<CachedBuiltinDevice>,
    ) -> Result<usize, String> {
        let candidates = enumerate_usb_devices()?;
        let identifiers = candidates
            .iter()
            .map(|candidate| candidate.identifier.clone())
            .collect::<Vec<_>>();
        let selected = select_usb_device_index(&identifiers, self.0.device_serial.as_deref())?;
        let candidate = candidates
            .into_iter()
            .nth(selected)
            .ok_or_else(|| "Selected USB device disappeared".to_string())?;
        let identifier = candidate.identifier.clone();
        let location = candidate.location;

        cached_devices.retain(|cached| {
            retain_cached_device_for_candidate(
                &cached.identifier,
                cached.location,
                &identifier,
                location,
            )
        });
        if let Some(index) = cached_devices.iter().position(|cached| {
            is_same_usb_device(&cached.identifier, cached.location, &identifier, location)
        }) {
            return Ok(index);
        }

        let device = Self::open_builtin_candidate(candidate)?;
        cached_devices.push(CachedBuiltinDevice {
            identifier,
            location,
            device,
        });
        Ok(cached_devices.len() - 1)
    }

    /// Execute a shell command directly over USB.
    #[cfg(feature = "builtin-adb")]
    fn run_shell_command_builtin(&self, shell_command: &str) -> Result<String, String> {
        if shell_command.trim().is_empty() {
            return Err("Empty shell command".into());
        }

        with_builtin_usb_devices(|cached_devices| {
            let device_index = self.ensure_builtin_device(cached_devices)?;
            info!("Ran direct USB shell command: {shell_command}");

            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let wrapped_command = wrap_builtin_shell_command(shell_command);
            let transport_result = cached_devices[device_index]
                .device
                .shell_command(&wrapped_command, Some(&mut stdout), Some(&mut stderr))
                .map_err(|e| {
                    error!("Direct USB shell command failed: {e}");
                    format!("Direct USB shell command failed: {e}")
                });
            if let Err(err) = transport_result {
                cached_devices.remove(device_index);
                return Err(err);
            }

            finish_builtin_shell_command(&stdout, &stderr)
        })
    }

    #[cfg(feature = "builtin-adb")]
    fn reboot_builtin(&self) -> Result<String, String> {
        with_builtin_usb_devices(|cached_devices| {
            let device_index = self.ensure_builtin_device(cached_devices)?;
            let result = cached_devices[device_index]
                .device
                .reboot(RebootType::System)
                .map(|()| String::new())
                .map_err(|e| format!("Direct USB reboot failed: {e}"));
            cached_devices.remove(device_index);
            result
        })
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
    fn devices_system() -> Result<Vec<(String, AdbDeviceStatus)>, String> {
        let mut cmd = std::process::Command::new("adb");
        cmd.arg("devices");
        Ok(Self::run_system_command(cmd)?
            .lines()
            .skip(1) // header
            .filter_map(|line| {
                let (serial, status) = line.split_once('\t')?;
                Some((serial.to_string(), AdbDeviceStatus::from_system(status)))
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

    fn reboot_system(&self) -> Result<String, String> {
        let mut cmd = std::process::Command::new("adb");
        if let Some(ref serial) = self.0.device_serial {
            cmd.args(["-s", serial]);
        }
        cmd.arg("reboot");
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

    fn reboot(&self) -> Result<String, String> {
        match self.0.backend {
            #[cfg(feature = "builtin-adb")]
            AdbBackend::Builtin => self.reboot_builtin(),
            AdbBackend::System => self.reboot_system(),
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
        self.0.reboot()
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
                    if PackageId::new(pkg).is_some() {
                        Some(pkg.to_string())
                    } else {
                        warn!("skipping nonstandard package name: {pkg:?}");
                        None
                    }
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
        assert_eq!(AdbBackend::Builtin.to_string(), "Builtin (direct USB)");
        assert_eq!(AdbBackend::System.to_string(), "System (adb)");
    }

    #[test]
    fn backend_default_is_system() {
        assert_eq!(AdbBackend::default(), AdbBackend::System);
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn builtin_version_does_not_embed_a_dependency_version() {
        assert_eq!(ACommand::version_builtin(), "adb_client (direct USB)");
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn builtin_device_selection_requires_an_exact_choice() {
        let available = vec!["phone-a".to_string(), "phone-b".to_string()];
        assert_eq!(select_usb_device_index(&available, Some("phone-b")), Ok(1));
        assert!(select_usb_device_index(&available, Some("missing")).is_err());
        assert!(select_usb_device_index(&available, None).is_err());
        assert_eq!(select_usb_device_index(&["only".to_string()], None), Ok(0));
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn cached_device_is_dropped_when_its_usb_location_changes() {
        let original = UsbDeviceLocation { bus: 1, address: 2 };
        let replugged = UsbDeviceLocation { bus: 1, address: 3 };

        assert!(!retain_cached_device_for_candidate(
            "phone", original, "phone", replugged
        ));
        assert!(retain_cached_device_for_candidate(
            "other-phone",
            original,
            "phone",
            replugged
        ));
        assert!(retain_cached_device_for_candidate(
            "phone", original, "phone", original
        ));
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn builtin_shell_nonzero_exit_is_an_error() {
        let result = finish_builtin_shell_command(
            b"partial output\n__UAD_EXIT_STATUS__:1\n",
            b"permission denied",
        );
        let error = result.expect_err("non-zero exit status must fail");
        assert!(error.contains("status 1"));
        assert!(error.contains("permission denied"));
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn builtin_shell_success_preserves_output() {
        assert_eq!(
            finish_builtin_shell_command(b"success\n__UAD_EXIT_STATUS__:0\n", &[]),
            Ok("success".to_string())
        );
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn builtin_shell_requires_a_valid_status_marker() {
        assert!(finish_builtin_shell_command(b"output without status", &[]).is_err());
        assert!(
            finish_builtin_shell_command(b"output\n__UAD_EXIT_STATUS__:not-a-number", &[]).is_err()
        );
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn builtin_shell_wrapper_reports_the_remote_status() {
        let wrapped = wrap_builtin_shell_command("pm list packages");
        assert!(wrapped.starts_with("pm list packages\n"));
        assert!(wrapped.contains(BUILTIN_EXIT_MARKER));
        assert!(wrapped.contains("$?"));
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn adb_key_path_honors_android_user_home_directly() {
        assert_eq!(
            resolve_adb_key_path(
                Some(PathBuf::from("/custom/android-home")),
                Some(PathBuf::from("/home/user"))
            ),
            Some(PathBuf::from("/custom/android-home/adbkey"))
        );
        assert_eq!(
            resolve_adb_key_path(None, Some(PathBuf::from("/home/user"))),
            Some(PathBuf::from("/home/user/.android/adbkey"))
        );
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn adb_key_write_is_atomic_and_does_not_replace_an_existing_key() {
        let directory = tempfile::tempdir().expect("temporary key directory");
        let key_path = directory.path().join("adbkey");

        write_adb_private_key(&key_path, "first key").expect("write initial key");
        assert_eq!(
            std::fs::read_to_string(&key_path).expect("read initial key"),
            "first key"
        );

        write_adb_private_key(&key_path, "replacement key")
            .expect("existing key wins concurrent creation");
        assert_eq!(
            std::fs::read_to_string(&key_path).expect("read preserved key"),
            "first key"
        );
        assert_eq!(
            std::fs::read_dir(directory.path())
                .expect("list temporary key directory")
                .count(),
            1,
            "temporary files must be cleaned up"
        );
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn existing_pkcs1_adb_key_is_converted_without_replacing_it() {
        use rsa::pkcs1::EncodeRsaPrivateKey;

        let directory = tempfile::tempdir().expect("temporary key directory");
        let key_path = directory.path().join("adbkey");
        let key = RsaPrivateKey::new(&mut OsRng, 1024).expect("generate test key");
        let pkcs1 = key
            .to_pkcs1_pem(LineEnding::LF)
            .expect("encode PKCS#1 test key");
        std::fs::write(&key_path, pkcs1.as_bytes()).expect("write PKCS#1 test key");

        let prepared =
            prepare_existing_adb_private_key(&key_path).expect("prepare existing PKCS#1 key");

        assert_ne!(prepared.path(), key_path);
        assert!(
            RsaPrivateKey::from_pkcs8_pem(
                &std::fs::read_to_string(prepared.path()).expect("read converted key")
            )
            .is_ok()
        );
        assert_eq!(
            std::fs::read_to_string(&key_path).expect("read original key"),
            pkcs1.as_str()
        );
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn invalid_existing_adb_key_has_an_actionable_error() {
        let directory = tempfile::tempdir().expect("temporary key directory");
        let key_path = directory.path().join("adbkey");
        std::fs::write(&key_path, "not a private key").expect("write invalid key");

        let error = prepare_existing_adb_private_key(&key_path)
            .err()
            .expect("invalid key must fail");

        assert!(error.contains("not a valid unencrypted PKCS#8 or PKCS#1"));
        assert!(error.contains(&key_path.display().to_string()));
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn usb_matching_only_accepts_interfaces_supported_by_transport() {
        assert!(is_supported_adb_interface(0xff, 0x42, 0x01));
        assert!(!is_supported_adb_interface(0xdc, 0x02, 0x01));
        assert!(!is_supported_adb_interface(0xff, 0x42, 0x00));
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn builtin_connection_errors_have_non_ready_statuses() {
        assert_eq!(
            ACommand::builtin_connection_status("DeviceBusy"),
            AdbDeviceStatus::Busy
        );
        assert_eq!(
            ACommand::builtin_connection_status("operation timed out"),
            AdbDeviceStatus::Unauthorized
        );
        assert_eq!(
            ACommand::builtin_connection_status("permission denied"),
            AdbDeviceStatus::NoPermissions
        );
        assert_eq!(
            ACommand::builtin_connection_status("unexpected disconnect"),
            AdbDeviceStatus::Offline
        );
        assert_eq!(
            ACommand::builtin_connection_status("invalid PKCS#8 key"),
            AdbDeviceStatus::BackendError("invalid PKCS#8 key".to_string())
        );
    }

    #[test]
    fn system_status_parser_accepts_detailed_permission_errors() {
        assert_eq!(
            AdbDeviceStatus::from_system(
                "no permissions (user in plugdev group; are your udev rules wrong?)"
            ),
            AdbDeviceStatus::NoPermissions
        );
        assert_eq!(
            AdbDeviceStatus::from_system("recovery"),
            AdbDeviceStatus::Other("recovery".to_string())
        );
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn usb_cache_identity_includes_physical_location() {
        let original = UsbDeviceLocation { bus: 1, address: 2 };
        assert!(is_same_usb_device("phone", original, "phone", original));
        assert!(!is_same_usb_device(
            "phone",
            original,
            "phone",
            UsbDeviceLocation { bus: 1, address: 3 }
        ));
    }

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn builtin_usb_operations_are_serialized() {
        use std::sync::mpsc;
        use std::time::Duration;

        let (first_entered_tx, first_entered_rx) = mpsc::channel();
        let (release_first_tx, release_first_rx) = mpsc::channel();
        let first = std::thread::spawn(move || {
            with_builtin_usb_devices(|_| {
                first_entered_tx.send(()).expect("signal first operation");
                release_first_rx.recv().expect("release first operation");
                Ok(())
            })
        });
        first_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first operation should acquire the lock");

        let (second_entered_tx, second_entered_rx) = mpsc::channel();
        let second = std::thread::spawn(move || {
            with_builtin_usb_devices(|_| {
                second_entered_tx.send(()).expect("signal second operation");
                Ok(())
            })
        });

        assert!(
            second_entered_rx
                .recv_timeout(Duration::from_millis(50))
                .is_err(),
            "second operation must wait for the first"
        );
        release_first_tx.send(()).expect("release first operation");
        second_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second operation should run after the first");

        first
            .join()
            .expect("first operation thread")
            .expect("first operation result");
        second
            .join()
            .expect("second operation thread")
            .expect("second operation result");
    }
}
