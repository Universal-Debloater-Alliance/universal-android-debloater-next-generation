# Universal Android Debloater Next Generation

> [!warning]
> **DISCLAIMER**: Use at your own risk. We're not responsible for anything that could happen to your devices.

<div align=center>
<img width=75% alt="screenshot of apps view" src=./resources/screenshots/v1.0.2.png>
</div>

This is a detached fork of the [UAD project](https://github.com/0x192/universal-android-debloater). This aims to improve privacy and efficiency (energy, speed, memory) by removing unnecessary and obscure system apps.
This can also improve security by reducing the [attack surface](https://en.wikipedia.org/wiki/Attack_surface). Read [the wiki](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki) for more details.

## Documentation

Everything about UAD-ng (and related stuff) can be found on the Wiki, such as:

- [Getting started](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Getting-started)
- Features of this app
- [Usage guide](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Usage)
- Suggested Android app replacements
- How to get the cutting-edge version by [building the source-code](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Building-from-source)
- Weird things OEMs do
- [Suggested apps for analyzing APKs](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/How-to-contribute#useful-apps)

## Removing related packages

When you remove a package, UAD-ng lists the related packages from the debloat list: packages that need it (they would break anyway) and packages that only it needs. In the GUI, tick "Also remove N related packages" in the review or confirmation dialog. In the CLI, pass `--with-related` to `uninstall` or `disable`. Packages rated `Unsafe` are never added automatically, and the CLI skips removing them unless you pass `--force`.

## Troubleshooting

- **The GUI closes right after start, or shows no window**: on Windows, UAD-ng uses DirectX 12 by default, because some graphics drivers crash when Vulkan and OpenGL are loaded together. If it still fails, set the `WGPU_BACKEND` environment variable to `gl` or `vulkan`, or set `ICED_BACKEND=tiny-skia` for software rendering. A value you set always overrides the default. The chosen backend is written to the log file (`%LOCALAPPDATA%\uad\uadng.log` on Windows).
- **Exports**: "Export current selection" and "Export uninstalled packages" save into the backup folder chosen in Settings. The dialog shows the full path and can open the folder.

## Privacy

UAD-ng does not collect or transmit any user data. The only external connections are `GET` requests to GitHub for fetching the [package list](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/blob/main/resources/assets/uad_lists.json) ([crates/uad-core/src/uad_lists.rs](crates/uad-core/src/uad_lists.rs)) and checking for updates ([crates/uad-core/src/update.rs](crates/uad-core/src/update.rs)).

## Contact

**For real-time communication and support, consider joining our Discord guild:**

[<img width=64em alt="Discord symbol" src=./resources/images/icon_clyde_blurple.svg>](https://discord.gg/CzwbMCPEZa)

**If you prefer using Matrix, we have a bridge to Discord:**

[<img width=64em src=https://matrix.org/images/matrix-favicon.svg>](https://matrix.to/#/#uad-ng:matrix.org)

## Friends

Several projects integrate with or depend on UAD-ng's data and tools:

- **[Canta](https://github.com/samolego/Canta)** — A mobile Android debloater that uses Shizuku for rootless privilege escalation. It integrates with UAD-ng's Universal Debloat List to provide safe package recommendations without requiring a PC.

- **Projects by [Muntashir Akon](https://github.com/MuntashirAkon):**
    - **[AppManager](https://github.com/MuntashirAkon/AppManager)** — An advanced Android app manager with powerful debloating features.
    - **[android-debloat-list](https://github.com/MuntashirAkon/android-debloat-list)** — A comprehensive, community-driven list of Android packages to debloat, based on UAD-ng's list.
- **[De-Bloater](https://github.com/sunilpaulmathew/De-Bloater)** — An application using the power of Magisk to debloat unwanted system apps!

## Special thanks

- [@0x192](https://github.com/0x192) who created the original UAD project.
- [@mawilms](https://github.com/mawilms) for his LotRO plugin manager ([Lembas](https://github.com/mawilms/lembas)) which helped a lot to understand how to use the [Iced](https://github.com/iced-rs/iced) GUI library.
- [@casperstorm](https://github.com/casperstorm) for the UI/UX inspiration.
