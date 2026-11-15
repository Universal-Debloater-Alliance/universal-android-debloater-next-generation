//! Stand-in `adb` that simulates one Samsung phone, used only to capture README screenshots.
//! `scripts/screenshots/capture-screenshots.ps1` builds it and puts it first on PATH.
//! It answers the commands UAD-ng sends and reports success for every package action.

/// One `<state> <package>` per line (`e` enabled, `d` disabled, `u` uninstalled). `#` lines are comments.
const PACKAGES: &str = include_str!("packages.txt");

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `-s <serial>` selects the device. There is only one.
    if args.first().map(String::as_str) == Some("-s") {
        args.drain(..2.min(args.len()));
    }
    // Shell actions arrive as one argument, such as "pm uninstall --user 0 <package>".
    let words: Vec<&str> = args.iter().flat_map(|arg| arg.split_whitespace()).collect();
    match words.as_slice() {
        // The debug build of UAD-ng asserts this four-line format.
        ["version", ..] => println!(
            "Android Debug Bridge version 1.0.41\nVersion 35.0.2-12147458\nInstalled as C:\\platform-tools\\adb.exe\nRunning on Windows 10.0.26300"
        ),
        ["devices", ..] => println!("List of devices attached\nR5CX20DEMO\tdevice"),
        ["shell", "getprop", "ro.product.brand"] => println!("samsung"),
        ["shell", "getprop", "ro.product.model"] => println!("SM-S921B"),
        ["shell", "getprop", "ro.build.version.sdk"] => println!("34"),
        ["shell", "getprop", ..] => println!(),
        ["shell", "pm", "list", "users"] => println!("Users:\n\tUserInfo{{0:Owner:c13}} running"),
        ["shell", "pm", "list", "packages", rest @ ..] => {
            let flag = rest.iter().find(|word| ["-u", "-e", "-d"].contains(word));
            let packages = PACKAGES
                .lines()
                .filter(|line| !line.starts_with('#'))
                .filter_map(|line| line.split_once(' '));
            for (state, name) in packages {
                let listed = match flag.copied() {
                    Some("-u") => true,
                    Some("-e") => state == "e",
                    Some("-d") => state == "d",
                    _ => state != "u",
                };
                if listed {
                    println!("package:{name}");
                }
            }
        }
        ["shell", ..] => println!("Success"),
        _ => {}
    }
}
