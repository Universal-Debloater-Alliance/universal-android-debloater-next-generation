use crate::{
    adb::{ACommand as AdbCommand, PM_CLEAR_PACK, PackageId},
    uad_lists::PackageState,
};
use log::{error, info};
use retry::{OperationResult, delay::Fixed, retry};
use serde::{Deserialize, Serialize};

pub use crate::adb::profile_access_denied;

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
    pub user_list_error: Option<String>,
    /// Unique serial identifier
    pub adb_id: String, // could be `Copy`
}

impl Default for Phone {
    fn default() -> Self {
        Self {
            model: "fetching devices...".to_string(),
            android_sdk: 0,
            user_list: vec![],
            user_list_error: None,
            adb_id: String::default(),
        }
    }
}

impl std::fmt::Display for Phone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.model)
    }
}

impl Phone {
    /// Only the owner on a known legacy device can be addressed without `--user`.
    pub fn package_user_id(&self, id: u16) -> Result<Option<u16>, String> {
        match self.android_sdk {
            0 => Err(
                "Could not determine the Android SDK version. Refresh the device and retry.".into(),
            ),
            1..21 if id == 0 => Ok(None),
            1..21 => Err(format!(
                "Cannot target user {id} on Android SDK {}.",
                self.android_sdk
            )),
            _ => Ok(Some(id)),
        }
    }
}

/// `UserInfo` but relevant to UAD
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct User {
    pub id: u16,
    pub index: usize,
    pub protected: bool,
}

impl User {
    /// System packages loaded for this user. Only a protected profile can have an empty list.
    pub fn packages<T>(self, inventory: &[Vec<T>]) -> Result<&[T], String> {
        let packages = inventory
            .get(self.index)
            .ok_or_else(|| format!("No package list loaded for {self}"))?;
        if packages.is_empty() && !self.protected {
            return Err(format!("The packages of {self} could not be read"));
        }
        Ok(packages)
    }
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "user {}", self.id)
    }
}

/// An enum to contain different variants for errors yielded by ADB.
#[derive(Debug, Clone)]
pub enum AdbError {
    Generic(String),
}

/// Run an arbitrary shell action via the typed ADB wrapper.
/// This replaces the deprecated `adb_shell_command`.
///
/// If `serial` is empty, it lets ADB choose the default device.
pub fn run_adb_shell_action<S: AsRef<str>>(
    device_serial: S,
    action: &str,
) -> Result<String, AdbError> {
    let serial = device_serial.as_ref();

    match AdbCommand::new().shell(serial).raw(action) {
        Ok(o) => {
            if crate::adb::command_failed(&o) {
                let friendly_msg = make_friendly_error_message(&o, action);
                return Err(AdbError::Generic(friendly_msg));
            }
            info!("{action} -> {o}");
            Ok(o)
        }
        Err(err) => {
            if !err.contains("[not installed for") {
                let friendly_msg = make_friendly_error_message(&err, action);
                return Err(AdbError::Generic(friendly_msg));
            }
            Err(AdbError::Generic(err))
        }
    }
}

/// Convert common OEM-specific ADB error messages into user-friendly explanations.
fn make_friendly_error_message(error_output: &str, action: &str) -> String {
    // Common Samsung errors
    if error_output.contains("DELETE_FAILED_USER_RESTRICTED") {
        return format!(
            "Cannot uninstall: This package is restricted by the device manufacturer (Samsung Knox or similar).\n\
            Error: {error_output}\n\
            Tip: Try disabling the package instead, or check device settings for Knox/security restrictions."
        );
    }

    if error_output.contains("NOT_INSTALLED_FOR_USER") {
        return format!(
            "Package is not installed for the current user.\n\
            Error: {error_output}\n\
            Tip: The package may be installed for a different user profile or work profile."
        );
    }

    // Empty package name error
    if error_output.contains("Shell cannot change component state for null") {
        return format!(
            "Invalid package: Empty package name detected.\n\
            Error: {error_output}\n\
            Tip: Please refresh the package list and try again."
        );
    }

    // Generic permission errors
    if error_output.contains("Permission denied")
        || error_output.contains("INSTALL_FAILED_PERMISSION_MODEL_DOWNGRADE")
    {
        return format!(
            "Permission denied: Insufficient privileges to perform this action.\n\
            Error: {error_output}\n\
            Tip: This may require root access or the package is protected by the system."
        );
    }

    // Work profile / managed device errors
    if error_output.contains("DELETE_FAILED_DEVICE_POLICY_MANAGER") {
        return format!(
            "Cannot modify: Package is managed by device policy (MDM/EMM).\n\
            Error: {error_output}\n\
            Tip: Contact your IT administrator if this is a work device."
        );
    }

    // Generic failure with context
    format!("{action} -> {error_output}")
}

