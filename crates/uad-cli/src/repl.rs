use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::collections::HashMap;
use std::io::Write;
use uad_core::adb::AdbCommand;
use uad_core::sync::{Phone, User, get_package_state};
use uad_core::uad_lists::{Package, PackageState, load_debloat_lists};

use crate::commands::{
    PackageListContext, display_package_list, execute_with_fallback, expand_with_related,
    plan_state_change, print_package_details,
};
use crate::device::{get_target_device, get_user};
use crate::filters::StateFilter;
use crate::println_ignoring_closed_pipe;

enum ReplAction {
    Continue,
    Exit,
}

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
    // `CACHE_DIR` is created on first use, so saving needs no extra folder setup.
    let history_file = uad_core::CACHE_DIR.join("cli_history.txt");
    let _ = rl.load_history(&history_file);

    loop {
        let readline = rl.readline("uad> ");
        match readline {
            Ok(line) => {
                match handle_repl_line(&line, &mut rl, &target_device, user, user_id, &uad_lists) {
                    Ok(ReplAction::Continue) => {}
                    Ok(ReplAction::Exit) => break,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
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
    let _ = rl.save_history(&history_file);

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
) -> Result<ReplAction, Box<dyn std::error::Error>> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(ReplAction::Continue);
    }

    let _ = rl.add_history_entry(line);

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(ReplAction::Continue);
    }

    match parts[0] {
        "help" => print_repl_help(),
        "exit" | "quit" => {
            println!("Goodbye!");
            return Ok(ReplAction::Exit);
        }
        "list" | "ls" => {
            handle_list_command(&parts[1..], device, user_id, uad_lists)?;
        }
        "info" => {
            handle_info_command(&parts[1..], device, user.id, uad_lists)?;
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

    Ok(ReplAction::Continue)
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
    let system_packages = AdbCommand::new()
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
        println_ignoring_closed_pipe!("  No packages found.");
    } else {
        println_ignoring_closed_pipe!("\nTotal: {} package(s)", displayed_count);
    }

    Ok(())
}

/// Handle info command in REPL
fn handle_info_command(
    args: &[&str],
    device: &Phone,
    user_id: u16,
    uad_lists: &HashMap<String, Package>,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("Usage: info <package_name>");
        return Ok(());
    }

    let package = args[0];
    println!("Package: {}", package);

    if let Some(pkg_info) = uad_lists.get(package) {
        print_package_details(package, pkg_info, uad_lists);
    } else {
        println!("  Not found in UAD lists (unlisted package)");
    }

    let state = get_package_state(&device.adb_id, package, Some(user_id))
        .ok_or("Package not found on device")?;
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
    let force = args.contains(&"--force");
    let with_related = args.contains(&"--with-related");
    let mut packages: Vec<String> = args
        .iter()
        .filter(|arg| !arg.starts_with("--"))
        .map(ToString::to_string)
        .collect();
    if packages.is_empty() {
        eprintln!(
            "Usage: {} <package_name> [package_name...] [--with-related] [--force]",
            action_name.to_lowercase()
        );
        return Ok(());
    }
    if target_state != PackageState::Enabled {
        packages = expand_with_related(packages, uad_lists, device, user, with_related, "  ");
    }

    for pkg_name in &packages {
        process_package_change(
            pkg_name,
            device,
            user,
            target_state,
            action_name,
            uad_lists,
            force,
        )?;
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
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_state = get_package_state(&device.adb_id, pkg_name, Some(user.id))
        .ok_or("Package not found on device")?;

    println!("{} {} (current: {})", action_name, pkg_name, current_state);

    let Some(plan) = plan_state_change(
        pkg_name,
        current_state,
        target_state,
        device,
        user,
        force,
        "  ",
        uad_lists,
    ) else {
        return Ok(());
    };

    execute_with_fallback(
        pkg_name,
        target_state,
        &plan.package,
        user,
        device,
        &plan.commands,
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
    println!("  uninstall <package_name> [package_name...] [--with-related] [--force]");
    println!("      Uninstall one or more packages. --with-related also removes related");
    println!("      packages. --force allows packages rated Unsafe");
    println!("  enable <package_name> [package_name...]");
    println!("      Enable/restore one or more packages");
    println!("  disable <package_name> [package_name...] [--with-related] [--force]");
    println!("      Disable one or more packages (same options as uninstall)");
    println!("  device");
    println!("      Show current device information");
    println!("  clear");
    println!("      Clear the screen");
    println!("  help");
    println!("      Show this help message");
    println!("  exit, quit");
    println!("      Exit the interactive mode");
}
