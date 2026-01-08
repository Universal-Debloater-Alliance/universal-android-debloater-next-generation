# Universal Android Debloater Next Generation

> [!warning]
> **DISCLAIMER**: Use at your own risk. We're not responsible for anything that could happen to your devices.

<div align=center>
<img width=75% alt="screenshot of apps view" src=./resources/screenshots/v1.0.2.png>
</div>

This is a detached fork of the [UAD project](https://github.com/0x192/universal-android-debloater). This aims to improve privacy and efficiency (energy, speed, memory) by removing unnecessary and obscure system apps.
This can also improve security by reducing the [attack surface](https://en.wikipedia.org/wiki/Attack_surface). Read [the wiki](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki) for more details.

## Documentation

Everything about UAD-ng (and related stuff) can be found on [the Wiki](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki), such as:

- Features of this app
- [Usage guide](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Usage)
- Suggested Android app replacements
- How to get the cutting-edge version by [building the source-code](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/Building-from-source)
- Weird things OEMs do
- [Suggested apps for analyzing APKs](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/How-to-contribute#useful-apps)

## Privacy

UAD-ng does not collect or transmit any user data. The only external connections are `GET` requests to GitHub for fetching the [package list](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/blob/main/resources/assets/uad_lists.json) ([src/core/uad_lists.rs#L210](src/core/uad_lists.rs#L210)) and checking for updates ([src/core/update.rs#L178](src/core/update.rs#L178)).

## Contact

**For real-time communication and support, consider joining our Discord guild:**

[<img width=64em alt="Discord symbol" src=./resources/images/icon_clyde_blurple.svg>](https://discord.gg/CzwbMCPEZa)

**If you prefer using Matrix, we have a bridge to Discord:**

[<img width=64em src=https://matrix.org/images/matrix-favicon.svg>](https://matrix.to/#/#uad-ng:matrix.org)

## Special thanks

- [@0x192](https://github.com/0x192) who created the original UAD project.
- [@mawilms](https://github.com/mawilms) for his LotRO plugin manager ([Lembas](https://github.com/mawilms/lembas)) which helped a lot to understand how to use the [Iced](https://github.com/iced-rs/iced) GUI library.
- [@casperstorm](https://github.com/casperstorm) for the UI/UX inspiration.
