# Universal Android Debloater Next Generation

> [!warning]
> **DISCLAIMER**: Use at your own risk. We're not responsible for anything that
could happen to your devices.

<img src="/resources/screenshots/v1.0.2.png" width="850" alt="uad_screenshot">

**Check out the issues, and [feel free to contribute!](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/How-to-contribute)**. We're in **HIGH NEED** of [Rust](https://www.rust-lang.org) developers for fixing critical issues, see [this announcement](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/discussions/731) for more information.

**For real-time communication, consider joining our Discord server:**

<a href="https://discord.gg/CzwbMCPEZa">
  <img src="./resources/images/icon_clyde_blurple_RGB.png" alt="Icon" width="75">
</a>

**In case you prefer using Matrix (using a Matrix bridge to Discord):**

[<img src="https://matrix.org/images/matrix-logo.svg">](https://matrix.to/#/#uad-ng:matrix.org)

## Summary

This is a detached fork of the [UAD project](https://github.com/0x192/universal-android-debloater), which aims to improve privacy and battery performance by removing unnecessary and obscure system apps.
This can also contribute to improving security by reducing (but not eliminating) [the attack surface](https://en.wikipedia.org/wiki/Attack_surface). Read the [wiki](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki) for more details on getting started. Whilst UAD-ng can remove system apps, it cannot detect or remove potentially malicious system services or drivers baked into the firmware of your device by various vendors; some vendor-specific apps are only UI front-ends to vendor-provided system services, and as such disabling/uninstalling those apps will not stop a service from running. Additional information can be found in package descriptions inside the Universal Android Debloater Next Generation application.

## Documentation

For documentation regarding how to use UAD-ng, the FAQ, building from source and how to decompile/extract APKs, see [our Wiki](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki).

## Special thanks

- [@0x192](https://github.com/0x192) who created the original UAD project.
- [@mawilms](https://github.com/mawilms) for his LotRO plugin manager ([Lembas](https://github.com/mawilms/lembas)) which helped a lot to understand how to use the [Iced](https://github.com/hecrj/iced) GUI library.
- [@casperstorm](https://github.com/casperstorm) for the UI/UX inspiration.
