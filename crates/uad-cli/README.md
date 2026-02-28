# UAD CLI - Universal Android Debloater Command Line Interface

A powerful command-line interface for debloating Android devices, sharing the core functionality with the UAD GUI application.

## Features

- 🚀 **Fast & Efficient** - Direct command execution without GUI overhead
- 📦 **Package Manager-like Interface** - Intuitive commands similar to `apt`, `dnf`, or `pacman`
- 🔍 **Advanced Filtering** - Filter packages by state, removal category, UAD list, or search terms
- 🛡️ **Safety Features** - Dry-run mode, warnings for unsafe packages
- 🔄 **Multiple Actions** - Enable, disable, uninstall, and restore packages
- 🖥️ **Interactive REPL Mode** - Keep device state in memory for faster operations
- 📋 **Shell Completion** - Generate completions for bash, zsh, fish, and more
- 👥 **Multi-user Support** - Target specific Android user profiles

## Installation

Build from source:

```bash
cd /path/to/universal-android-debloater-next-generation
cargo build --release -p uad-cli
```

The binary will be available at `target/release/uad-cli`.

## Usage

### List Connected Devices

```bash
uad devices
```

### List Packages

List all packages on the first connected device:

```bash
uad list
# or use the short alias
uad ls
```

Filter packages by state:

```bash
uad list --state enabled
uad list --state disabled
uad list --state uninstalled
```

Filter by removal category:

```bash
uad list --removal recommended
uad list --removal advanced
uad list --removal expert
uad list --removal unsafe
```

Filter by UAD list:

```bash
uad list --list google
uad list --list oem
uad list --list aosp
```

Search for packages:

```bash
uad list --search "facebook"
uad list -q "chrome"
```

Combine filters:

```bash
uad list --state enabled --removal recommended --search "google"
```

### Show Package Information

```bash
uad info com.android.chrome
uad info com.facebook.katana --device SERIAL123
```

### Uninstall Packages

Uninstall one or more packages:

```bash
uad uninstall com.facebook.katana
uad rm com.facebook.katana com.facebook.services
```

Dry-run to see what would happen:

```bash
uad uninstall com.facebook.katana --dry-run
```

Specify device and user:

```bash
uad uninstall com.facebook.katana --device SERIAL123 --user 0
```

### Enable/Restore Packages

```bash
uad enable com.android.chrome
uad restore com.google.android.gms
```

### Disable Packages

```bash
uad disable com.facebook.katana
```

### Update Package Lists

Update UAD lists from the remote repository:

```bash
uad update
```

### Interactive REPL Mode

Start an interactive session for faster repeated operations:

```bash
uad repl
# or
uad shell
```

Within the REPL:

```
uad> help
uad> list --state enabled
uad> info com.facebook.katana
uad> uninstall com.facebook.katana
uad> enable com.android.chrome
uad> device
uad> exit
```

The REPL mode:
- Keeps device state in memory (faster operations)
- Saves command history (accessible with up/down arrows)
- Loads UAD lists once at startup
- Supports all the same commands as the CLI

### Shell Completion

Generate completion scripts for your shell:

```bash
# Bash
uad completions bash > /usr/share/bash-completion/completions/uad

# Zsh
uad completions zsh > /usr/local/share/zsh/site-functions/_uad

# Fish
uad completions fish > ~/.config/fish/completions/uad.fish

# PowerShell
uad completions powershell > uad.ps1
```

## Command Reference

| Command | Aliases | Description |
|---------|---------|-------------|
| `devices` | - | List connected Android devices |
| `list` | `ls` | List packages with optional filters |
| `info` | - | Show detailed package information |
| `uninstall` | `rm` | Uninstall one or more packages |
| `enable` | `restore` | Enable/restore packages |
| `disable` | - | Disable packages (keeps data) |
| `update` | - | Update UAD package lists |
| `completions` | - | Generate shell completions |
| `repl` | `shell` | Start interactive mode |

## Examples

### Remove all Facebook packages

```bash
uad list --search facebook
uad uninstall com.facebook.katana com.facebook.services --dry-run
# If it looks good:
uad uninstall com.facebook.katana com.facebook.services
```

### Debloat recommended Google apps

```bash
# See what would be uninstalled
uad list --removal recommended --list google

# Uninstall them (you'd list the actual package names)
uad uninstall com.google.package1 com.google.package2
```

### Work with a specific device

```bash
# List devices
uad devices

# Use specific device
uad list --device SERIAL123
uad uninstall com.facebook.katana --device SERIAL123
```

## Safety

- **Dry-run mode**: Always test with `--dry-run` first
- **Warnings**: The CLI warns you about packages marked as "Unsafe"
- **Reversible**: Most operations can be reversed with the `enable` command
- **Multi-user aware**: Respects Android's multi-user system

## Comparison with GUI

### CLI Advantages:
- Faster for power users
- Scriptable and automatable
- Lower resource usage
- Works over SSH
- Better for batch operations
- Can run on Android itself (though not recommended for same-device debloating)

### GUI Advantages:
- Visual feedback
- Easier for beginners
- Package descriptions always visible
- Batch selection with checkboxes

## Requirements

- ADB (Android Debug Bridge) installed and in PATH
- Android device with USB debugging enabled
- Device authorized in ADB (run `adb devices` to check)

## Troubleshooting

**No devices found**
- Ensure USB debugging is enabled on your device
- Run `adb devices` to check if device is authorized
- Try unplugging and replugging the device

**Package not found**
- The package might already be uninstalled
- Check the exact package name with `adb shell pm list packages`
- Try running `uad update` to refresh package lists

**Permission denied**
- Some system packages can't be modified without root
- Some devices have locked bootloaders preventing certain operations

## Contributing

See the main project [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

## License

GPL-3.0 - See [LICENSE](../../LICENSE) for details.

