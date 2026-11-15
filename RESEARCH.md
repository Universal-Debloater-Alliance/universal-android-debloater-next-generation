# Research

Purpose: dated evidence behind tooling, dependency and security decisions. Read when: choosing or revisiting a library, tool or approach.

## Self-update integrity (2026-11-15)

**Brief:** the updater downloaded a binary and replaced the running executable without any integrity check.
**Scope:** the release workflow, the assets of release v1.3.1, the `sha2` crate.
**Findings:**

- `.github/workflows/build_artifacts.yml` already publishes a `sha256sum` file per asset: `<asset>.exe-checksum.txt` on Windows, `<asset>-checksum` elsewhere. Release v1.3.1 lists all of them (GitHub API, `releases/latest`).
- `sha2` 0.11.1 (RustCrypto, MIT or Apache-2.0, MSRV 1.88) was not yet in the dependency tree.
- The old check compared version strings as text, so `"1.9.0" > "1.10.0"`.

**Decision:** Adapt. Verify each download against the published checksum with `sha2`, behind the `self-update` feature. Compare versions as numbers. Release signing was not added: it changes the release process and needs a maintainer key.
**Sources:** `.github/workflows/build_artifacts.yml`, GitHub API `repos/Universal-Debloater-Alliance/universal-android-debloater-next-generation/releases/latest`, `cargo info sha2`.

## HTTP client settings (2026-11-15)

**Brief:** `ureq` calls had no timeout and accepted plain HTTP.
**Scope:** `ureq` 3.2.1 source in the Cargo registry.
**Findings:** `ConfigBuilder::timeout_global` and `timeout_connect` default to `None`, `https_only` defaults to `false` (`src/config.rs`). The `https_only` check runs before DNS and connect (`src/run.rs` `call_run`), so the test for it needs no network.
**Decision:** Build. One private `http_client::agent(total_timeout)` with HTTPS only and a 15 s connect timeout. The list retry loop was cut from 60 x 1 s to 10 x 2 s, so an offline start falls back to the cached list after about 20 s instead of 60 s.
**Sources:** `ureq-3.2.1/src/config.rs`, `ureq-3.2.1/src/run.rs`.

## Dependency advisories (2026-11-15)

**Brief:** clear RustSec findings without upgrading Iced.
**Scope:** `cargo audit` 0.23.0 with the RustSec database of 2026-11-15, `cargo tree -i` for each finding, crates.io metadata.
**Findings:**

- 4 vulnerabilities and 16 warnings before. Semver-compatible lockfile updates fixed crossbeam-epoch, webpki-roots, quick-xml (via wayland-scanner 0.31.11), anyhow, event-listener, memmap2, rand, rustix and clipboard-win.
- Iced's `image` feature enables every `image` codec, including the AVIF encoder (`rav1e`, which pulls the yanked `core2`). The GUI only decodes one PNG icon, and `iced_core::window::icon::from_rgba` needs no Iced feature.
- `dark-light` 3.0.0 (2026-08-12) drops `async-std` and `ashpd`. `detect()` and `Mode` are unchanged.
- The CLI declared `tokio` with `full` features for a `main` without `.await`, plus 7 other unused crates.

**Decision:** Adapt. Lockfile updates, `image` with PNG only, `dark-light` 3, unused CLI dependencies removed. The 7 remaining advisories sit inside Iced 0.14 and are ignored with reasons in `.cargo/audit.toml`.
**Sources:** `cargo audit` output (`.agent-local/output/cargo-audit-before.json`, not committed), `iced-0.14.0/Cargo.toml`, crates.io API for `dark-light` 3.0.0.

## Developer tooling (2026-11-15)

**Brief:** one local launcher and one local CI command, and enforced naming and duplication rules, in a Rust repository that already formats Markdown, JSON and YAML with Prettier.
**Scope:** npm registry release dates, install scripts, and output against the committed files.
**Findings:**

- Prettier 3.9.10 rewrote two issue templates that the committed formatting (Prettier 3.10.1) leaves alone. 3.10.1 matches the repository.
- `@ls-lint/ls-lint` 2.3.2 (file names by directory and extension) and `jscpd` 5.4.2 (clone detection, supports Rust) have no install scripts.
- zizmor 1.32.0 recommends the `$/` self-repository syntax for in-repository reusable workflows. GitHub supports it since July 2026, and it makes the reference count as pinned.

**Decision:** Adopt. `package.json` pins Prettier 3.10.1, ls-lint 2.3.2 and jscpd 5.4.2. The other tools were chosen at least two weeks after release, to lower the risk of a freshly compromised version. GitHub Actions are pinned to commit SHAs of releases at least one week old.
**Sources:** `npm view <package> time`, `npm view <package> scripts`, <https://docs.zizmor.sh/audits/#self-repository>.
