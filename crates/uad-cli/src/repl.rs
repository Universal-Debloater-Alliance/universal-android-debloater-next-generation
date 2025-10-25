use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::collections::HashMap;
use std::io::Write;
use uad_core::adb::ACommand;
use uad_core::sync::{CorePackage, Phone, User, apply_pkg_state_commands, get_package_state};
use uad_core::uad_lists::{Package, PackageState, Removal, UadList, load_debloat_lists};

use crate::commands::{PackageListContext, display_package_list, execute_with_fallback};
use crate::device::{get_target_device, get_user};
use crate::filters::StateFilter;
use crate::println_or_exit;

/// Start interactive REPL mode
pub fn repl_mode(
    device: Option<String>,
    user_id: Option<u16>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Universal Android Debloater - Interactive Mode");
    println!("Type 'help' for available commands, 'exit' or 'quit' to leave\n");

    let target_device = get_target_device(device)?;
    let user = get_user(&target_device, user_id)?;

    println!(
        "Connected to: {} ({})",
        target_device.model, target_device.adb_id
    );
    println!("User: {}\n", user.id);

    let uad_lists = load_debloat_lists(false).unwrap_or_else(|lists| lists);
    let mut rl = DefaultEditor::new()?;

    // Try to load history
    let history_file = dirs::cache_dir().map(|d| d.join("uad").join("cli_history.txt"));
    if let Some(ref path) = history_file {
        let _ = rl.load_history(path);
    }

    loop {
        let readline = rl.readline("uad> ");
        match readline {
            Ok(line) => {
                if let Err(e) =
                    handle_repl_line(&line, &mut rl, &target_device, user, user_id, &uad_lists)
                {
                    if e.to_string() == "exit" {
                        break;
                    }
                    eprintln!("Error: {}", e);
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Interrupted (Ctrl-C). Type 'exit' to quit.");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("Goodbye!");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    if let Some(ref path) = history_file {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = rl.save_history(path);
    }

    Ok(())
}

/// Handle a single line of REPL input
fn handle_repl_line(
    line: &str,
    rl: &mut DefaultEditor,
    device: &Phone,
    user: User,
    user_id: Option<u16>,
    uad_lists: &HashMap<String, Package>,
) -> Result<(), Box<dyn std::error::Error>> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }

    let _ = rl.add_history_entry(line);

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    match parts[0] {
        "help" => print_repl_help(),
        "exit" | "quit" => {
            println!("Goodbye!");
            return Err("exit".into());
        }
        "list" | "ls" => {
            handle_list_command(&parts[1..], device, user_id, uad_lists)?;
        }
        "info" => {
            handle_info_command(&parts[1..], device, uad_lists)?;
        }
        "uninstall" | "rm" => {
            handle_state_change_command(
                &parts[1..],
                device,
                user,
                PackageState::Uninstalled,
                "Uninstalling",
                uad_lists,
            )?;
        }
        "enable" | "restore" => {
            handle_state_change_command(
                &parts[1..],
                device,
                user,
                PackageState::Enabled,
                "Enabling",
                uad_lists,
            )?;
        }
        "disable" => {
            handle_state_change_command(
                &parts[1..],
                device,
                user,
                PackageState::Disabled,
                "Disabling",
                uad_lists,
            )?;
        }
        "device" => {
            println!(
                "Device: {} ({}), Android SDK: {}, User: {}",
                device.model, device.adb_id, device.android_sdk, user.id
            );
        }
        "clear" => {
            print!("\x1B[2J\x1B[1;1H");
        }
        _ => {
            eprintln!(
                "Unknown command: '{}'. Type 'help' for available commands.",
                parts[0]
            );
        }
    }

    Ok(())
}

/// Parse REPL arguments into filters
struct ReplListArgs {
    state_filter: Option<StateFilter>,
    search_term: Option<String>,
}

impl ReplListArgs {
    fn parse(args: &[&str]) -> Result<Self, String> {
        let mut state_filter = None;
        let mut search_term = None;
        let mut i = 0;

        while i < args.len() {
            match args[i] {
                "--state" | "-s" => {
                    if i + 1 >= args.len() {
                        return Err("--state requires a value".to_string());
                    }
                    state_filter = match args[i + 1].to_lowercase().as_str() {
                        "enabled" => Some(StateFilter::Enabled),
                        "disabled" => Some(StateFilter::Disabled),
                        "uninstalled" => Some(StateFilter::Uninstalled),
                        "all" => Some(StateFilter::All),
                        _ => return Err(format!("Invalid state: {}", args[i + 1])),
                    };
                    i += 2;
                }
                "--search" | "-q" => {
                    if i + 1 >= args.len() {
                        return Err("--search requires a value".to_string());
                    }
                    search_term = Some(args[i + 1].to_string());
                    i += 2;
                }
                _ => {
                    return Err(format!("Unknown option: {}", args[i]));
                }
            }
        }

        Ok(Self {
            state_filter,
            search_term,
        })
    }
}

/// Handle list command in REPL
fn handle_list_command(
    args: &[&str],
    device: &Phone,
    user_id: Option<u16>,
    uad_lists: &HashMap<String, Package>,
) -> Result<(), Box<dyn std::error::Error>> {
    let parsed = ReplListArgs::parse(args)?;

    let pm_flag = parsed.state_filter.and_then(StateFilter::to_pm_flag);
    let system_packages = ACommand::new()
        .shell(&device.adb_id)
        .pm()
        .list_packages_sys(pm_flag, user_id)?;

    let context = PackageListContext {
        state_filter: parsed.state_filter,
        removal_filter: None,
        list_filter: None,
        search: parsed.search_term,
    };

    let displayed_count = display_package_list(
        &system_packages,
        uad_lists,
        &device.adb_id,
        user_id,
        &context,
    )?;

    if displayed_count == 0 {
        println_or_exit!("  No packages found.");
    } else {
        println_or_exit!("\nTotal: {} package(s)", displayed_count);
    }

    Ok(())
}

/// Handle info command in REPL
fn handle_info_command(
    args: &[&str],
    device: &Phone,
    uad_lists: &HashMap<String, Package>,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: info <package_name>");
        return Ok(());
    }

    let package = args[0];
    println!("Package: {}", package);

    if let Some(pkg_info) = uad_lists.get(package) {
        println!("  List:        {}", pkg_info.list);
        println!("  Removal:     {}", pkg_info.removal);
        println!("  Description: {}", pkg_info.description);
    } else {
        println!("  Not found in UAD lists (unlisted package)");
    }

    let state =
        get_package_state(&device.adb_id, package, None).ok_or("Package not found on device")?;
    println!("  State:       {}", state);

    Ok(())
}

