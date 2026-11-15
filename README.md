# Universal Android Debloater Next Generation

> [!warning]
> **DISCLAIMER**: Use at your own risk. We're not responsible for anything that could happen to your devices.

<div align=center>
<img width=75% alt="UAD-ng package list in the Dark theme, with one package selected and its description shown" src=./resources/screenshots/package-list-dark-950.png>
</div>

This is a detached fork of the [UAD project](https://github.com/0x192/universal-android-debloater). This aims to improve privacy and efficiency (energy, speed, memory) by removing unnecessary and obscure system apps.
This can also improve security by reducing the [attack surface](https://en.wikipedia.org/wiki/Attack_surface). Read [the wiki](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki) for more details.

UAD-ng has a desktop app (`uad-ng`) and a command-line tool (`uad`). Both talk to the phone through [ADB](https://developer.android.com/tools/adb) over USB or Wi-Fi. No root is needed.

## Quick Start

1. Install Android platform-tools, so `adb` is on your PATH, and turn on USB debugging on the phone ([Getting started](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Getting-started)).
2. Download `uad-ng` for your system from the [latest release](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/releases/latest) and run it. Each file has a matching SHA-256 file to check the download.
3. Connect the phone, accept the debugging prompt on the phone, and press the refresh button in UAD-ng.

To run the newest code instead, install [Rust](https://rustup.rs) and [Node.js](https://nodejs.org) 20 or later, then in the repository root run `npm run dev`. It builds and opens the app (the first build takes a few minutes). Ctrl+C stops it.

## Screenshots

Taken from the current code (v1.3.1) on Windows with a simulated Samsung phone. Package names and descriptions come from the real debloat list.

The window opens at 950 x 700 and can be resized down to 900 x 560. The package list and its details panel grow with the window:

| Minimum (900 x 560)                                                                                       | Default (950 x 700)                                                                                       | Wide (1440 x 800)                                                                                |
| --------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| <img alt="Package list at the minimum window size" src=./resources/screenshots/package-list-dark-900.png> | <img alt="Package list at the default window size" src=./resources/screenshots/package-list-dark-950.png> | <img alt="Package list in a wide window" src=./resources/screenshots/package-list-dark-1440.png> |

| Light theme                                                                                        | Settings                                                                                                            | About                                                                                              |
| -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| <img alt="Package list in the Light theme" src=./resources/screenshots/package-list-light-950.png> | <img alt="Settings screen with theme, backup and device options" src=./resources/screenshots/settings-dark-950.png> | <img alt="About screen with update and help links" src=./resources/screenshots/about-dark-950.png> |

## Documentation

The wiki covers UAD-ng in depth, including:

- [Getting started](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Getting-started)
- Features of this app
- [Usage guide](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Usage)
- Suggested Android app replacements
- How to get the cutting-edge version by [building the source-code](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Building-from-source)
- Weird things OEMs do
- [Suggested apps for analyzing APKs](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/How-to-contribute#useful-apps)

In this repository:

| File                               | Answers                                                     |
| ---------------------------------- | ----------------------------------------------------------- |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How do I add packages or code, and how are releases made?   |
| [ARCHITECTURE.md](ARCHITECTURE.md) | How is the code organised, and where does new code go?      |
| [DESIGN.md](DESIGN.md)             | How does the GUI look and behave?                           |
| [SECURITY.md](SECURITY.md)         | How is it protected, how do I report a vulnerability?       |
| [RESEARCH.md](RESEARCH.md)         | What evidence backs the tooling and dependency choices?     |
| [AGENTS.md](AGENTS.md)             | Commands and conventions for coding agents and contributors |

## Removing related packages

When you remove a package, UAD-ng lists the related packages from the debloat list: packages that need it (they would break anyway) and packages that only it needs. In the GUI, tick "Also remove N related packages" in the review or confirmation dialog. In the CLI, pass `--with-related` to `uninstall` or `disable`. Packages rated `Unsafe` are never added automatically, and the CLI skips removing them unless you pass `--force`.

## Development

| Command                          | What it does                                                                         |
| -------------------------------- | ------------------------------------------------------------------------------------ |
| `npm run dev`                    | Builds and starts the GUI in debug mode                                              |
| `cargo run -p uad-cli -- --help` | Builds the CLI and shows its commands                                                |
| `npm ci`                         | Installs the formatting and lint tools (once)                                        |
| `npm run check`                  | Runs every CI check locally: formatting, names, duplicates, Clippy, builds, tests    |
| `npm run screenshots`            | Retakes the README screenshots with a simulated phone (Windows, see CONTRIBUTING.md) |
| `npm run format`                 | Formats Markdown, JSON and YAML with Prettier (Rust: `cargo fmt --all`)              |

## Troubleshooting

- **The GUI closes right after start, or shows no window**: on Windows, UAD-ng uses DirectX 12 by default, because some graphics drivers crash when Vulkan and OpenGL are loaded together. If it still fails, set the `WGPU_BACKEND` environment variable to `gl` or `vulkan`, or set `ICED_BACKEND=tiny-skia` for software rendering. A value you set always overrides the default. The chosen backend is written to the log file (`%LOCALAPPDATA%\uad\uadng.log` on Windows).
- **No device appears**: run `adb devices`. If the phone shows `unauthorized`, accept the prompt on the phone. If `adb` is not found, install platform-tools and add them to PATH.
- **Exports**: "Export current selection" and "Export uninstalled packages" save into the backup folder chosen in Settings. The dialog shows the full path and can open the folder.
- **`npm run dev` says `cargo` is not found**: install Rust from [rustup.rs](https://rustup.rs) and open a new terminal.

## Privacy

UAD-ng does not collect or transmit any user data. The only external connections are HTTPS `GET` requests to GitHub for fetching the [package list](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/blob/main/resources/assets/uad_lists.json) ([crates/uad-core/src/uad_lists.rs](crates/uad-core/src/uad_lists.rs)), checking for updates, and downloading an update you start ([crates/uad-core/src/update.rs](crates/uad-core/src/update.rs)).

## Contact

**For real-time communication and support, consider joining our Discord guild:**

[<img width=64em alt="Discord symbol" src=./resources/images/icon_clyde_blurple.svg>](https://discord.gg/CzwbMCPEZa)

**If you prefer using Matrix, we have a bridge to Discord:**

[<img width=64em src=https://matrix.org/images/matrix-favicon.svg>](https://matrix.to/#/#uad-ng:matrix.org)

## Friends

Several projects integrate with or depend on UAD-ng's data and tools:

- **[Canta](https://github.com/samolego/Canta)**: a mobile Android debloater that uses Shizuku for rootless privilege escalation. It integrates with UAD-ng's Universal Debloat List to provide safe package recommendations without requiring a PC.

- **Projects by [Muntashir Akon](https://github.com/MuntashirAkon):**
    - **[AppManager](https://github.com/MuntashirAkon/AppManager)**: an advanced Android app manager with powerful debloating features.
    - **[android-debloat-list](https://github.com/MuntashirAkon/android-debloat-list)**: a comprehensive, community-driven list of Android packages to debloat, based on UAD-ng's list.
- **[De-Bloater](https://github.com/sunilpaulmathew/De-Bloater)**: an application using the power of Magisk to debloat unwanted system apps!

## Special thanks

- [@0x192](https://github.com/0x192) who created the original UAD project.
- [@mawilms](https://github.com/mawilms) for his LotRO plugin manager ([Lembas](https://github.com/mawilms/lembas)) which helped a lot to understand how to use the [Iced](https://github.com/iced-rs/iced) GUI library.
- [@casperstorm](https://github.com/casperstorm) for the UI/UX inspiration.
