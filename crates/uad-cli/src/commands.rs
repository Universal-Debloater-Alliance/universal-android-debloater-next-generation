use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use uad_core::adb::{AdbCommand, PmListPacksFlag};
use uad_core::package_text::{matches_search, truncate_description};
use uad_core::relations::{DevicePackage, Relations};
use uad_core::sync::{
    CorePackage, Phone, User, apply_pkg_state_commands, get_devices_list, get_package_state,
    run_adb_shell_action,
};
use uad_core::uad_lists::{Package, PackageState, Removal, UadList, load_debloat_lists};

use crate::device::{NO_DEVICES_FOUND, get_target_device, get_user};
use crate::filters::{ListFilter, RemovalFilter, StateFilter};
use crate::{Cli, print_ignoring_closed_pipe, println_ignoring_closed_pipe};

/// List all connected Android devices
pub fn list_devices() -> Result<(), Box<dyn std::error::Error>> {
    println_ignoring_closed_pipe!("Scanning for connected devices...");
    let devices = get_devices_list();

    if devices.is_empty() {
        return Err(NO_DEVICES_FOUND.into());
    }

    println_ignoring_closed_pipe!("\nFound {} device(s):\n", devices.len());
    for device in &devices {
        println_ignoring_closed_pipe!("  Model:       {}", device.model);
        println_ignoring_closed_pipe!("  Serial:      {}", device.adb_id);
        println_ignoring_closed_pipe!("  Android SDK: {}", device.android_sdk);

        if !device.user_list.is_empty() {
            println_ignoring_closed_pipe!("  Users:       {} user(s)", device.user_list.len());
            for user in &device.user_list {
                let protected = if user.protected { " (protected)" } else { "" };
                println_ignoring_closed_pipe!("               - User ID: {}{}", user.id, protected);
            }
        }
        println_ignoring_closed_pipe!();
    }

    Ok(())
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
    let uad_lists = load_debloat_lists(false).unwrap_or_else(|lists| lists);

    println_ignoring_closed_pipe!(
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

    let pm_flag = state_filter.and_then(StateFilter::to_pm_flag);
    let system_packages = AdbCommand::new()
        .shell(&target_device.adb_id)
        .pm()
        .list_packages_sys(pm_flag, user_id)?;

    let displayed_count = display_package_list(
        &system_packages,
        &uad_lists,
        &target_device.adb_id,
        user_id,
        &context,
    )?;

    if displayed_count == 0 {
        println_ignoring_closed_pipe!("  No packages found matching the specified filters.");
    } else {
        println_ignoring_closed_pipe!("\nTotal: {} package(s)", displayed_count);
    }

    Ok(())
}

/// Display a list of packages with their info
pub fn display_package_list(
    packages: &[String],
    uad_lists: &HashMap<String, Package>,
    device_serial: &str,
    user_id: Option<u16>,
    context: &PackageListContext,
) -> Result<usize, Box<dyn std::error::Error>> {
    let config = context.display_config();
    let mut displayed_count = 0;

    let enabled_packages: HashSet<String> = AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_packages_sys(Some(PmListPacksFlag::OnlyEnabled), user_id)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let disabled_packages: HashSet<String> = AdbCommand::new()
        .shell(device_serial)
        .pm()
        .list_packages_sys(Some(PmListPacksFlag::OnlyDisabled), user_id)
        .unwrap_or_default()
        .into_iter()
        .collect();

    for pkg_name in packages {
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
    print_ignoring_closed_pipe!("[");

    if let Some(info) = pkg_info {
        if config.show_removal {
            print_ignoring_closed_pipe!("{}", info.removal);
            if config.show_state {
                print_ignoring_closed_pipe!(" - ");
            }
        }
        if config.show_state {
            print_ignoring_closed_pipe!("{}", pkg_state);
        }
        print_ignoring_closed_pipe!("] {}", pkg_name);
        if !info.description.is_empty() {
            print_ignoring_closed_pipe!(" - {}", truncate_description(&info.description, 80));
        }
    } else {
        if config.show_removal {
            print_ignoring_closed_pipe!("Unlisted");
            if config.show_state {
                print_ignoring_closed_pipe!(" - ");
            }
        }
        if config.show_state {
            print_ignoring_closed_pipe!("{}", pkg_state);
        }
        print_ignoring_closed_pipe!("] {}", pkg_name);
    }

    println_ignoring_closed_pipe!();
}

/// Options shared by the `uninstall`, `disable` and `enable` commands.
/// The doc comments of the fields are the command-line help.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
pub struct ChangeOptions {
    /// Dry run - show what would be done without actually doing it
    #[arg(long)]
    pub dry_run: bool,
    /// Also remove related packages: packages that need the given ones,
    /// and packages that only the given ones need. Unsafe packages are never added.
    #[arg(long)]
    pub with_related: bool,
    /// Remove packages rated Unsafe (they are skipped otherwise)
    #[arg(long)]
    pub force: bool,
}

/// Change the state of one or more packages
pub fn change_package_state(
    packages: &[String],
    device: Option<String>,
    user_id: Option<u16>,
    options: ChangeOptions,
    target_state: PackageState,
    action_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if packages.is_empty() {
        return Err("No packages specified".into());
    }

    let target_device = get_target_device(device)?;
    let user = get_user(&target_device, user_id)?;
    let uad_lists = load_debloat_lists(false).unwrap_or_else(|lists| lists);
    let dry_run = options.dry_run;

    let mut packages = packages.to_vec();
    if target_state != PackageState::Enabled {
        packages = expand_with_related(
            packages,
            &uad_lists,
            &target_device,
            user,
            options.with_related,
            "",
        );
    }

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

    for pkg_name in &packages {
        process_package_state_change(
            pkg_name,
            &target_device,
            user,
            target_state,
            options,
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

/// Process state change for a single package
/// The device package and the ADB commands that move it to `target_state`.
pub struct PlannedChange {
    pub package: CorePackage,
    pub commands: Vec<String>,
}

/// Plans moving `pkg_name` from `current_state` to `target_state`.
/// Returns `None`, after saying why, when the package is already there or is
/// an `Unsafe` package that `force` does not allow.
#[expect(
    clippy::too_many_arguments,
    reason = "Each value comes from a different caller context, so a struct would be built once per call"
)]
pub fn plan_state_change(
    pkg_name: &str,
    current_state: PackageState,
    target_state: PackageState,
    device: &Phone,
    user: User,
    force: bool,
    indent: &str,
    uad_lists: &HashMap<String, Package>,
) -> Option<PlannedChange> {
    if current_state == target_state {
        println!("{indent}→ Already in target state, skipping");
        return None;
    }

    let pkg_info = uad_lists.get(pkg_name);
    if blocked_as_unsafe(pkg_info, target_state, force, indent) {
        return None;
    }

    let package = CorePackage {
        name: pkg_name.to_string(),
        description: pkg_info.map(|p| p.description.clone()).unwrap_or_default(),
        removal: pkg_info.map(|p| p.removal).unwrap_or(Removal::Unlisted),
        state: current_state,
        list: pkg_info.map(|p| p.list).unwrap_or(UadList::Unlisted),
    };
    let commands = apply_pkg_state_commands(&package, target_state, user, device);
    Some(PlannedChange { package, commands })
}

fn process_package_state_change(
    pkg_name: &str,
    device: &Phone,
    user: User,
    target_state: PackageState,
    options: ChangeOptions,
    uad_lists: &HashMap<String, Package>,
) -> Result<(), Box<dyn std::error::Error>> {
    let dry_run = options.dry_run;
    let current_state = get_package_state(&device.adb_id, pkg_name, Some(user.id))
        .ok_or("Package not found on device")?;

    println!("  {} ({})", pkg_name, current_state);

    let Some(PlannedChange {
        package: core_pkg,
        commands,
    }) = plan_state_change(
        pkg_name,
        current_state,
        target_state,
        device,
        user,
        options.force,
        "    ",
        uad_lists,
    )
    else {
        return Ok(());
    };

    if dry_run {
        for cmd in &commands {
            println!("    Would run: {}", cmd);
        }
    } else {
        execute_with_fallback(
            pkg_name,
            target_state,
            &core_pkg,
            user,
            device,
            &commands,
            "    ",
        )?;
    }

    Ok(())
}

/// Execute commands and verify package state with fallback
pub fn execute_with_fallback(
    package: &str,
    target_state: PackageState,
    core_pkg: &CorePackage,
    user: User,
    device: &Phone,
    commands: &[String],
    indent: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Capture the before-state of packages on other users for cross-user detection
    let before_cross_user_states =
        uad_core::sync::capture_cross_user_states(package, &device.adb_id, user.id, device);

    // Execute commands
    for cmd in commands {
        match run_adb_shell_action(&device.adb_id, cmd.as_str()) {
            Ok(_) => println!("{}✓ {}", indent, cmd),
            Err(e) => {
                return Err(format!("{indent}✗ Failed to execute `{cmd}`: {e:?}").into());
            }
        }
    }

    // Verify package state and attempt fallback if needed
    let actual_state = match get_package_state(&device.adb_id, package, Some(user.id)) {
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
        if let Ok(fallback_action) =
            uad_core::sync::attempt_fallback(core_pkg, target_state, actual_state, user, device)
        {
            println!("{}↻ Fallback: {}", indent, fallback_action);
        } else {
            println!("{}✗ No fallback available", indent);
        }
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

/// Returns `true`, after explaining why, when a removal must be skipped because
/// the package is rated `Unsafe` and `force` is not set. Restoring is never blocked.
pub fn blocked_as_unsafe(
    pkg_info: Option<&Package>,
    target_state: PackageState,
    force: bool,
    indent: &str,
) -> bool {
    if target_state == PackageState::Enabled
        || pkg_info.is_none_or(|info| info.removal != Removal::Unsafe)
    {
        return false;
    }
    println!(
        "{indent}⚠ This package is rated UNSAFE: {}",
        Removal::Unsafe.explanation()
    );
    if force {
        println!("{indent}  Continuing because --force was given.");
        false
    } else {
        println!("{indent}→ Skipped. Pass --force to remove it anyway.");
        true
    }
}

/// Prints the related packages of a removal, warns about packages that may
/// break, and returns `packages`, plus the related ones when `with_related`.
pub fn expand_with_related(
    packages: Vec<String>,
    uad_lists: &HashMap<String, Package>,
    device: &Phone,
    user: User,
    with_related: bool,
    indent: &str,
) -> Vec<String> {
    let relations = Relations::new(uad_lists);
    // Each lookup is an ADB call, so remember the answers.
    let cache: RefCell<HashMap<String, Option<DevicePackage>>> = RefCell::default();
    let lookup = |name: &str| {
        *cache
            .borrow_mut()
            .entry(name.to_string())
            .or_insert_with(|| {
                get_package_state(&device.adb_id, name, Some(user.id)).map(|state| DevicePackage {
                    state,
                    removal: uad_lists.get(name).map_or(Removal::Unlisted, |p| p.removal),
                })
            })
    };
    let plan = relations.plan_removal(&packages, lookup, with_related);

    if !plan.related.is_empty() {
        if with_related {
            println!("{indent}Also removing related packages:");
        } else {
            println!("{indent}Related packages (add --with-related to remove them too):");
        }
        for related in &plan.related {
            println!("{indent}  - {} ({})", related.name, related.reason);
        }
        println!();
    }
    for skipped in &plan.excluded_unsafe {
        println!(
            "{indent}⚠ {} is rated Unsafe ({}), so it is not added automatically.",
            skipped.name, skipped.reason
        );
    }
    for risk in &plan.at_risk {
        println!(
            "{indent}⚠ {} may stop working, because it needs {}.",
            risk.dependent, risk.requirement
        );
    }
    if !plan.excluded_unsafe.is_empty() || !plan.at_risk.is_empty() {
        println!();
    }

    let mut packages = packages;
    if with_related {
        packages.extend(plan.related.into_iter().map(|r| r.name));
    }
    packages
}

/// Prints what a rating means and which listed packages relate to `package`.
pub fn print_package_details(
    package: &str,
    pkg_info: &Package,
    uad_lists: &HashMap<String, Package>,
) {
    println!("  List:        {}", pkg_info.list);
    println!("  Removal:     {}", pkg_info.removal);
    println!("               {}", pkg_info.removal.explanation());
    println!("  Description: {}", pkg_info.description);
    let relations = Relations::new(uad_lists);
    let needs = relations.requirements(package);
    if !needs.is_empty() {
        println!("  Needs:       {}", needs.join(", "));
    }
    let needed_by = relations.dependents(package);
    if !needed_by.is_empty() {
        println!("  Needed by:   {}", needed_by.join(", "));
    }
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
        print_package_details(package, pkg_info, &uad_lists);
        println!();
    } else {
        println!("  Not found in UAD lists (unlisted package)\n");
    }

    if let Some(device_id) = device {
        let target_device = get_target_device(Some(device_id))?;
        let user = get_user(&target_device, user_id)?;
        println!("Device: {} ({})", target_device.model, target_device.adb_id);
        println!("User:   {}", user.id);

        let state = get_package_state(&target_device.adb_id, package, Some(user.id))
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
