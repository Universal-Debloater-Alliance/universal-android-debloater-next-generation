#![windows_subsystem = "windows"]

use fern::{
    FormatCallback,
    colors::{Color, ColoredLevelConfig},
};
use log::Record;
use std::sync::LazyLock;
use std::{fmt::Arguments, fs::OpenOptions, path::PathBuf};
use uad_core::utils::setup_uad_dir;

use uad_gui::gui::UadGui;

static CACHE_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| setup_uad_dir(&dirs::cache_dir().expect("Can't detect cache dir")));

fn main() -> iced::Result {
    // Safety: This function is safe to call in a single-threaded program.
    // The exact requirement is: you must ensure that there are no other threads concurrently writing or
    // reading(!) the environment through functions or global variables other than the ones in this module.
    unsafe {
        // Force WGPU/Iced to use discrete GPU to prevent crashes on PCs with two GPUs.
        // See #848 and related pull 850.
        std::env::set_var("WGPU_POWER_PREF", "high");
    }

    setup_logger().expect("setup logging");
    UadGui::start()
}

/// Sets up logging to a new file in `CACHE_DIR"/uadng.log"`
/// Also attaches the terminal on Windows machines
/// '''
/// match `setup_logger().expect("Error` setting up logger")
/// '''
fn setup_logger() -> Result<(), fern::InitError> {
    #[cfg(target_os = "windows")]
    {
        attach_windows_console();
    }

    let colors = ColoredLevelConfig::new().info(Color::Green);

    let make_formatter = |use_colors: bool| {
        move |out: FormatCallback, message: &Arguments, record: &Record| {
            out.finish(format_args!(
                "{} {} [{}:{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                if use_colors {
                    format!("{:5}", colors.color(record.level()))
                } else {
                    format!("{:5}", record.level().to_string())
                },
                record.file().unwrap_or("?"),
                record.line().map(|l| l.to_string()).unwrap_or_default(),
                message
            ));
        }
    };

    let default_log_level = log::LevelFilter::Warn;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .truncate(false)
        .open(CACHE_DIR.join("uadng.log"))?;

    let file_dispatcher = fern::Dispatch::new()
        .format(make_formatter(false))
        .level(default_log_level)
        // Rust compiler makes module names use _ instead of -
        .level_for("uad_gui", log::LevelFilter::Debug)
        .chain(log_file);

    let stdout_dispatcher = fern::Dispatch::new()
        .format(make_formatter(true))
        .level(default_log_level)
        // Rust compiler makes module names use _ instead of -
        .level_for("uad_gui", log::LevelFilter::Warn)
        .chain(std::io::stdout());

    fern::Dispatch::new()
        .chain(stdout_dispatcher)
        .chain(file_dispatcher)
        .apply()?;

    Ok(())
}

/// (Windows) Allow the application to display logs to the terminal
/// regardless if it was compiled with `windows_subsystem = "windows"`.
///
/// This is excluded on non-Windows targets.
#[cfg(target_os = "windows")]
fn attach_windows_console() {
    use win32console::console::WinConsole;

    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
    let _ = WinConsole::attach_console(ATTACH_PARENT_PROCESS);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn init_logger() {
        match setup_logger() {
            Ok(()) => (),
            Err(error) => panic!("Error: {error}"),
        }
    }
}
