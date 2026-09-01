#![allow(
    clippy::needless_continue,
    clippy::collapsible_if,
    clippy::uninlined_format_args,
    clippy::map_unwrap_or,
    clippy::unnecessary_wraps,
    clippy::exit,
    reason = "Suppress non-critical pedantic/style lints to keep build green"
)]

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::process::ExitCode;
use uad_core::adb::{ACommand, AdbBackend, AdbDeviceStatus};
use uad_core::sync::{DeviceDiscoveryIssue, classify_device_issue};
use uad_core::uad_lists::PackageState;

mod commands;
mod device;
mod filters;
mod output;
mod repl;

use filters::{ListFilter, RemovalFilter, StateFilter};

/// CLI-compatible ADB backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AdbBackendArg {
    /// Embedded ADB over USB
    #[cfg(feature = "builtin-adb")]
    Builtin,
    /// Use system-installed adb binary
    System,
}

impl From<AdbBackendArg> for AdbBackend {
    fn from(arg: AdbBackendArg) -> Self {
        match arg {
            #[cfg(feature = "builtin-adb")]
            AdbBackendArg::Builtin => AdbBackend::Builtin,
            AdbBackendArg::System => AdbBackend::System,
        }
    }
}

#[derive(Parser)]
#[command(name = "uad")]
#[command(about = "Universal Android Debloater - Command Line Interface", long_about = None)]
#[command(version)]
#[command(propagate_version = true)]
pub struct Cli {
    /// ADB backend to use
    #[arg(
        short = 'B',
        long = "backend",
        value_enum,
        global = true,
        default_value = "system"
    )]
    backend: AdbBackendArg,

    /// Stop the system ADB server before using the Builtin backend
    #[arg(long, global = true)]
    kill_adb_server: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List connected Android devices
    Devices,

    /// List packages on a device
    #[command(name = "list", visible_alias = "ls")]
    List {
        /// Device serial number (optional, uses first device if not specified)
        #[arg(short, long)]
        device: Option<String>,

        /// Filter by package state
        #[arg(short, long, value_enum)]
        state: Option<StateFilter>,

        /// Filter by removal category
        #[arg(short, long, value_enum)]
        removal: Option<RemovalFilter>,

        /// Filter by UAD list
        #[arg(short, long, value_enum)]
        list: Option<ListFilter>,

        /// Search pattern (matches package name or description)
        #[arg(short = 'q', long)]
        search: Option<String>,

        /// User ID (defaults to 0)
        #[arg(short, long)]
        user: Option<u16>,
    },

    /// Uninstall packages (default removal action)
    #[command(visible_alias = "rm")]
    Uninstall {
        /// Package names to uninstall
        packages: Vec<String>,

        /// Device serial number (optional, uses first device if not specified)
        #[arg(short, long)]
        device: Option<String>,

        /// User ID (defaults to 0)
        #[arg(short, long)]
        user: Option<u16>,

        /// Dry run - show what would be done without actually doing it
        #[arg(long)]
        dry_run: bool,
    },

    /// Restore (reinstall) packages
    #[command(visible_alias = "restore")]
    Enable {
        /// Package names to restore/enable
        packages: Vec<String>,

        /// Device serial number (optional, uses first device if not specified)
        #[arg(short, long)]
        device: Option<String>,

        /// User ID (defaults to 0)
        #[arg(short, long)]
        user: Option<u16>,

        /// Dry run - show what would be done without actually doing it
        #[arg(long)]
        dry_run: bool,
    },

    /// Disable packages (keeps data but prevents execution)
    Disable {
        /// Package names to disable
        packages: Vec<String>,

        /// Device serial number (optional, uses first device if not specified)
        #[arg(short, long)]
        device: Option<String>,

        /// User ID (defaults to 0)
        #[arg(short, long)]
        user: Option<u16>,

        /// Dry run - show what would be done without actually doing it
        #[arg(long)]
        dry_run: bool,
    },

    /// Show detailed information about a package
    Info {
        /// Package name
        package: String,

        /// Device serial number (optional, uses first device if not specified)
        #[arg(short, long)]
        device: Option<String>,

        /// User ID (defaults to 0)
        #[arg(short, long)]
        user: Option<u16>,
    },

    /// Update UAD package lists from remote repository
    Update,

    /// Show ADB backend and version information
    Adb,

    /// Generate shell completion script
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Start interactive REPL mode
    #[command(visible_alias = "shell")]
    Repl {
        /// Device serial number (optional, uses first device if not specified)
        #[arg(short, long)]
        device: Option<String>,

        /// User ID (defaults to 0)
        #[arg(short, long)]
        user: Option<u16>,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    configure_adb(&cli)?;

    match cli.command {
        Commands::Devices => {
            commands::list_devices()?;
        }
        Commands::List {
            device,
            state,
            removal,
            list,
            search,
            user,
        } => {
            commands::list_packages(device, state, removal, list, search, user)?;
        }
        Commands::Uninstall {
            packages,
            device,
            user,
            dry_run,
        } => {
            commands::change_package_state(
                &packages,
                device,
                user,
                dry_run,
                PackageState::Uninstalled,
                "Uninstalling",
            )?;
        }
        Commands::Enable {
            packages,
            device,
            user,
            dry_run,
        } => {
            commands::change_package_state(
                &packages,
                device,
                user,
                dry_run,
                PackageState::Enabled,
                "Enabling",
            )?;
        }
        Commands::Disable {
            packages,
            device,
            user,
            dry_run,
        } => {
            commands::change_package_state(
                &packages,
                device,
                user,
                dry_run,
                PackageState::Disabled,
                "Disabling",
            )?;
        }
        Commands::Info {
            package,
            device,
            user,
        } => {
            commands::show_package_info(&package, device, user)?;
        }
        Commands::Update => {
            commands::update_lists()?;
        }
        Commands::Adb => {
            commands::show_adb_info()?;
        }
        Commands::Completions { shell } => {
            commands::generate_completions(shell);
        }
        Commands::Repl { device, user } => {
            repl::repl_mode(device, user)?;
        }
    }

    Ok(())
}

