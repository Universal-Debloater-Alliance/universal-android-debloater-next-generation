# Contributing

Purpose: how to propose changes to the app code or to the package list, and how maintainers release. Read when: preparing a contribution or a release.
Last verified: 2026-11-15.

These rules apply to everyone, the UAD-ng team included. For adding packages, the wiki page [How to contribute](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/How-to-contribute) explains the research behind an entry.

## Setup

Follow [README.md#quick-start](README.md#quick-start). Then run `npm ci` once to install the formatting and lint tools.

## Workflow

### Checks

Run `npm run check` before opening a pull request. It runs everything CI runs: Prettier, file-name and duplicate-code checks, rustfmt, Clippy, builds with warnings denied, and all tests. Fix every failure, never silence a check.

### Package list entries

Rules for `resources/assets/uad_lists.json`, checked by the tests in `crates/uad-core/src/uad_lists.rs` (`cargo test -p uad-core`):

- Each package name appears once. A duplicate key is silently dropped by JSON parsers, so its text is lost.
- Each package has a non-empty `description`.
- `dependencies` lists the packages this package needs. `neededBy` lists the packages that need it. Record every link on both packages: if `A.dependencies` has `B`, then `B.neededBy` has `A`.
- An overlay needs the package it overlays, so the overlay lists its target in `dependencies`. Add the reverse link only when the target really needs the overlay.
- The app uses these links to offer removing related packages together (GUI checkbox, CLI `--with-related`) and to warn about packages that may break. A wrong link causes a wrong suggestion, so add only links you have verified.
- A good description says what the package is, what breaks without it, what you gain by removing it, and which packages depend on it. Do not repeat the removal rating, the app explains it next to the description.
- If something cannot be verified, say so. Text inferred only from the package name ends with `Unverified: inferred from the package name.`, so reviewers can find and improve it.
- Format the file with `npm run format` (Prettier with the repository's `.prettierrc.json`: tabs, short arrays on one line).

### Mass edits

If you made large edits with automation, put the exact command in the pull request description, even for "safe" tools such as `jq`, and for AI tools name the tool and, if known, its version. Reviewers can then check the patch without reading every line. Better still, add commands that prove the patch contains only that change, for example:

```sh
# alt: `gh pr checkout 0000 && git checkout main`
git remote add fork https://github.com/user/uadng
git fetch fork
# a commit from upstream (not the fork) is more reproducible than a branch name
git checkout beefcafe
sed -i 's/a/b/' resources/assets/uad_lists.json
# compare the unstaged change with the commit from the fork
git diff cafebeef
```

Mass edits can still have [unexpected effects](https://en.wikipedia.org/wiki/Scunthorpe_problem) even when the verification passes. AI edits are not reproducible, so a verification script does not apply to them.

### Branches

We use [trunk-based development](https://trunkbaseddevelopment.com/#one-line-summary): one trunk, `main`, and short-lived branches.

- Features: `feature/<summary>`. Fixes: `fix/<summary>` (or `hotfix/<summary>`). Dependency updates: `deps/<summary>`.
- Package additions use `<issue number>-<issue title>`, which GitHub can [create from the issue](https://docs.github.com/en/issues/tracking-your-work-with-issues/creating-a-branch-for-an-issue).
- To update a branch from `main`, `rebase` rather than `merge`. Do not sync changes between two non-`main` branches. If that is ever unavoidable, the UAD-ng team decides case by case.

### Commits

Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). Close the related issue with a [closing keyword](https://docs.github.com/en/issues/tracking-your-work-with-issues/linking-a-pull-request-to-an-issue#linking-a-pull-request-to-an-issue-using-a-keyword).

### Labels

Issues are labelled by the [issue template](.github/ISSUE_TEMPLATE/) you choose. Labels sort the generated [release notes](.github/release.yml).

| Label                    | Purpose                                          | Issue template                            | Release notes section |
| ------------------------ | ------------------------------------------------ | ----------------------------------------- | --------------------- |
| `bug`                    | Software bugs in UAD-ng                          | Bug Report                                | Fixed                 |
| `enhancement`            | New feature requests                             | Feature Request                           | Features              |
| `package::addition`      | Adding new packages to the debloat list          | Add new package(s)                        | Packages/apps         |
| `package::breakage`      | Issues caused by debloating                      | Debloat issue report                      | Packages/apps         |
| `package::documentation` | Updating package descriptions or recommendations | Update apps description or recommendation | Packages/apps         |
| `documentation`          | Improvements to docs or the wiki                 | UAD-ng documentation                      | Documentation         |
| `ci`                     | CI/CD related issues                             | CI/CD                                     | Development           |
| `refactor`               | Code refactoring                                 | None (maintainer-assigned)                | Refactors             |
| `dependencies`           | Dependency updates                               | None (maintainer-assigned)                | Development           |

## Releases

For maintainers. Versions follow [semantic versioning](https://semver.org/), for both GitHub milestones and releases. If a pull request adds a feature during a bug-fix milestone (for example 1.0.3), review it against semantic versioning and rename the milestone (for example to 1.1.0) when the change warrants it.

Before releasing:

- [ ] The milestone is complete, or what remains is known.
- [ ] Every merged pull request follows Conventional Commits and has the right labels (the release notes are generated from them).
- [ ] A release preparation pull request bumps the version in `Cargo.toml` and `Cargo.lock`.

Then push a signed tag to `main`. `.github/workflows/release.yml` builds every binary with its SHA-256 file and publishes the release (the self-updater refuses assets without that file):

```sh
git tag -s v1.2.3 -am '1.2.3'
git push origin v1.2.3
```

Cycle: update dependencies right after a release, develop and fix during the milestone, then release. A faulty release stays published: ship a hotfix and add a warning to the faulty release's notes.
