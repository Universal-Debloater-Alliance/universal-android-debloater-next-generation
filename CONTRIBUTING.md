# Contributors Guide

## For who is this guide?

This guide is meant for users who want to contribute to the codebase of Universal Android Debloater Next Generation, whether that is the application code or [the "database"-file for adding packages](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/wiki/How-to-contribute). To keep all processes streamlined and consistent, we're asking you to stick to this guide whenever contributing.

Even though the guide is made for contributors, it's also strongly recommended that the UAD-ng team sticks to these guidelines. After all, we're a prime example.

## What are the guidelines?

### Mass edits

If you've performed massive edits via automation, you must specify what automation you used. For example, if you ran the command:
```sh
sed -i 's/a/b/' resources/assets/uad_lists.json
```
You should 📋copy-paste that cmd to the pull-request description. This rule applies even if you use "proper" cmds such as `jq`, which safely edit structured data. And it also applies to AI, *especially* LLMs.

This is required so that we can review your big patch without reading it. This improves [transparency](https://en.wikipedia.org/wiki/Transparency_(behavior)), which makes you more trust-worthy! Bonus points if you can provide a sequence of cmds that proves your patch contains exactly what you said it does. For the simple `sed` case, something like:
```sh
# alt: `gh pr checkout 0000 && git checkout main`
git remote add fork https://github.com/user/uadng
git fetch fork
# example commit hash from upstream (not fork);
# more reproducible than branch-name
git checkout beefcafe
sed -i 's/a/b/' resources/assets/uad_lists.json
# compare unstaged change with commit from fork
git diff cafebeef
```
Should be enough.

You should be careful with mass-editing anyways. Even if the verification is successful, the effects [might not be what you expect](https://en.wikipedia.org/wiki/Scunthorpe_problem).

### Branching strategy

As for our branching strategy, we're using [Trunk-Based Development](https://trunkbaseddevelopment.com/#one-line-summary).

In short, there's one trunk branch named `main` (`master`). Apart from `main`/`master` there can be different short-lived branches, such as:

- Features (`feature/*`)
- Fixes (`hotfix/*` or simply `fix/*`)
- Dependency updates (`deps/*`)
- Etc.

Do mind that these branch names do only not apply to the addition of packages; for that, we use the following scheme: `[issue number][issue title]`. This can be done [automatically](https://docs.github.com/en/issues/tracking-your-work-with-issues/creating-a-branch-for-an-issue) too.

This is how it looks like and works:
![Trunk-Based Development](https://trunkbaseddevelopment.com/trunk1c.png)

### Syncing branches

If you want to [back-port](https://en.wikipedia.org/wiki/Backporting) (GitHub's "Update" and "Sync" buttons) from upstream `main` to any other branch (for testing purposes, or any other reasonable case), it's recommended that you `rebase` rather than `merge` the changes.

**Don't apply changes to/from arbitrary branches**, that's a recipe for chaos! If you encounter a situation where it's absolutely necessary to sync non-`main` branches, whether to `merge` or `rebase` should be decided by the UAD-ng team (with some contributor feedback) on a case-by-case basis.

### Commit messages

As for commits, we prefer using [Conventional Commit Messages](https://gist.github.com/qoomon/5dfcdf8eec66a051ecd85625518cfd13). When working in any of the branches listed above (if there's an existing issue for it), close it using a [closing keyword](https://docs.github.com/en/issues/tracking-your-work-with-issues/linking-a-pull-request-to-an-issue#linking-a-pull-request-to-an-issue-using-a-keyword). For more information regarding Conventional Commit Messages, see <https://www.conventionalcommits.org/en/v1.0.0/> as well.
