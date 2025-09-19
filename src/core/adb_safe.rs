use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum AdbError {
    #[error("ADB failed to start: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("ADB returned non-success status ({code}): {msg}")]
    NonZero { code: i32, msg: String },
}

#[derive(Debug, Clone)]
pub struct AdbOutput {
    pub stdout: String,
    pub stderr: String,
}

fn run_adb(args: &[&str]) -> Result<AdbOutput, AdbError> {
    let output = Command::new("adb").args(args).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let mut msg = stderr.trim().to_owned();
        if msg.is_empty() && !stdout.trim().is_empty() {
            msg = stdout.trim().to_owned();
        }
        return Err(AdbError::NonZero { code, msg });
    }

    Ok(AdbOutput { stdout, stderr })
}

/// pm uninstall --user <user> <package>
pub fn uninstall_for_user(pkg: &str, user: u32) -> Result<AdbOutput, AdbError> {
    run_adb(&["shell", "pm", "uninstall", "--user", &user.to_string(), pkg])
}

/// Nice hints for common vendor messages
pub fn friendly_hint(err_msg: &str) -> Option<&'static str> {
    let e = err_msg;
    if e.contains("DELETE_FAILED_USER_RESTRICTED") || e.contains("package is protected") {
        Some("This package is protected by the vendor. Try Disable instead.")
    } else if e.contains("NOT_INSTALLED_FOR_USER") {
        Some("It's already gone for this user. Refresh the list.")
    } else if e.contains("Shell does not have permission to access user") {
        Some("Wrong user/profile. Use the primary user or a permitted profile.")
    } else {
        None
    }
}
