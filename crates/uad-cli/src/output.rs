/// `println!` that ignores write errors, so piping into a command such as `head`
/// that closes the pipe early does not panic with a broken-pipe error.
#[macro_export]
macro_rules! println_ignoring_closed_pipe {
    () => {
        let _ = writeln!(std::io::stdout());
    };
    ($($arg:tt)*) => {
        let _ = writeln!(std::io::stdout(), $($arg)*);
    };
}

/// `print!` that ignores write errors. See `println_ignoring_closed_pipe`.
#[macro_export]
macro_rules! print_ignoring_closed_pipe {
    ($($arg:tt)*) => {
        let _ = write!(std::io::stdout(), $($arg)*);
    };
}
