//! Hands a path or URL to the operating system's default handler.

use log::error;
use std::path::PathBuf;

/// Open a directory, file or URL with the system's default application.
pub fn open_url(dir: PathBuf) {
    const OPENER: &str = match std::env::consts::OS.as_bytes() {
        b"windows" => "explorer",
        b"macos" => "open",
        // "linux"
        _ => "xdg-open",
    };
    match std::process::Command::new(OPENER).arg(dir).output() {
        Ok(o) => {
            // `explorer.exe` exits with code 1 even when it opened the target,
            // so its exit code says nothing about success.
            if !o.status.success() && OPENER != "explorer" {
                // Use lossy conversion for stderr - some systems (like Windows)
                // may output non-UTF8 characters in error messages
                let stderr = String::from_utf8_lossy(&o.stderr);
                let stderr_trimmed = stderr.trim_end();
                if stderr_trimmed.is_empty() {
                    error!("Can't open URL: command failed with no error message");
                } else {
                    error!("Can't open the following URL: {stderr_trimmed}");
                }
            }
        }
        Err(e) => error!("Failed to run command to open the file explorer: {e}"),
    }
}
