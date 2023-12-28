# Contributors Guide

## For who is this guide?

This guide is meant for users who want to contribute to the codebase of Universal Android Debloater Next Generation, whether that is the application code or the JSON-file for adding packages. To keep all processes streamlined and consistent, we're asking you to stick to this guide whenever contributing.

## What are the guidelines?

As for our branching strategy, we're using [Trunk-Based Development](https://trunkbaseddevelopment.com/#one-line-summary).

In short, there's one trunk branch named `main` (also known as `master`). Apart from `main`/`master` there can be different short-lived branches, such as:

- Features (`feature/*`)
- Fixes (`hotfix/*` or simply `fix/*`)
- Dependency updates (`deps/*`)
- Etc.

Do mind that these branch names do only not apply to the addition of packages; for that, we use the following scheme: `[issue number][issue title]`. This can be done [automatically](https://docs.github.com/en/issues/tracking-your-work-with-issues/creating-a-branch-for-an-issue) too.

This is how it looks like and works:

![Trunk-Based Development](https://trunkbaseddevelopment.com/trunk1c.png)

As for commits, we prefer using [Conventional Commit Messages](https://gist.github.com/qoomon/5dfcdf8eec66a051ecd85625518cfd13). When working in any of the branches listed above (if there's an existing issue for it), close it using a [closing keyword](https://docs.github.com/en/issues/tracking-your-work-with-issues/linking-a-pull-request-to-an-issue#linking-a-pull-request-to-an-issue-using-a-keyword).

When creating a PR, please make sure your changes are documented clearly.