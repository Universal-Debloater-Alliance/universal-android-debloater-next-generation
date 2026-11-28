use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use uad_core::adb::{ACommand, PackageId, PmListPacksFlag};
use uad_core::sync::{
    CorePackage, Phone, User, apply_pkg_state_commands, query_package_state, run_adb_shell_action,
    try_get_devices_list,
};
use uad_core::uad_lists::{Package, PackageState, Removal, UadList, load_debloat_lists};
use uad_core::utils::{matches_search, truncate_description};

use crate::device::{NO_DEVICES_FOUND, get_target_device, get_user};
use crate::filters::{ListFilter, RemovalFilter, StateFilter};
use crate::{Cli, print_or_exit, println_or_exit};

/// List all connected Android devices
pub fn list_devices() -> Result<(), Box<dyn std::error::Error>> {
    println_or_exit!("Scanning for connected devices...");
    let devices = try_get_devices_list()?;

    if devices.is_empty() {
        return Err(NO_DEVICES_FOUND.into());
    }

    println_or_exit!("\nFound {} device(s):\n", devices.len());
    let mut errors = Vec::new();
    for device in &devices {
        println_or_exit!("  Model:       {}", device.model);
        println_or_exit!("  Serial:      {}", device.adb_id);
        println_or_exit!("  Android SDK: {}", device.android_sdk);

        if let Some(error) = &device.user_list_error {
            errors.push(format!("{}: {error}", device.adb_id));
        } else if !device.user_list.is_empty() {
            println_or_exit!("  Users:       {} user(s)", device.user_list.len());
            for user in &device.user_list {
                let protected = if user.protected { " (protected)" } else { "" };
                println_or_exit!("               - User ID: {}{}", user.id, protected);
            }
        }
        println_or_exit!();
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
}

/// Context for package filtering and display
pub struct PackageListContext {
    pub state_filter: Option<StateFilter>,
    pub removal_filter: Option<RemovalFilter>,
    pub list_filter: Option<ListFilter>,
    pub search: Option<String>,
}

impl PackageListContext {
    /// Check if package passes all filters
    pub fn filter_package(
        &self,
        pkg_name: &str,
        pkg_info: Option<&Package>,
        pkg_state: PackageState,
    ) -> bool {
        // Removal filter
        if let Some(removal) = self.removal_filter {
            if !removal.matches(pkg_info) {
                return false;
            }
        }

        // State filter
        if let Some(state) = self.state_filter {
            if !state.matches(pkg_state) {
                return false;
            }
        }

        // List filter
        if let Some(list) = self.list_filter {
            if !list.matches(pkg_info) {
                return false;
            }
        }

        // Search filter
        if let Some(ref search_term) = self.search {
            let description = pkg_info.map(|p| p.description.as_str());
            if !matches_search(pkg_name, search_term, description) {
                return false;
            }
        }

        true
    }

    /// Determine what information to show based on active filters
    pub fn display_config(&self) -> DisplayConfig {
        DisplayConfig {
            show_state: self.state_filter.is_none_or(|f| !f.is_specific()),
            show_removal: self.removal_filter.is_none_or(|f| !f.is_specific()),
        }
    }
}

pub struct DisplayConfig {
    pub show_state: bool,
    pub show_removal: bool,
}

/// List packages on a device with filtering
pub fn list_packages(
    device: Option<String>,
    state_filter: Option<StateFilter>,
    removal_filter: Option<RemovalFilter>,
    list_filter: Option<ListFilter>,
    search: Option<String>,
    user_id: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_device = get_target_device(device)?;
    let user = get_user(&target_device, user_id)?;
    let uad_lists = load_debloat_lists(false).unwrap_or_else(|lists| lists);

    println_or_exit!(
        "Listing packages on: {} ({})\n",
        target_device.model,
        target_device.adb_id
    );

    let context = PackageListContext {
        state_filter,
        removal_filter,
        list_filter,
        search,
    };

    let displayed_count = display_package_list(&target_device, user, &uad_lists, &context)?;

    if displayed_count == 0 {
        println_or_exit!("  No packages found matching the specified filters.");
    } else {
        println_or_exit!("\nTotal: {} package(s)", displayed_count);
    }

    Ok(())
}

/// Display a list of packages with their info
pub fn display_package_list(
    device: &Phone,
    user: User,
    uad_lists: &HashMap<String, Package>,
    context: &PackageListContext,
) -> Result<usize, Box<dyn std::error::Error>> {
    let config = context.display_config();
    let mut displayed_count = 0;

    let user_id = device.package_user_id(user.id)?;
    let list = |flag| {
        ACommand::new()
            .shell(&device.adb_id)
            .pm()
            .list_packages_sys(flag, user_id)
    };
    let all = list(Some(PmListPacksFlag::IncludeUninstalled))?;
    if all.is_empty() && !user.protected {
        return Err(format!("ADB returned an empty system package list for {user}").into());
    }
    // Without a state filter, list only the packages installed for this user.
    let packages = if context.state_filter.is_none() {
        list(None)?
    } else {
        all
    };
    let state_list =
        |flag| list(Some(flag)).map_err(|e| format!("Could not list the packages: {e}"));
    let enabled_packages: HashSet<_> = state_list(PmListPacksFlag::OnlyEnabled)?
        .into_iter()
        .collect();
    let disabled_packages: HashSet<_> = state_list(PmListPacksFlag::OnlyDisabled)?
        .into_iter()
        .collect();

    for pkg_name in &packages {
        let pkg_info = uad_lists.get(pkg_name);

        let pkg_state = if enabled_packages.contains(pkg_name) {
            PackageState::Enabled
        } else if disabled_packages.contains(pkg_name) {
            PackageState::Disabled
        } else {
            PackageState::Uninstalled
        };

        if !context.filter_package(pkg_name, pkg_info, pkg_state) {
            continue;
        }

        display_package_entry(pkg_name, pkg_info, pkg_state, &config);
        displayed_count += 1;
    }

    Ok(displayed_count)
}

/// Display a single package entry
pub fn display_package_entry(
    pkg_name: &str,
    pkg_info: Option<&Package>,
    pkg_state: PackageState,
    config: &DisplayConfig,
) {
    print_or_exit!("[");

    if let Some(info) = pkg_info {
        if config.show_removal {
            print_or_exit!("{}", info.removal);
            if config.show_state {
                print_or_exit!(" - ");
            }
        }
        if config.show_state {
            print_or_exit!("{}", pkg_state);
        }
        print_or_exit!("] {}", pkg_name);
        if !info.description.is_empty() {
            print_or_exit!(" - {}", truncate_description(&info.description, 80));
        }
    } else {
        if config.show_removal {
            print_or_exit!("Unlisted");
            if config.show_state {
                print_or_exit!(" - ");
            }
        }
        if config.show_state {
            print_or_exit!("{}", pkg_state);
        }
        print_or_exit!("] {}", pkg_name);
    }

    println_or_exit!();
}

/// Change the state of one or more packages
pub fn change_package_state(
    packages: &[String],
    device: Option<String>,
    user_id: Option<u16>,
    dry_run: bool,
    target_state: PackageState,
    action_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if packages.is_empty() {
        return Err("No packages specified".into());
    }

    let target_device = get_target_device(device)?;
    let user = get_user(&target_device, user_id)?;
    let uad_lists = load_debloat_lists(false).unwrap_or_else(|lists| lists);

    println!(
        "{} {} package(s) on: {} ({})\n",
        action_name,
        packages.len(),
        target_device.model,
        target_device.adb_id
    );

    if dry_run {
        println!("DRY RUN - No changes will be made\n");
    }

    for pkg_name in packages {
        process_package_state_change(
            pkg_name,
            &target_device,
            user,
            target_state,
            dry_run,
            &uad_lists,
        )?;
        println!();
    }

    if dry_run {
        println!("Dry run completed. No changes were made.");
    } else {
        println!("Operation completed successfully.");
    }

    Ok(())
}

/// A valid uninstall request is already satisfied if the full inventory omits the name.
pub fn action_package_state(
    device: &Phone,
    user: User,
    name: &str,
    target: PackageState,
) -> Result<Option<PackageState>, Box<dyn std::error::Error>> {
    if user.protected {
        return Err(format!("Cannot modify packages for protected {user}.").into());
    }
    match query_package_state(&device.adb_id, name, device.package_user_id(user.id)?)? {
        Some(state) => Ok(Some(state)),
        None if target == PackageState::Uninstalled && PackageId::new(name).is_some() => Ok(None),
        None => Err("Package not found on device".into()),
    }
}

/// Process state change for a single package
fn process_package_state_change(
    pkg_name: &str,
    device: &Phone,
    user: User,
    target_state: PackageState,
    dry_run: bool,
    uad_lists: &HashMap<String, Package>,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(current_state) = action_package_state(device, user, pkg_name, target_state)? else {
        println!("    → {pkg_name} is already absent, skipping");
        return Ok(());
    };

    println!("  {} ({})", pkg_name, current_state);

    if current_state == target_state {
        println!("    → Already in target state, skipping");
        return Ok(());
    }

    let pkg_info = uad_lists.get(pkg_name);
    if let Some(info) = pkg_info {
        if info.removal == Removal::Unsafe {
            println!("    ⚠ WARNING: This package is marked as UNSAFE to remove!");
        }
    }

    let core_pkg = CorePackage {
        name: pkg_name.to_string(),
        description: pkg_info.map(|p| p.description.clone()).unwrap_or_default(),
        removal: pkg_info.map(|p| p.removal).unwrap_or(Removal::Unlisted),
        state: current_state,
        list: pkg_info.map(|p| p.list).unwrap_or(UadList::Unlisted),
    };

    execute_with_fallback(
        pkg_name,
        target_state,
        &core_pkg,
        user,
        device,
        dry_run,
        "    ",
    )
}

/// Execute commands and verify package state with fallback
pub fn execute_with_fallback(
    package: &str,
    target_state: PackageState,
    core_pkg: &CorePackage,
    user: User,
    device: &Phone,
    dry_run: bool,
    indent: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let commands = apply_pkg_state_commands(core_pkg, target_state, user, device)?;
    if commands.is_empty() {
        return Err(format!(
            "Nothing to run: cannot go from {} to {target_state} for {package}",
            core_pkg.state
        )
        .into());
    }
    if dry_run {
        for cmd in &commands {
            println!("{indent}Would run: {cmd}");
        }
        return Ok(());
    }

    // Capture the before-state of packages on other users for cross-user detection
    let before_cross_user_states =
        uad_core::sync::capture_cross_user_states(package, &device.adb_id, user.id, device);

    // Execute commands
    for cmd in &commands {
        run_adb_shell_action(&device.adb_id, cmd)?;
        println!("{}✓ {}", indent, cmd);
    }

    // Verify package state and attempt fallback if needed
    // A failed query does not mean that the package is uninstalled.
    let reinstalled =
        core_pkg.state == PackageState::Uninstalled && target_state == PackageState::Enabled;
    let actual_state = match device
        .finish_package_action(user, package, reinstalled)
        .map_err(|e| e.with_context(format_args!("Could not verify the state of '{package}'")))?
    {
        Some(state) => state,
        None if target_state == PackageState::Uninstalled => PackageState::Uninstalled,
        None => {
            return Err(format!(
                "Package '{package}' was not found on device for user {}",
                user.id
            )
            .into());
        }
    };

    if actual_state != target_state {
        println!(
            "{}⚠ Package state verification failed: expected {:?}, got {:?}",
            indent, target_state, actual_state
        );

        // Attempt fallback
        let fallback_action =
            uad_core::sync::attempt_fallback(core_pkg, target_state, actual_state, user, device)?;
        println!("{}↻ Fallback: {}", indent, fallback_action);
    }

    // Check for cross-user behavior if operation succeeded
    if actual_state == target_state {
        if let Some(notification) = uad_core::sync::detect_cross_user_behavior(
            package,
            device.adb_id.as_str(),
            user.id,
            target_state,
            actual_state,
            device,
            &before_cross_user_states,
        ) {
            println!("{}ℹ {}", indent, notification);
        }
    }

    Ok(())
}

/// Show detailed information about a package
pub fn show_package_info(
    package: &str,
    device: Option<String>,
    user_id: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Package: {}\n", package);

    let uad_lists = load_debloat_lists(false).unwrap_or_else(|lists| lists);

    if let Some(pkg_info) = uad_lists.get(package) {
        println!("UAD Information:");
        println!("  List:        {}", pkg_info.list);
        println!("  Removal:     {}", pkg_info.removal);
        println!("  Description: {}", pkg_info.description);
        println!();
    } else {
        println!("  Not found in UAD lists (unlisted package)\n");
    }

    if let Some(device_id) = device {
        let target_device = get_target_device(Some(device_id))?;
        let user = get_user(&target_device, user_id)?;
        println!("Device: {} ({})", target_device.model, target_device.adb_id);
        println!("User:   {}", user.id);

        let state = query_package_state(
            &target_device.adb_id,
            package,
            target_device.package_user_id(user.id)?,
        )?
        .ok_or("Package not found on device")?;
        println!("  State: {}", state);
    }

    Ok(())
}

/// Update UAD package lists from remote repository
pub fn update_lists() -> Result<(), Box<dyn std::error::Error>> {
    println!("Updating UAD package lists from remote repository...");

    match load_debloat_lists(true) {
        Ok(_lists) => {
            println!("✓ Successfully updated package lists");
            Ok(())
        }
        Err(_lists) => Err("Failed to update lists from remote, using cached version".into()),
    }
}

/// Generate shell completion script
pub fn generate_completions(shell: Shell) {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut std::io::stdout());
}