/// If `None`, returns an empty String, not " --user 0"
#[must_use]
pub fn user_flag(user_id: Option<User>) -> String {
    user_id.map_or_else(String::new, |user| format!(" --user {}", user.id))
}

// Minimum information for processing adb commands
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct CorePackage {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub removal: crate::uad_lists::Removal,
    #[serde(default)]
    pub state: PackageState,
    #[serde(default)]
    pub list: crate::uad_lists::UadList,
}

pub fn apply_pkg_state_commands(
    package: &CorePackage,
    wanted_state: PackageState,
    selected_user: User,
    phone: &Phone,
) -> Result<Vec<String>, String> {
    let user = phone
        .package_user_id(selected_user.id)?
        .map(|_| selected_user);
    commands_for_user(package, wanted_state, user, phone)
}

/// Plan one package transition after the caller has resolved the target user.
pub(crate) fn commands_for_user(
    package: &CorePackage,
    wanted_state: PackageState,
    user: Option<User>,
    phone: &Phone,
) -> Result<Vec<String>, String> {
    // https://github.com/Universal-Debloater-Alliance/universal-android-debloater/wiki/ADB-reference
    // ALWAYS PUT THE COMMAND THAT CHANGES THE PACKAGE STATE FIRST!
    let commands = match wanted_state {
        PackageState::Enabled => match package.state {
            PackageState::Disabled => vec!["pm enable"],
            PackageState::Uninstalled => match phone.android_sdk {
                i if i >= 23 => vec!["cmd package install-existing"],
                21 | 22 => vec!["pm unhide"],
                19 | 20 => vec!["pm unblock", PM_CLEAR_PACK],
                sdk => return Err(format!("Cannot reinstall a package on Android SDK {sdk}.")),
            },
            _ => vec![],
        },
        PackageState::Disabled => match package.state {
            // Not for an uninstalled package: it would stay uninstalled,
            // and be disabled the next time it is installed.
            PackageState::Enabled => {
                if supports_disabling(phone) {
                    vec!["pm disable-user", "am force-stop", PM_CLEAR_PACK]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        },
        PackageState::Uninstalled => match package.state {
            PackageState::Enabled | PackageState::Disabled => match phone.android_sdk {
                sdk if sdk >= 23 => vec!["pm uninstall"], // > Marshmallow (6.0)
                21 | 22 => vec!["pm hide", PM_CLEAR_PACK], // Lollipop (5.x)
                _ => vec!["pm block", PM_CLEAR_PACK], // Disable mode is unavailable on older devices because the specific ADB commands need root
            },
            _ => vec![],
        },
        PackageState::All => vec![],
    }; // this should be a `tinyvec`, as `len <= 4`

    Ok(request_builder(&commands, &package.name, user))
}

/// Build a command request to be sent via ADB to a device.
/// `commands` accepts one or more ADB shell commands
/// which act on a common `package` and `user`.
#[must_use]
pub fn request_builder(commands: &[&str], package: &str, user: Option<User>) -> Vec<String> {
    // Defense-in-depth: `package` is interpolated verbatim into a device-shell
    // action, so refuse any invalid name/ID. Every current caller already
    // reconciles the name against the live device package list before reaching
    // here, so this rejects nothing legitimate — it just keeps the no-injection
    // guarantee local to the sink instead of relying on each caller to sanitise.
    // Fail closed: emit no command for a malformed name rather than an injectable
    // device-shell string.
    if PackageId::new(package).is_none() {
        error!("request_builder: refusing invalid package name: {package:?}");
        return Vec::new();
    }
    let maybe_user_flag = user_flag(user);
    commands
        .iter()
        .map(|c| format!("{c}{maybe_user_flag} {package}"))
        .collect()
}

/// Get the model by querying the `ro.product.model` property.
///
/// If `serial` is empty, it lets ADB choose the default device.
#[must_use]
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
#[must_use]
pub fn get_device_brand(serial: &str) -> String {
    AdbCommand::new()
        .shell(serial)
        .getprop("ro.product.brand")
        // `trim` is just-in-case
        .map_or_else(|_| String::new(), |s| s.trim().to_string())
}

/// Get Android SDK version by querying the
// `ro.build.version.sdk` property or defaulting to 0.
///
/// If `device_serial` is empty, it lets ADB choose the default device.
#[must_use]
pub fn get_android_sdk(device_serial: &str) -> u8 {
    AdbCommand::new()
        .shell(device_serial)
        .getprop("ro.build.version.sdk")
        .ok()
        .and_then(|sdk| sdk.parse().ok())
        .unwrap_or(0)
}

/// Capture the current state of a package across all non-protected users.
/// This is used to detect cross-user behavior by comparing before and after states.
///
/// Only includes users where the package exists (Some state). Users where the package
/// doesn't exist (None) are not tracked.
#[must_use]
pub fn capture_cross_user_states(
    package_name: &str,
    device_serial: &str,
    target_user_id: u16,
    phone: &Phone,
) -> Vec<(u16, PackageState)> {
    phone
        .user_list
        .iter()
        .filter(|u| !u.protected && u.id != target_user_id)
        .filter_map(|u| {
            verify_package_state(package_name, device_serial, Some(u.id)).map(|state| (u.id, state))
        })
        .collect()
}

/// Detect cross-user behavior and return appropriate notification message.
/// This handles unexpected cross-user behavior:
/// - Case A: Uninstall → Restore (package appears on other users)
/// - Case B: Uninstall → Uninstall (package disappears from other users that previously had it)
/// - Case C: Restore → Restore (package appears on other users)
#[must_use]
pub fn detect_cross_user_behavior(
    package_name: &str,
    device_serial: &str,
    target_user_id: u16,
    wanted_state: PackageState,
    actual_state: PackageState,
    phone: &Phone,
    before_states: &[(u16, PackageState)],
) -> Option<String> {
    // Only check if operation was successful on target user
    if actual_state != wanted_state {
        return None;
    }

    // Only check if we have multiple users
    if phone.user_list.len() < 2 {
        return None;
    }

    let after_states =
        check_cross_user_package_existence(package_name, device_serial, target_user_id, phone);
    cross_user_change(target_user_id, wanted_state, before_states, &after_states)
}

/// Compare the package of the other users before and after an action on `target_user_id`.
fn cross_user_change(
    target_user_id: u16,
    wanted_state: PackageState,
    before_states: &[(u16, PackageState)],
    after_states: &[(u16, PackageState)],
) -> Option<String> {
    // Check if a user didn't have the package before (not tracked or package didn't exist).
    // Detects packages that appear on users where they didn't exist previously (OEM cross-user restoration).
    let was_package_absent_before = |uid: &u16| {
        before_states
            .iter()
            .find(|(before_uid, _)| before_uid == uid)
            .is_none_or(|(_, before_state)| *before_state == PackageState::Uninstalled)
    };
    let newly_appeared: Vec<_> = after_states
        .iter()
        .filter(|(uid, _after_state)| was_package_absent_before(uid))
        .collect();

    match wanted_state {
        PackageState::Uninstalled => {
            if after_states.is_empty() {
                // Case B: Uninstall → Uninstall (check if all users lost package)
                let affected_users: Vec<_> = before_states
                    .iter()
                    .filter(|(uid, before_state)| {
                        // Only flag if the package was installed/enabled/disabled before
                        *before_state != PackageState::Uninstalled
                            // And is NOT in after_states (doesn't exist in usable state anymore)
                            && !after_states.iter().any(|(after_uid, _)| after_uid == uid)
                    })
                    .map(|(uid, _)| uid)
                    .collect();

                if affected_users.is_empty() {
                    None
                } else {
                    let user_list = affected_users
                        .iter()
                        .map(|uid| format!("user {uid}"))
                        .collect::<Vec<_>>()
                        .join(", ");

                    Some(format!(
                        "Detected cross-user uninstall: package was also uninstalled from {user_list} after uninstalling from user {target_user_id}"
                    ))
                }
            } else if newly_appeared.is_empty() {
                // The other users only kept the package they already had.
                None
            } else {
                // Case A: Uninstall → Restore (package appears on other users)
                let user_list = newly_appeared
                    .iter()
                    .map(|(uid, state)| format!("user {uid} ({state:?})"))
                    .collect::<Vec<_>>()
                    .join(", ");

                Some(format!(
                    "Detected cross-user restoration: package exists on {user_list} after uninstalling from user {target_user_id}"
                ))
            }
        }
        PackageState::Enabled | PackageState::Disabled => {
            // Case C: Restore → Restore (package appears on other users)
            if newly_appeared.is_empty() {
                None
            } else {
                let user_list = newly_appeared
                    .iter()
                    .map(|(uid, state)| format!("user {uid} ({state:?})"))
                    .collect::<Vec<_>>()
                    .join(", ");

                Some(format!(
                    "Detected cross-user restoration: package exists on {user_list} after {} from user {target_user_id}",
                    if wanted_state == PackageState::Enabled {
                        "enabling"
                    } else {
                        "disabling"
                    }
                ))
            }
        }
        PackageState::All => None,
    }
}

#[must_use]
pub const fn supports_disabling(dev: &Phone) -> bool {
    // >= Marshmallow (6.0)
    dev.android_sdk >= 23
}

/// Check if it might support multi-user mode,
/// by simply comparing SDK version.
/// `true` isn't reliable, you can only trust `false`.
///
/// See:
/// - <https://source.android.com/docs/devices/admin/multi-user#applying_the_overlay>
/// - <https://developer.android.com/reference/android/os/UserManager#supportsMultipleUsers()>
#[must_use]
pub const fn supports_multi_user(dev: &Phone) -> bool {
    // >= Lollipop (5.0)
    dev.android_sdk >= 21
}

/// Check if a `user_id` is protected on a device by trying
/// to list associated packages.
///
/// If `device_serial` is empty, it lets ADB choose the default device.
pub fn is_protected_user<S: AsRef<str>>(user_id: u16, device_serial: S) -> bool {
    match AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_packages_sys(None, Some(user_id))
    {
        Ok(_) => false,
        Err(error) if profile_access_denied(&error) => true,
        Err(error) => {
            error!("Could not check access to user {user_id}: {error}");
            false
        }
    }
}

pub fn list_users_idx_prot(device_serial: &str) -> Result<Vec<User>, String> {
    let users = AdbCommand::new().shell(device_serial).pm().list_users()?;
    if users.is_empty() {
        return Err("ADB returned an empty user list".into());
    }
    Ok(users
        .into_iter()
        .enumerate()
        .map(|(i, user)| {
            let id = user.get_id();
            User {
                id,
                index: i,
                protected: is_protected_user(id, device_serial),
            }
        })
        .collect())
}

/// Older versions of pm print this diagnostic followed by their usage text.
fn user_listing_unsupported(error: &str) -> bool {
    error.trim_start().lines().next().is_some_and(|line| {
        [
            "Error: unknown list type 'users'",
            "Unknown command: list users",
            "Unknown command: users",
        ]
        .iter()
        .any(|message| line.trim_end().eq_ignore_ascii_case(message))
    })
}

/// This matches serials (`getprop ro.serialno`)
/// that are authorized by the user.
#[must_use]
pub fn get_devices_list() -> Vec<Phone> {
    retry(
        Fixed::from_millis(500).take(if cfg!(debug_assertions) { 3 } else { 10 }),
        || match AdbCommand::new().devices() {
            Ok(devices) => {
                let mut device_list: Vec<Phone> = vec![];
                if devices.iter().all(|(_, stat)| stat != "device") {
                    return OperationResult::Retry(vec![]);
                }
                for device in devices {
                    let serial = &device.0;
                    let android_sdk = get_android_sdk(serial);
                    let (user_list, user_list_error) = match list_users_idx_prot(serial) {
                        Ok(users) => (users, None),
                        Err(error) if (1..21).contains(&android_sdk) && user_listing_unsupported(&error) => (vec![User::default()], None),
                        Err(error) => (Vec::new(), Some(format!("Could not list device users: {error}. Refresh the device and retry."))),
                    };
                    device_list.push(Phone {
                        model: format!("{} {}", get_device_brand(serial), get_device_model(serial)),
                        android_sdk,
                        user_list,
                        user_list_error,
                        adb_id: serial.clone(),
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

#[must_use]
pub fn initial_load() -> bool {
    match AdbCommand::new().devices() {
        Ok(_devices) => true,
        Err(_err) => false,
    }
}

/// Get the current state of a package on a device
#[must_use]
pub fn get_package_state(
    device_serial: &str,
    package_name: &str,
    user_id: Option<u16>,
) -> Option<PackageState> {
    query_package_state(device_serial, package_name, user_id)
        .ok()
        .flatten()
}

/// Distinguish a successful search with no match from a failed ADB query.
pub fn query_package_state(
    device_serial: &str,
    package_name: &str,
    user_id: Option<u16>,
) -> Result<Option<PackageState>, String> {
    use crate::adb::PmListPacksFlag;
    for (flag, state) in [
        (PmListPacksFlag::OnlyEnabled, PackageState::Enabled),
        (PmListPacksFlag::OnlyDisabled, PackageState::Disabled),
        (
            PmListPacksFlag::IncludeUninstalled,
            PackageState::Uninstalled,
        ),
    ] {
        let packages = AdbCommand::new()
            .shell(device_serial)
            .pm()
            .list_packages_sys(Some(flag), user_id)?;
        if flag == PmListPacksFlag::IncludeUninstalled && packages.is_empty() {
            return Err("ADB returned an empty system package list".into());
        }
        if packages.iter().any(|name| name == package_name) {
            return Ok(Some(state));
        }
    }
    Ok(None)
}

/// Verify the current state of a package on a device.
/// Returns the package state, or `None` if the package doesn't exist.
#[must_use]
pub fn verify_package_state(
    package_name: &str,
    device_serial: &str,
    user_id: Option<u16>,
) -> Option<PackageState> {
    get_package_state(device_serial, package_name, user_id)
}

/// Check if a package exists on any other users besides the target user.
/// This helps detect OEM-specific cross-user restoration behavior.
///
/// Only includes users where the package exists in a non-uninstalled state
/// (i.e., Enabled or Disabled).
#[must_use]
pub fn check_cross_user_package_existence(
    package_name: &str,
    device_serial: &str,
    target_user_id: u16,
    phone: &Phone,
) -> Vec<(u16, PackageState)> {
    let mut other_user_states = Vec::new();

    for user in &phone.user_list {
        if user.id != target_user_id
            && !user.protected
            && let Some(state) = verify_package_state(package_name, device_serial, Some(user.id))
            && state != PackageState::Uninstalled
        {
            other_user_states.push((user.id, state));
        }
    }

    other_user_states
}

/// Creates a `CorePackage` with the specified state, preserving other fields from the original.
fn package_with_state(package: &CorePackage, state: PackageState) -> CorePackage {
    CorePackage {
        state,
        ..package.clone()
    }
}

/// Executes the first available command and returns the result.
fn execute_first_command(commands: &[String], phone: &Phone) -> Result<(), String> {
    let command = commands.first().ok_or("No command available")?;
    run_adb_shell_action(&phone.adb_id, command)
        .map(|_| ())
        .map_err(|AdbError::Generic(error)| error)
}

/// Attempt fallback action when package state verification fails
pub fn attempt_fallback(
    package: &CorePackage,
    wanted_state: PackageState,
    actual_state: PackageState,
    user: User,
    phone: &Phone,
) -> Result<String, String> {
    let (expected, message) =
        run_fallback_commands(package, wanted_state, actual_state, user, phone)?;
    let confirmed = query_package_state(
        &phone.adb_id,
        &package.name,
        phone.package_user_id(user.id)?,
    )?
    .unwrap_or(PackageState::Uninstalled);
    verify_fallback_state(expected, confirmed)?;
    Ok(message.to_owned())
}

pub fn verify_fallback_state(
    expected: PackageState,
    confirmed: PackageState,
) -> Result<(), String> {
    if confirmed == expected {
        Ok(())
    } else {
        Err(format!(
            "After recovery, expected {expected:?} but found {confirmed:?}."
        ))
    }
}

/// Run the recovery commands and return the state the caller must verify.
pub fn run_fallback_commands(
    package: &CorePackage,
    wanted_state: PackageState,
    actual_state: PackageState,
    user: User,
    phone: &Phone,
) -> Result<(PackageState, &'static str), String> {
    match (wanted_state, actual_state) {
        // Case 1: Tried to uninstall but package was reinstalled -> disable it
        (PackageState::Uninstalled, PackageState::Enabled) => {
            let pkg = package_with_state(package, PackageState::Enabled);
            let commands = apply_pkg_state_commands(&pkg, PackageState::Disabled, user, phone)?;
            execute_first_command(&commands, phone)
                .map_err(|e| format!("Failed to disable package: {e}"))?;
            Ok((
                PackageState::Disabled,
                "disabled package instead of uninstalling",
            ))
        }

        // Case 2: Tried to disable but package re-enabled itself -> try uninstall
        (PackageState::Disabled, PackageState::Enabled) => {
            let pkg = package_with_state(package, PackageState::Enabled);
            let commands = apply_pkg_state_commands(&pkg, PackageState::Uninstalled, user, phone)?;
            execute_first_command(&commands, phone)
                .map_err(|e| format!("Failed to uninstall: {e}"))?;

            Ok((
                PackageState::Uninstalled,
                "uninstalled package instead of disabling",
            ))
        }

        // Case 3: Tried to enable but package was disabled -> try uninstall then reinstall
        (PackageState::Enabled, PackageState::Disabled) => {
            let pkg = package_with_state(package, PackageState::Disabled);
            let uninstall_cmds =
                apply_pkg_state_commands(&pkg, PackageState::Uninstalled, user, phone)?;
            let pkg_uninstalled = package_with_state(package, PackageState::Uninstalled);
            let enable_cmds =
                apply_pkg_state_commands(&pkg_uninstalled, PackageState::Enabled, user, phone)?;

            if enable_cmds.is_empty() {
                return Err("No command available to reinstall the package".to_string());
            }

            execute_first_command(&uninstall_cmds, phone)
                .map_err(|e| format!("Failed to uninstall for reinstall: {e}"))?;
            execute_first_command(&enable_cmds, phone)
                .map_err(|e| format!("Failed to reinstall package: {e}"))?;
            Ok((
                PackageState::Enabled,
                "uninstalled and reinstalled package to enable it",
            ))
        }

        // Other cases - no fallback available
        _ => Err(format!(
            "No fallback available for wanted state {wanted_state:?} and actual state {actual_state:?}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_unsupported_user_list_can_fall_back_to_the_owner() {
        assert!(user_listing_unsupported(
            "Error: unknown list type 'users'\nusage: pm ..."
        ));
        for error in [
            "error: device offline",
            "Can't find service: package",
            "ADB returned an empty user list",
            "Invalid user ID",
            "Unknown command: packages",
        ] {
            assert!(!user_listing_unsupported(error), "{error}");
        }
    }

    #[test]
    fn only_a_known_legacy_owner_uses_unscoped_commands() {
        let mut phone = Phone::default();
        assert!(phone.package_user_id(0).is_err());
        phone.android_sdk = 19;
        assert_eq!(phone.package_user_id(0), Ok(None));
        assert!(phone.package_user_id(10).is_err());
        phone.android_sdk = 35;
        assert_eq!(phone.package_user_id(10), Ok(Some(10)));
    }

    #[test]
    fn an_uninstalled_package_is_not_disabled() {
        let phone = Phone {
            android_sdk: 35,
            ..Phone::default()
        };
        let commands = |state| {
            let package = CorePackage {
                name: "com.example.app".to_string(),
                description: String::new(),
                removal: crate::uad_lists::Removal::Recommended,
                state,
                list: crate::uad_lists::UadList::Misc,
            };
            apply_pkg_state_commands(&package, PackageState::Disabled, User::default(), &phone)
                .unwrap()
        };
        assert_eq!(commands(PackageState::Enabled).len(), 3);
        assert_eq!(commands(PackageState::Uninstalled), Vec::<String>::new());
    }

    #[test]
    fn uninstall_only_reports_users_where_the_package_appeared() {
        let enabled = [(10, PackageState::Enabled)];
        // The other user kept its package.
        assert_eq!(
            cross_user_change(0, PackageState::Uninstalled, &enabled, &enabled),
            None
        );
        // It was uninstalled for the other user, and came back.
        let uninstalled = [(10, PackageState::Uninstalled)];
        let message = cross_user_change(0, PackageState::Uninstalled, &uninstalled, &enabled);
        assert!(message.unwrap().contains("cross-user restoration"));
    }
}
