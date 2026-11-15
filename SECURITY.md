# Security

Purpose: how UAD-ng is protected and what risk remains. Read when: touching ADB commands, HTTP, files, the self-updater, dependencies or CI.
Last verified: 2026-11-15.

## Reporting

Report a vulnerability privately through GitHub: repository Security tab, "Report a vulnerability". Do not open a public issue for it. For everything else use the [issue templates](.github/ISSUE_TEMPLATE/).

## Threat Model

- **Assets**: the user's Android device (removing the wrong system package can stop it booting), the user's computer (the self-updater replaces and runs an executable), backups and exports in the backup folder.
- **Entry points and trust boundaries**:
    - GitHub over HTTPS: the debloat list (`uad_lists.rs`), release metadata and binaries (`update.rs`). Untrusted until parsed and, for binaries, checksum-verified.
    - `adb` output: device serials, user lists, package names (`adb.rs`). Untrusted text from a connected device.
    - User input: CLI package names and filters, GUI search text, a chosen backup folder.
    - Local files: `config.toml`, backup JSON files, the cached list.
- **Main threats**: shell injection through a package name into `adb shell`, a tampered or truncated update binary, a hostile or malformed device crashing the app, path tricks through a device serial, spreadsheet formulas in exported text, a hijacked CI workflow publishing a release.

## Controls

| Control                                                                                    | Where                                                                     |
| ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------- |
| Package names validated before any device shell command (fails closed)                     | `adb.rs` `PackageId::new`, `sync.rs` `request_builder`                    |
| HTTPS only, 15 s connect and per-call total timeouts                                       | `uad-core/src/http_client.rs`, the only HTTP client builder               |
| Remote list size capped at 8 MiB, cached only when it parses                               | `uad_lists.rs` `load_debloat_lists`                                       |
| Update binaries verified against the release's SHA-256 file before use                     | `update.rs` `download_verified_asset`                                     |
| Release versions compared numerically, never as text                                       | `update.rs` `is_newer_release`                                            |
| Malformed `adb` output skipped and logged, never a panic                                   | `adb.rs` `parse_device_list`, `parse_user_list`                           |
| Device serials made safe as folder names                                                   | `save.rs` `device_backup_dir`                                             |
| CSV cells that start a formula are prefixed with `'`                                       | `export.rs` `neutralise_spreadsheet_formula`                              |
| Dependency audit (RustSec, `--deny warnings`), npm audit, workflow audit (zizmor)          | `.github/workflows/ci.yml` job `security`, ignores in `.cargo/audit.toml` |
| Actions pinned to commit SHAs, read-only `GITHUB_TOKEN`, no persisted checkout credentials | all files in `.github/workflows/`                                         |
| Downloaded CI tool (grcov) checked against a pinned SHA-256                                | `ci.yml` job `coverage`                                                   |
| Dependabot for Cargo, npm and GitHub Actions, with a 7-day cooldown                        | `.github/dependabot.yml`                                                  |
| Node dev tools installed with `--ignore-scripts` and an exact-version lockfile             | `package.json`, `package-lock.json`                                       |

Checklist status (OWASP Top 10:2025 categories that apply to a local desktop app):

| Category                                   | Status                                                                                                |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| Secrets                                    | Clean: a pattern scan of every added line in all 2,105 commits found none (gitleaks could not be run) |
| Authentication, authorisation, sessions    | Not applicable: no accounts, server or network listener                                               |
| Injection (OS command)                     | Clean: arguments passed as an argument vector, package names validated                                |
| Injection (CSV formula)                    | Fixed (SEC-07)                                                                                        |
| Server parsing, SSRF, XXE, deserialisation | Clean: fixed HTTPS URLs, serde into typed structs, size cap on the list                               |
| Files and paths                            | Fixed (SEC-06)                                                                                        |
| Exceptional conditions                     | Fixed (SEC-05, SEC-08)                                                                                |
| Crypto and transport                       | Fixed (SEC-01, SEC-04): rustls with the bundled Mozilla roots (`webpki-roots`)                        |
| Supply chain                               | Fixed (SEC-01, SEC-02, SEC-09, SEC-10), residual items below                                          |
| CI/CD                                      | Fixed (SEC-09)                                                                                        |
| Denial of service                          | Fixed (SEC-03)                                                                                        |
| Web client, API, LLM, containers           | Not applicable                                                                                        |

## Findings

From the review of 2026-11-15. Each fix has a regression test in the same file.

| ID     | Severity | CWE      | Status | Location                                    | Fix                                                                                                                                         |
| ------ | -------- | -------- | ------ | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| SEC-01 | Medium   | CWE-494  | Fixed  | `crates/uad-core/src/update.rs`             | Self-update checks the published SHA-256 before replacing the executable                                                                    |
| SEC-02 | Medium   | CWE-697  | Fixed  | `crates/uad-core/src/update.rs`             | Numeric version comparison: text comparison would offer 1.9.0 as newer than 1.10.0                                                          |
| SEC-03 | Low      | CWE-400  | Fixed  | `uad_lists.rs`, `update.rs`                 | Timeouts on every HTTP call (none by default in `ureq` 3)                                                                                   |
| SEC-04 | Low      | CWE-319  | Fixed  | `crates/uad-core/src/http_client.rs`        | HTTPS only, including redirects                                                                                                             |
| SEC-05 | Low      | CWE-248  | Fixed  | `crates/uad-core/src/adb.rs`                | Unexpected `adb devices` or `pm list users` lines no longer panic                                                                           |
| SEC-06 | Low      | CWE-22   | Fixed  | `crates/uad-core/src/save.rs`               | Device serials cannot add path components to the backup folder                                                                              |
| SEC-07 | Low      | CWE-1236 | Fixed  | `crates/uad-core/src/export.rs`             | Exported CSV cells cannot start a spreadsheet formula                                                                                       |
| SEC-08 | Low      | CWE-129  | Fixed  | `save.rs`, `export.rs`, `views/settings.rs` | Unknown user index or missing backup selection returns an error instead of panicking                                                        |
| SEC-09 | Medium   | CWE-829  | Fixed  | `.github/workflows/`                        | SHA-pinned actions, read-only token, no persisted credentials, verified grcov download                                                      |
| SEC-10 | Medium   | CWE-1395 | Fixed  | `Cargo.lock`, `Cargo.toml`                  | 4 vulnerable crates updated (crossbeam-epoch, quick-xml, webpki-roots), unused codecs and `tokio` removed, `dark-light` 3 drops `async-std` |

## Residual Risks and Required Actions

- **Accepted dependency advisories**: 7, all inside Iced 0.14's dependency tree, listed with reasons in `.cargo/audit.toml`. Recheck when upgrading Iced.
- **Release checksums share a trust root with the binaries**: both come from the same GitHub release, so the check catches corruption and partial downloads, not a compromised maintainer account. Signing releases (for example with Sigstore or minisign) would close this.
- **Old releases still use the text version comparison**: users on 1.3.1 and earlier get the fix only by updating.
- **Owner actions**: enable GitHub secret scanning with push protection and Dependabot alerts in the repository settings, and keep "Require approval for workflows from outside collaborators" on.
