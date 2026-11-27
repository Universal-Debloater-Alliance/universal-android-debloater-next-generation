use uad_core::sync::AdbError;

/// Give each command failure one visible label, preserving the diagnostic text.
pub(crate) fn print_command_error(error: &(dyn std::error::Error + 'static)) {
    eprintln!("{}", command_error_message(error));
}

fn command_error_message(error: &(dyn std::error::Error + 'static)) -> String {
    let message = if let Some(adb) = error.downcast_ref::<AdbError>()
        && let Some(advice) = &adb.advice
    {
        format!(
            "{}{}\n{}\nTip: {}",
            adb.context, adb.diagnostic, advice.explanation, advice.tip
        )
    } else {
        error.to_string()
    };
    match message.split_once(':') {
        Some((label, detail)) if label.eq_ignore_ascii_case("error") => format!("Error:{detail}"),
        _ => format!("Error: {message}"),
    }
}

/// Helper macro to handle broken pipe errors gracefully
/// When piping to commands like `head`, we want to exit cleanly when the pipe closes
#[macro_export]
macro_rules! println_or_exit {
    () => {
        let _ = writeln!(std::io::stdout());
    };
    ($($arg:tt)*) => {
        let _ = writeln!(std::io::stdout(), $($arg)*);
    };
}

#[macro_export]
macro_rules! print_or_exit {
    ($($arg:tt)*) => {
        let _ = write!(std::io::stdout(), $($arg)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_errors_keep_the_android_reply_first_and_normalize_only_the_leading_label() {
        let error = AdbError::from_action(
            "pm enable com.example.app",
            "ERROR: Permission denied\n  Android detail".to_owned(),
        );
        let explanation = "Insufficient privileges to perform this action.\nTip: This may require root access or the package is protected by the system.";
        assert_eq!(
            command_error_message(&error),
            format!("Error: Permission denied\n  Android detail\n{explanation}")
        );
        assert_eq!(
            command_error_message(&error.with_context("Failed to reinstall package")),
            format!(
                "Error: Failed to reinstall package: ERROR: Permission denied\n  Android detail\n{explanation}"
            )
        );
    }
}
