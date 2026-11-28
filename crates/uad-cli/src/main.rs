#![allow(
    clippy::needless_continue,
    clippy::collapsible_if,
    clippy::uninlined_format_args,
    clippy::map_unwrap_or,
    clippy::unnecessary_wraps,
    reason = "Suppress non-critical pedantic/style lints to keep build green"
)]

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::process::ExitCode;
use uad_core::uad_lists::PackageState;

mod commands;
mod device;
mod filters;
mod output;
mod repl;

use filters::{ListFilter, RemovalFilter, StateFilter};

#[derive(Parser)]
#[command(name = "uad")]
#[command(about = "Universal Android Debloater - Command Line Interface", long_about = None)]
#[command(version)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, Default, clap::Args)]
pub struct ActionOptions {
    /// Show the commands without running them
    #[arg(long)]
    dry_run: bool,

    /// Allow disabling or uninstalling Unsafe packages, including a temporary uninstall during Enable recovery
    #[arg(long)]
    allow_unsafe: bool,
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

        #[command(flatten)]
        options: ActionOptions,
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

        #[command(flatten)]
        options: ActionOptions,
    },

    /// Disable packages (clears their data and prevents execution)
    Disable {
        /// Package names to disable
        packages: Vec<String>,

        /// Device serial number (optional, uses first device if not specified)
        #[arg(short, long)]
        device: Option<String>,

        /// User ID (defaults to 0)
        #[arg(short, long)]
        user: Option<u16>,

        #[command(flatten)]
        options: ActionOptions,
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

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            output::print_command_error(err.as_ref());
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

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
            options,
        } => {
            commands::change_package_state(
                &packages,
                device,
                user,
                options,
                PackageState::Uninstalled,
                "Uninstalling",
            )?;
        }
        Commands::Enable {
            packages,
            device,
            user,
            options,
        } => {
            commands::change_package_state(
                &packages,
                device,
                user,
                options,
                PackageState::Enabled,
                "Enabling",
            )?;
        }
        Commands::Disable {
            packages,
            device,
            user,
            options,
        } => {
            commands::change_package_state(
                &packages,
                device,
                user,
                options,
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
        Commands::Completions { shell } => {
            commands::generate_completions(shell);
        }
        Commands::Repl { device, user } => {
            repl::repl_mode(device, user)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
