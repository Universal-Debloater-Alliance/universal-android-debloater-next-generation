#![windows_subsystem = "windows"]

use fern::{
    FormatCallback,
    colors::{Color, ColoredLevelConfig},
};
use log::Record;
use std::{fmt::Arguments, fs::OpenOptions};
use uad_core::CACHE_DIR;

use uad_gui::gui::UadGui;

fn main() -> iced::Result {
    configure_graphics();
    setup_logger().expect("setup logging");
    log_graphics_configuration();
    UadGui::start()
}

/// Graphics defaults for wgpu. A variable the user already set always wins,
/// so `WGPU_BACKEND=gl` or `ICED_BACKEND=tiny-skia` still work as overrides.
fn configure_graphics() {
    let defaults: &[(&str, &str)] = &[
        // Prefer the discrete GPU to prevent crashes on PCs with two GPUs.
        // See #848 and related pull 850.
        ("WGPU_POWER_PREF", "high"),
        // Without this, wgpu probes every backend (Vulkan, DirectX 12 and OpenGL)
        // in one process. Some Windows drivers crash natively while loading
        // Vulkan and OpenGL together (seen with Intel UHD 630 driver
        // 26.20.100.6911: access violation in `ig9icd64.dll`), which kills the app
        // before Iced can fall back to software rendering. DirectX 12 is the
        // native Windows API. Where it is unavailable, Iced still falls back to
        // its `tiny-skia` software renderer.
        #[cfg(target_os = "windows")]
        ("WGPU_BACKEND", "dx12"),
    ];
    for (name, value) in defaults {
        if std::env::var_os(name).is_none() {
            // SAFETY: called first thing in `main`, before the logger, Iced or any
            // other code starts a thread, so nothing can read or write the
            // environment concurrently.
            unsafe { std::env::set_var(name, value) };
        }
    }
}

/// Records the graphics settings, so bug reports show which backend was used.
fn log_graphics_configuration() {
    for name in ["WGPU_BACKEND", "WGPU_POWER_PREF", "ICED_BACKEND"] {
        if let Some(value) = std::env::var_os(name) {
            log::info!("{name}={}", value.to_string_lossy());
        }
    }
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
        // the binary crate (`main.rs`), which logs the graphics configuration
        .level_for("uad_ng", log::LevelFilter::Info)
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

    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
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
