/// Give each command failure one visible label, preserving the diagnostic text.
pub(crate) fn print_command_error(error: &dyn std::error::Error) {
    let message = error.to_string();
    match message.split_once(':') {
        Some((label, detail)) if label.eq_ignore_ascii_case("error") => eprintln!("Error:{detail}"),
        _ => eprintln!("Error: {message}"),
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
