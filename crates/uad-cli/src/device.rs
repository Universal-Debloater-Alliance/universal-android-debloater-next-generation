use uad_core::sync::{Phone, User, get_devices_list};

/// Get target device, either by serial or first available
pub fn get_target_device(device: Option<String>) -> Result<Phone, Box<dyn std::error::Error>> {
    let devices = get_devices_list();

    if devices.is_empty() {
        eprintln!("Error: No devices found");
        return Err("No devices found".into());
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

/// Get user from device, creating a basic one if not found
pub fn get_user(device: &Phone, user_id: Option<u16>) -> Result<User, Box<dyn std::error::Error>> {
    let uid = user_id.unwrap_or(0);

    if let Some(user) = device.user_list.iter().find(|u| u.id == uid) {
        Ok(*user)
    } else {
        // Create a basic user if not found in list
        Ok(User {
            id: uid,
            index: 0,
            protected: false,
        })
    }
}