fn configure_adb(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let backend: AdbBackend = cli.backend.into();
    #[cfg(feature = "builtin-adb")]
    let using_builtin = backend == AdbBackend::Builtin;
    #[cfg(not(feature = "builtin-adb"))]
    let using_builtin = false;

    if cli.kill_adb_server && !using_builtin {
        return Err("--kill-adb-server requires --backend builtin".into());
    }
    if cli.kill_adb_server {
        ACommand::kill_system_server()
            .map_err(|err| format!("Failed to stop the system ADB server: {err}"))?;
    }

    backend.set_current();

    if using_builtin && !cli.kill_adb_server && command_uses_device(&cli.command) {
        let devices = ACommand::new().devices()?;
        match builtin_preflight_issue(&devices) {
            Some(DeviceDiscoveryIssue::Busy) => {
                return Err(
                    "The system ADB server is using the USB device. Stop it with `adb kill-server` \
                     or rerun with `--kill-adb-server`."
                        .into(),
                );
            }
            Some(issue @ DeviceDiscoveryIssue::NoPermissions) => {
                return Err(issue.to_string().into());
            }
            _ => {}
        }
    }

    Ok(())
}

fn builtin_preflight_issue(devices: &[(String, AdbDeviceStatus)]) -> Option<DeviceDiscoveryIssue> {
    if devices
        .iter()
        .any(|(_, status)| status == &AdbDeviceStatus::Device)
    {
        return None;
    }

    classify_device_issue(devices).filter(|issue| {
        matches!(
            issue,
            DeviceDiscoveryIssue::Busy | DeviceDiscoveryIssue::NoPermissions
        )
    })
}

fn command_uses_device(command: &Commands) -> bool {
    match command {
        Commands::Devices
        | Commands::List { .. }
        | Commands::Uninstall { .. }
        | Commands::Enable { .. }
        | Commands::Disable { .. }
        | Commands::Repl { .. } => true,
        Commands::Info { device, .. } => device.is_some(),
        Commands::Update | Commands::Adb | Commands::Completions { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "builtin-adb")]
    #[test]
    fn builtin_server_shutdown_is_explicitly_opted_in() {
        let cli = Cli::try_parse_from([
            "uad",
            "--backend",
            "builtin",
            "--kill-adb-server",
            "devices",
        ])
        .expect("valid Builtin CLI arguments");

        assert_eq!(cli.backend, AdbBackendArg::Builtin);
        assert!(cli.kill_adb_server);
        assert!(command_uses_device(&cli.command));
    }

    #[test]
    fn non_device_commands_skip_usb_preflight() {
        assert!(!command_uses_device(&Commands::Update));
        assert!(!command_uses_device(&Commands::Adb));
    }

    #[test]
    fn builtin_preflight_allows_any_ready_device() {
        let devices = vec![
            ("ready".to_string(), AdbDeviceStatus::Device),
            ("busy".to_string(), AdbDeviceStatus::Busy),
        ];

        assert_eq!(builtin_preflight_issue(&devices), None);
    }

    #[test]
    fn builtin_preflight_reports_blocking_issues_without_a_ready_device() {
        assert_eq!(
            builtin_preflight_issue(&[("busy".to_string(), AdbDeviceStatus::Busy)]),
            Some(DeviceDiscoveryIssue::Busy)
        );
        assert_eq!(
            builtin_preflight_issue(&[("denied".to_string(), AdbDeviceStatus::NoPermissions,)]),
            Some(DeviceDiscoveryIssue::NoPermissions)
        );
        assert_eq!(
            builtin_preflight_issue(&[("pending".to_string(), AdbDeviceStatus::Unauthorized,)]),
            None
        );
    }
}
