# Agent Guide

Purpose: rules and commands every agent task in this repository needs. Read when: always.
Last verified: 2026-11-15.

## Commands

- Run the GUI: `npm run dev` (needs Rust, and `adb` on PATH to see devices)
- Run the CLI: `cargo run -p uad-cli -- --help`
- Install dev tools (Prettier, ls-lint, jscpd): `npm ci`
- Full local CI, same as GitHub: `npm run check`
- Tests: `cargo test --workspace --all-features`. One crate: `cargo test -p uad-core`. One test: `cargo test -p uad-core device_backup_dir`
- Lint: `cargo clippy --workspace --all-targets --all-features -- -D clippy::all -W clippy::style`
- Format: `cargo fmt --all` and `npm run format`
- Release build: `cargo build --release -p uad-gui` (`--no-default-features --features wgpu,no-self-update,img` without the updater)

## Conventions

| Kind                       | Convention                                          | Example                           | Exceptions                                                       |
| -------------------------- | --------------------------------------------------- | --------------------------------- | ---------------------------------------------------------------- |
| Directories                | kebab-case                                          | `crates/uad-core/`                | `.github/ISSUE_TEMPLATE/`, `PULL_REQUEST_TEMPLATE/` (GitHub)     |
| Rust files and modules     | snake_case, named by responsibility                 | `package_text.rs`                 | none                                                             |
| Rust types, functions      | rustc defaults (PascalCase types, snake_case items) | `AdbCommand`, `device_backup_dir` | Iced style functions in `style.rs` are PascalCase (Iced classes) |
| Root docs                  | UPPERCASE                                           | `ARCHITECTURE.md`                 | none                                                             |
| Scripts, config, workflows | kebab-case                                          | `scripts/run-dev.mjs`             | `build_artifacts.yml` (referenced by `release.yml`)              |
| Files in `resources/`      | kebab-case for new files                            | `logo-dark.png`                   | `uad_lists.json`, `icon_clyde_blurple.svg`: public raw URLs      |
| Booleans                   | `is_`, `has_`, `supports_` prefix                   | `is_adb_available`                | serde fields kept for `config.toml` compatibility                |

- File names are checked by `npm run lint:names` (`.ls-lint.yml`). Identifiers by rustc and Clippy.
- No `utils`, `helpers` or `common` modules: name a module by what it does.
- Errors: return `Result<_, String>` with a user-readable message from `uad-core`. Log internals with `log`, never `panic!` on device or network input.
- Tests sit in a `#[cfg(test)] mod tests` at the end of the file they test. Tests never touch the real config or cache folders: pass a path (see `config.rs` tests).

## Gotchas

- **Shell edits corrupt `\t`, `\0` or `\\` in Rust literals**: the Bash tool rewrites backslashes. Edit Rust strings with the Edit tool.
- **`cargo test` used to overwrite the user's `config.toml`**: config tests now use `load_config_file(path)` with a scratch path. Keep it that way.
- **`npm run format:check` fails on issue templates with another Prettier version**: Prettier is pinned to 3.10.1. Use `npx prettier` from `node_modules`, not a global one.
- **GUI text clips or shapes wrongly**: use `crate::widgets::text`, `iced::widget::text` is banned by `clippy.toml`.
- **The window looks wrong below 900 px**: `min_size` in `gui.rs` stops users going smaller. Check layouts at 900 px and wider.
- **Contrast test fails after a palette change**: `theme.rs` tests every theme against WCAG AA. Adjust the colour, never the test.

## Docs

- `README.md`: what the app is, quick start, troubleshooting.
- `ARCHITECTURE.md`: module map, dependency rules, where new code goes.
- `DESIGN.md`: palettes, components and UI rules. Read before UI changes.
- `SECURITY.md`: threat model, controls and findings. Read before touching ADB, HTTP, files or CI.
- `RESEARCH.md`: evidence behind tooling and security choices.
- `CONTRIBUTING.md`: list-entry rules, branches, commits, labels, releases.
