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
use uad_core::adb::AdbBackend;
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
    /// Built-in ADB implementation (no external dependencies)
    Builtin,
    /// Use system-installed adb binary
    System,
}

impl From<AdbBackendArg> for AdbBackend {
    fn from(arg: AdbBackendArg) -> Self {
        match arg {
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
    /// ADB backend to use: system (default, uses adb binary) or builtin (no dependencies)
    #[arg(
        short = 'B',
        long = "backend",
        value_enum,
        global = true,
        default_value = "system"
    )]
    backend: AdbBackendArg,

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
    let backend: AdbBackend = cli.backend.into();

    match cli.command {
        Commands::Devices => {
            commands::list_devices(backend)?;
        }
        Commands::List {
            device,
            state,
            removal,
            list,
            search,
            user,
        } => {
            commands::list_packages(backend, device, state, removal, list, search, user)?;
        }
        Commands::Uninstall {
            packages,
            device,
            user,
            dry_run,
        } => {
            commands::change_package_state(
                backend,
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
                backend,
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
                backend,
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
            commands::show_package_info(backend, &package, device, user)?;
        }
        Commands::Update => {
            commands::update_lists()?;
        }
        Commands::Adb => {
            commands::show_adb_info(backend)?;
        }
        Commands::Completions { shell } => {
            commands::generate_completions(shell);
        }
        Commands::Repl { device, user } => {
            repl::repl_mode(backend, device, user)?;
        }
    }

    Ok(())
}
