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