/// Handle state change command in REPL
fn handle_state_change_command(
    args: &[&str],
    device: &Phone,
    user: User,
    target_state: PackageState,
    action_name: &str,
    uad_lists: &HashMap<String, Package>,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!(
            "Usage: {} <package_name> [package_name...]",
            action_name.to_lowercase()
        );
        return Ok(());
    }

    for pkg_name in args {
        process_package_change(pkg_name, device, user, target_state, action_name, uad_lists)?;
    }

    Ok(())
}

/// Process state change for a single package in REPL
fn process_package_change(
    pkg_name: &str,
    device: &Phone,
    user: User,
    target_state: PackageState,
    action_name: &str,
    uad_lists: &HashMap<String, Package>,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_state = get_package_state(&device.adb_id, pkg_name, Some(user.id))
        .ok_or("Package not found on device")?;

    println!("{} {} (current: {})", action_name, pkg_name, current_state);

    if current_state == target_state {
        println!("  → Already in target state, skipping");
        return Ok(());
    }

    let pkg_info = uad_lists.get(pkg_name);
    if let Some(info) = pkg_info {
        if info.removal == Removal::Unsafe {
            println!("  ⚠ WARNING: This package is marked as UNSAFE to remove!");
        }
    }

    let core_pkg = CorePackage {
        name: pkg_name.to_string(),
        description: pkg_info.map(|p| p.description.clone()).unwrap_or_default(),
        removal: pkg_info.map(|p| p.removal).unwrap_or(Removal::Unlisted),
        state: current_state,
        list: pkg_info.map(|p| p.list).unwrap_or(UadList::Unlisted),
    };

    let commands = apply_pkg_state_commands(&core_pkg, target_state, user, device);

    execute_with_fallback(
        pkg_name,
        target_state,
        &core_pkg,
        user,
        device,
        &commands,
        "  ",
    )
}

/// Print REPL help message
fn print_repl_help() {
    println!("Available commands:");
    println!("  list [--state <state>] [--search <term>]");
    println!("      List packages with optional filters");
    println!("  info <package_name>");
    println!("      Show information about a package");
    println!("  uninstall <package_name> [package_name...]");
    println!("      Uninstall one or more packages");
    println!("  enable <package_name> [package_name...]");
    println!("      Enable/restore one or more packages");
    println!("  disable <package_name> [package_name...]");
    println!("      Disable one or more packages");
    println!("  device");
    println!("      Show current device information");
    println!("  clear");
    println!("      Clear the screen");
    println!("  help");
    println!("      Show this help message");
    println!("  exit, quit");
    println!("      Exit the interactive mode");
}
