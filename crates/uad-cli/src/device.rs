use uad_core::sync::{Phone, User, try_get_devices_list};

pub const NO_DEVICES_FOUND: &str =
    "No devices found. Make sure ADB is installed and devices are connected.";

/// Get target device, either by serial or first available
pub fn get_target_device(device: Option<String>) -> Result<Phone, Box<dyn std::error::Error>> {
    let devices = try_get_devices_list()?;

    if devices.is_empty() {
        return Err(NO_DEVICES_FOUND.into());
    }

    let target_device = if let Some(device_id) = device {
        devices
            .iter()
            .find(|d| d.adb_id == device_id)
            .ok_or("Device not found")?
            .clone()
    } else {
        if devices.len() > 1 {
            eprintln!(
                "Warning: Multiple devices found, using first one: {}",
                devices[0].adb_id
            );
        }
        devices[0].clone()
    };

    Ok(target_device)
}

/// Resolve the requested user, defaulting to the owner.
pub fn get_user(device: &Phone, user_id: Option<u16>) -> Result<User, Box<dyn std::error::Error>> {
    if let Some(error) = &device.user_list_error {
        return Err(error.clone().into());
    }
    let uid = user_id.unwrap_or(0);

    device
        .user_list
        .iter()
        .find(|u| u.id == uid)
        .copied()
        .ok_or_else(|| format!("User {uid} was not found on the device").into())
}
