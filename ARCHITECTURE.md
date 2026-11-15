# Architecture

Purpose: how the code is organised and why. Read when: adding a module, moving code, or changing dependencies between crates or modules.
Last verified: 2026-11-15.

## Overview

A Cargo workspace with one library and two front ends. Both front ends are local programs: there is no server.

```text
uad-gui (Iced desktop app)  --+
                              +--> uad-core --> `adb` process (USB or TCP device)
uad-cli (clap CLI and REPL) --+          \--> HTTPS: GitHub (debloat list, releases)
```

- `uad-core` owns every side effect: running `adb`, HTTP, the config file, backups and exports.
- The GUI runs blocking `uad-core` calls inside `Task::perform` futures, so they run on the Iced executor, not in `update`.
- The debloat list (`resources/assets/uad_lists.json`) is embedded at build time, refreshed from GitHub at start-up, and cached in `<OS cache dir>/uad/`. A list that fails to parse never replaces the cache.

## Module Map

| Path                                      | Responsibility                                                                                                                                                             | May depend on                        |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| `crates/uad-core/src/adb.rs`              | Typed builder for `adb` commands, output parsing, package-name validation                                                                                                  | std only                             |
| `crates/uad-core/src/sync.rs`             | Devices, users, package states, state-change commands and fallbacks                                                                                                        | `adb`, `uad_lists`                   |
| `crates/uad-core/src/uad_lists.rs`        | Debloat list types and loading (remote, cache, embedded)                                                                                                                   | `http_client`, `relative_time`       |
| `crates/uad-core/src/relations.rs`        | Which packages to remove together, and what breaks                                                                                                                         | `uad_lists`                          |
| `crates/uad-core/src/save.rs`             | Device backups and restore plans, per-device backup folders                                                                                                                | `sync`, `config`                     |
| `crates/uad-core/src/export.rs`           | Selection and uninstalled-package exports (text, CSV)                                                                                                                      | `sync`                               |
| `crates/uad-core/src/config.rs`           | `config.toml` load and save                                                                                                                                                | `save`                               |
| `crates/uad-core/src/update.rs`           | Self-update: release check, checksum-verified download (`self-update` feature)                                                                                             | `http_client`                        |
| `crates/uad-core/src/http_client.rs`      | The only HTTP client builder (private): HTTPS only, timeouts                                                                                                               | `ureq`                               |
| `crates/uad-core/src/package_text.rs`     | Description truncation and search matching for both front ends                                                                                                             | std only                             |
| `crates/uad-core/src/opener.rs`           | Opens a path or URL with the OS default handler                                                                                                                            | std only                             |
| `crates/uad-gui/src/gui.rs`               | Root Iced application: routing, device selection, self-update flow                                                                                                         | views, widgets, `uad-core`           |
| `crates/uad-gui/src/views/list/`          | Package list: state and messages (`mod.rs`), main layout (`view.rs`), dialogs (`dialogs.rs`), ADB actions (`package_actions.rs`), relation lookups (`related_packages.rs`) | widgets, style, `uad-core`           |
| `crates/uad-gui/src/views/settings.rs`    | Settings, backups and restore screen                                                                                                                                       | widgets, `folder_picker`             |
| `crates/uad-gui/src/widgets/`             | Reusable widgets: package row, modal, navigation bar, primary button, export dialog, shaped text                                                                           | style, theme                         |
| `crates/uad-gui/src/theme.rs`, `style.rs` | Palettes (`theme.rs`) and widget styles built from them (`style.rs`)                                                                                                       | iced                                 |
| `crates/uad-cli/src/`                     | Arguments (`main.rs`), commands, REPL, filters, device selection                                                                                                           | `uad-core`                           |
| `scripts/`                                | `npm run dev` launcher, `npm run check` local CI, and `screenshots/` (`npm run screenshots`: capture script and a stand-in `adb` crate outside the workspace)              | Node built-ins, PowerShell, Rust std |

## Dependency Rules

- `uad-gui` and `uad-cli` depend on `uad-core`, never on each other. `uad-core` depends on neither: enforced by Cargo (a cycle does not build).
- Only `uad-core` declares `ureq`, so HTTP cannot start anywhere else: enforced by the crate manifests. Inside `uad-core`, requests go through `http_client::agent`.
- `adb` runs only through `uad_core::adb::AdbCommand` and its sub-builders. Never build a `std::process::Command` for `adb` elsewhere (see the module docs in `adb.rs`).
- A package name reaches a device shell only after `PackageId::new` accepts it (`sync::request_builder` fails closed).
- GUI text uses `crate::widgets::text`, never `iced::widget::text`: enforced by `clippy.toml` (`disallowed-methods`, `disallowed-types`).

## Where New Code Goes

- A new ADB command: a method on the matching builder in `adb.rs`, plus a pure parser with tests.
- A new device action or state rule: `sync.rs`. Shared by both front ends, so never in a view or CLI command.
- A new list field or rule: `uad_lists.rs`, with a test over the embedded list if it is a data rule.
- A new GUI screen: `views/<screen>.rs` (or `views/<screen>/` when it passes about 800 lines), routed from `gui.rs`.
- A new reusable widget: `widgets/<widget_name>.rs`. Colours come from `theme.rs` palettes only.
- A new CLI command: a variant in `main.rs`, shared flags through `TargetArgs` or `ChangeOptions` (`#[command(flatten)]`), logic in `commands.rs`.

## Decisions

- One `uad-core` library for both front ends: device logic and list rules stay identical in the GUI and the CLI.
- `views/list` split into five files by responsibility: the single 2,114-line file mixed state, layout, dialogs and ADB work.
- The GUI `self-update` feature enables `uad-core/self-update`: `no-self-update` builds (Nix, packagers) now leave the updater out of the binary entirely.
- Node tooling (`package.json`) holds only developer tools: Prettier was already the repository formatter but was not pinned. See [RESEARCH.md](RESEARCH.md#developer-tooling-2026-11-15).
- `npm run dev` is the one local launcher: it checks for `cargo` and `adb`, then runs `cargo run -p uad-gui`.

## Allowed Duplication

`npm run lint:duplicates` (jscpd, 50-token minimum, threshold 0) reports no clones in `crates/`. Kept on purpose:

- `uad-cli/src/filters.rs` mirrors `PackageState`, `Removal` and `UadList` as clap `ValueEnum`s: clap needs its own types for parsing and help text.
- `resources/assets/uad_lists.json` is both embedded (`include_str!`) and fetched from GitHub: the embedded copy is the offline fallback.

## Growth Signals

- `views/list/mod.rs` or `view.rs` over about 800 lines, or a new screen sharing its state: split the list state into its own module.
- A third front end, or a front end needing async ADB: put `uad-core` behind a trait so ADB can be faked in tests.
- The debloat list over 8 MiB: the remote download limit in `uad_lists.rs` rejects it.
