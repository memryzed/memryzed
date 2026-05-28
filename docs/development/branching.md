# Branching and pull requests

This document describes the git workflow used by Memryzed. The model
is small and predictable, designed to keep a clean history and a
simple release process.

## Branches

There are three kinds of branches:

- `main`. The trunk. Always in a state where it could be released.
  All work merges here. Tags are cut from here for releases.
- Topic branches. Short-lived branches for individual changes.
  Created from `main`, merged back into `main` via pull request, and
  deleted after merge.
- Release branches. Long-lived branches of the form `release/0.1`,
  `release/0.2`, etc. Created at the time we ship a `0.x.0` minor
  version, used only for backporting patch releases. Discussed in
  `release-process.md`.

There is no `develop` branch. There are no permanent feature
branches.

## Topic branch naming

Topic branches use the convention:

    <kind>/<short-description>

Where kind is one of:

    feat       A new feature.
    fix        A bug fix.
    perf       A performance improvement.
    refactor   Internal change with no user-visible effect.
    docs       Documentation only.
    test       Test changes only.
    chore      Tooling, dependencies, build system.
    security   A security fix; see release-process.md for handling.

Examples:

    feat/session-pin
    fix/recall-empty-query
    docs/agent-author-guide
    chore/bump-rmcp

Branch names use lowercase letters, digits, and hyphens. No spaces,
no slashes beyond the kind prefix.

## Workflow

1. Sync with main:

       git checkout main
       git pull --ff-only

2. Create a topic branch:

       git checkout -b feat/session-pin

3. Make your change. Keep commits focused. Write good commit messages
   (see "Commit messages" below).

4. Push the branch:

       git push -u origin feat/session-pin

5. Open a pull request against `main`.

6. Address review feedback by adding new commits, not by force-pushing
   over feedback that has already been left. After approval, you may
   squash if the maintainer prefers.

7. Once approved and CI is green, the maintainer merges. We default
   to squash-merge for clean history. Multi-commit merges are
   allowed when the individual commits are coherent and add value.

8. Delete the topic branch after merge.

## Commit messages

The first line of every commit follows this form:

    <kind>(<scope>): <short summary>

Examples:

    feat(memory): allow scope=session in remember
    fix(retrieval): correctly normalize FTS5 scores
    docs(spec): update v1.md acceptance criteria
    chore(deps): bump tokio to 1.39

The `<scope>` is optional but encouraged. It is one of: `core`,
`mcp`, `cli`, `tui`, `storage`, `retrieval`, `extractor`, `sessions`,
`integration`, `update`, `audit`, `config`, `docs`, `deps`, `ci`, or
the name of a specific module.

The summary is in imperative mood, present tense, lowercase, no
trailing period, ideally under 60 characters.

The body, if present, explains the why. Wrap at 72 columns. Reference
issues with `Refs #123`. Use `Fixes #123` for issues this commit
closes.

When a commit is the result of a security fix, the body should
include a note that the fix is security-relevant and reference the
advisory if one is being prepared.

## Pull request expectations

A pull request is ready to merge when:

- The PR description explains motivation, approach, and tradeoffs.
- All CI checks pass.
- There are tests that cover the change.
- Documentation is updated where the change affects it.
- An entry has been added to `CHANGELOG.md` under `[Unreleased]`.
  See `changelog-conventions.md`.
- At least one maintainer has approved.

Small PRs review faster than large ones. If your PR exceeds about
500 lines of diff, consider splitting it.

## Reviewing pull requests

Reviewers should check:

- The change matches what the PR description claims.
- The code follows the conventions in `setup.md`.
- New or changed code has tests.
- The change does not introduce a backward-incompatible API or
  schema change without a major version bump (see `versioning.md`).
- The CHANGELOG entry is present and well-written.
- The PR does not silently change behavior of unrelated areas.

Approve, request changes, or comment. If you approve with comments,
the author can address comments without re-review. If you request
changes, the author must request re-review after addressing them.

## Force-pushing

Force-push is allowed on your own topic branches before review
starts. Once a maintainer has reviewed, prefer adding commits over
force-pushing, so reviewers can see what changed since their last
look. After approval, squash-rebasing before merge is fine.

`main` is never force-pushed. Release branches are never
force-pushed.

## Hotfixes

For urgent fixes that must reach a stable release branch, see the
"Patch release" section of `release-process.md`. The branch name
convention for hotfixes is:

    hotfix/<release-line>/<short-description>

Example: `hotfix/0.1/install-checksum-mismatch`. The branch is
created from `release/0.1`, fixed, merged into the release branch,
backported to `main`, and the maintainer cuts a patch release.
