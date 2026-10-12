/// Helper macro to handle broken pipe errors gracefully
/// When piping to commands like `head`, we want to exit cleanly when the pipe closes
#[macro_export]
macro_rules! println_or_exit {
    () => {
        if writeln!(std::io::stdout()).is_err() {
            std::process::exit(0);
        }
    };
    ($($arg:tt)*) => {
        if writeln!(std::io::stdout(), $($arg)*).is_err() {
            std::process::exit(0);
        }
    };
}

#[macro_export]
macro_rules! print_or_exit {
    ($($arg:tt)*) => {
        if write!(std::io::stdout(), $($arg)*).is_err() {
            std::process::exit(0);
        }
    };
}
