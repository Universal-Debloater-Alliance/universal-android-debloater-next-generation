# Release management

## Semantic versioning

We're making use of [semantic versioning](https://semver.org/). We use semantic versioning for two things:

1. GitHub Milestones
1. UAD-ng releases

Speaking of milestones: it could be that we have a bug-fix release (e.g. 1.0.3), but a contributor submits a PR that adds a new feature. What do we do? Merge after we released 1.0.3? Change the name of the milestone to for example 1.1.0? Or another approach?

In that case, we should closely review the PR and 'compare' it with what semantic versioning has to say about the submitted feature. If the changes are significant enough, we're renaming the milestone to (for example) 1.1.0.

## Checklist before releasing a new version

- [ ] Make sure the Milestone is completed and if not, check what needs to be done to complete it.
- [ ] Make sure all merged PRs regarding the application follow the Conventional Commit Messages style. This makes sure that when generating a changelog, the changes look consistent, which in turn improves readability.
- [ ] Make sure all merged PRs have the correct labels assigned, as the changelog is generated based on labels.
- [ ] Create a release preparation PR that bumps the Rust package version in `Cargo.toml` and `Cargo.lock`

When all the above has been checked, you can push a tag to `main` to trigger the GitHub Action that creates a release:

- Create the tag: `git tag -s v1.2.3 -am '1.2.3'`
- Push the tag: `git push origin v1.2.3`

Change the version number to whatever version we're releasing.

## Development process

1. Update dependencies right after a new release, so that we start a new milestone with updated dependencies
1. On-going development of bug-fixes and new features etc, along with fixing potential bugs, potentially due to the updated dependencies
1. Release

## Faulty releases

If we publish a faulty release, for example containing a critical bug, this is how we should deal with it:

We issue a hotfix and keep the faulty release but we'll add warnings to the faulty release in the changelog.
