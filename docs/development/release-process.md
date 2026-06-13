# Release process

This document describes how Memryzed releases are cut. It covers
regular minor and patch releases, hotfix releases, and security
releases.

The general principle is that the release pipeline is automated; the
maintainer's job at release time is to confirm the contents are
correct, then push a tag.

## Roles

- Release manager: a maintainer responsible for cutting one
  release. Rotates among maintainers.
- Reviewer: a second maintainer who reviews the release commit and
  signs off before tag.

For solo maintainership, the release manager and reviewer are the
same person but the checklist must still be completed in order.

## Release cadence

There is no fixed schedule. Releases happen when there is meaningful
new content or when bugs warrant a patch. Targets:

- Minor releases (X.Y.0): roughly every 4 to 8 weeks during active
  development.
- Patch releases (X.Y.Z): as needed for bug and security fixes.
- Pre-releases (RC, beta): before any 1.0 or major release.

## Release types

### Minor release (0.Y+1.0 or X.Y+1.0)

Used for new features and intentional behavior changes.

Pre-flight:

1. Confirm `[Unreleased]` in `CHANGELOG.md` reflects all merged
   changes since the previous release. Reorder, combine, and tighten
   entries.
2. Confirm the v1 specification (or the relevant spec) is current
   with the implementation.
3. Confirm the documentation set in `docs/` reflects current
   behavior. Pay specific attention to `cli-reference.md`,
   `mcp-reference.md`, and `configuration.md`.
4. Run the full test suite locally:

       cargo nextest run --workspace
       cargo clippy --workspace --all-targets -- -D warnings
       cargo fmt --check
       cargo deny check
       cargo audit

5. Run end-to-end tests against at least one real MCP client and
   confirm `memryzed doctor` reports clean.
6. Verify benchmarks have not regressed beyond thresholds.

Cut:

1. Bump the version in `Cargo.toml` (workspace and all member
   crates).
2. In `CHANGELOG.md`, rename `[Unreleased]` to the new version with
   today's date, and add a fresh `[Unreleased]` section above:

       ## [Unreleased]
       ### Added
       (none)
       ### Changed
       (none)
       ...

       ## [0.2.0] - 2026-MM-DD
       ### Added
       - ...

3. Open a PR titled `release: 0.2.0` containing only the version
   bump and changelog rename.
4. After approval and CI green, merge.
5. Tag from `main`:

       git checkout main
       git pull --ff-only
       git tag -a v0.2.0 -m "Memryzed 0.2.0"
       git push origin v0.2.0

6. The release pipeline (`cargo-dist` via GitHub Actions) takes
   over: builds binaries for every target, generates installer
   scripts, publishes to GitHub Releases, updates the Homebrew tap
   and Scoop bucket.
7. Once the pipeline completes, verify:

   - The GitHub Releases page shows the new release with all
     assets.
   - `https://memryzed.com/install.sh` resolves to the new version.
   - `brew upgrade memryzed` upgrades to the new version.
   - `cargo install memryzed --force` works.

8. Cut a `release/X.Y` branch from the tag if this is a fresh minor
   line:

       git checkout -b release/0.2 v0.2.0
       git push -u origin release/0.2

   This branch will receive backports for any subsequent patch
   releases.

9. Announce the release: a short post on the project's announcement
   channels and the relevant communities. Link to the release page
   and the CHANGELOG.

### Patch release (X.Y.Z+1)

Used for bug fixes and security fixes that should reach existing
users quickly.

Pre-flight:

1. Confirm the fix has been merged into `main`.
2. Identify which release lines need the patch. Typically the most
   recent minor line; sometimes older lines if they are still
   supported.
3. For each line:

   - Check out the relevant `release/X.Y` branch.
   - Cherry-pick the fix commit(s) from `main`.
   - Resolve any conflicts.
   - Push to the release branch.

Cut:

1. Bump the patch version in the release branch's `Cargo.toml`.
2. Move the relevant changelog entries from `main`'s `[Unreleased]`
   section into a new released section in the release branch's
   `CHANGELOG.md`. Make sure the entry mentions the patch version.
3. Tag from the release branch:

       git checkout release/0.2
       git pull --ff-only
       git tag -a v0.2.1 -m "Memryzed 0.2.1"
       git push origin v0.2.1

4. Verify the pipeline output as for a minor release.
5. Update the user-facing CHANGELOG on `main` if necessary so users
   reading the trunk see the patch fix represented.

### Security release

A patch release that addresses a security vulnerability has the same
mechanics with these additions:

1. Coordinate disclosure with the reporter under `SECURITY.md`.
2. Develop and review the fix in a private fork or a private branch
   if the vulnerability is not yet public.
3. Prepare the security advisory in GitHub Security Advisories.
4. Cut the patch release.
5. Publish the advisory simultaneously with the release.
6. The CHANGELOG entry for the fix goes under `Security` and
   references the advisory by its identifier.

Severity targets are documented in `SECURITY.md`. The incident
process for an active vulnerability is in `incident-response.md`.

### Pre-release

Used to validate a major or significant minor release in the wild.

1. Bump the version with a pre-release identifier:
   `0.1.0-rc.1`, `0.1.0-rc.2`, etc.
2. Tag and push as for a regular release.
3. Pre-releases do not update the `latest` GitHub Release. The
   install scripts default to the latest non-prerelease, so users
   on the standard install command are unaffected.
4. Announce on the relevant channels with a clear "this is a
   pre-release; please report issues" framing.
5. After at least one full week with no critical issues, cut the
   final release.

## The release pipeline in CI

The pipeline is defined in `.github/workflows/release.yml`. It
triggers on tag push (`v*`). For security it has **no AWS or deploy
credentials**: it only builds and publishes a GitHub Release.
Promoting a release to the live install endpoint is a separate manual
step (below) performed from a trusted machine, so a compromised CI
run can never reach production.

On a tag, CI:

- Builds the `memryzed` binary for every supported target on its
  native runner (Linux x86_64 and aarch64, macOS Intel and Apple
  Silicon, Windows x86_64). Linux builds run on Ubuntu 22.04 (glibc
  2.35) so they run on older distributions too.
- Smoke-tests each binary (`memryzed --version`) on its own
  architecture, so a binary that does not start, for example because
  the embedder/ONNX stack failed to link, fails the release.
- Packages each as `memryzed-<target>.tar.gz` (or `.zip` on Windows)
  with a `.sha256`.
- Generates a build-provenance attestation for each artifact, so
  anyone can verify a download was built by this workflow from this
  commit.
- Attaches everything to a GitHub Release.

The pipeline does not modify `main` and does not touch memryzed.com.

## Promoting a release to memryzed.com

The GitHub Release is the build output; the install endpoint
(`memryzed.com/releases/`) is updated by a human from a trusted
machine that holds the AWS credentials. CI never has them.

The one-shot script `.script-deploy.sh` at the repo root automates the
whole promotion. From a trusted machine with `gh` authenticated and
the AWS profile configured:

       ./.script-deploy.sh              # promote the version in Cargo.toml
       ./.script-deploy.sh v0.7.1       # promote a specific tag
       DRY_RUN=1 ./.script-deploy.sh    # preview without changing anything

It downloads the release artifacts, verifies every checksum, requires
all expected target archives to be present, uploads them to
`s3://memryzed.com/releases/v<version>/`, points `latest.txt` at the
new version (skip with `--no-latest`), re-syncs the install scripts
from `dist/`, invalidates the CDN, and verifies the live endpoint. It
is idempotent.

The equivalent manual steps, if you ever need them:

1. Download the artifacts from the GitHub Release (or the run's
   attestation-verified artifacts).
2. Verify each archive against its `.sha256`.
3. Optionally verify provenance: `gh attestation verify <file>
   --repo memryzed/memryzed`.
4. Sync to S3 under the version path and update `latest.txt`:

       aws s3 cp <archive> s3://memryzed.com/releases/v<version>/
       aws s3 cp <archive>.sha256 s3://memryzed.com/releases/v<version>/
       printf '%s' "<version>" | aws s3 cp - s3://memryzed.com/releases/latest.txt

5. Invalidate the CDN: `aws cloudfront create-invalidation
   --distribution-id <id> --paths '/releases/*'`.
6. Run the verification checklist below before announcing.

Only promote to `latest.txt` after the macOS and Windows binaries
have been run on a real machine of each kind; CI proves they build
and start, not that every feature works on every OS.

## Verifying a release

After every release, run this checklist on a clean machine:

1. Install via `curl -fsSL https://memryzed.com/install.sh | bash`.
2. `memryzed --version` prints the new version.
3. `memryzed init` works.
4. `memryzed install` detects an MCP client and writes its config.
5. `memryzed doctor` reports clean.
6. A simple end-to-end interaction works: agent calls `remember`,
   then `recall`, gets the right result.

If any check fails, post a notice and prepare a follow-up patch
release.

## Rolling back a release

Memryzed releases are immutable once tagged. If a release is broken,
the response is:

1. Mark the GitHub Release as a pre-release so that install scripts
   and update checks ignore it.
2. Cut a patch release that fixes the problem.
3. Add a notice to the broken release's notes pointing to the patch.

We do not delete tags. The only acceptable rollback is to ship the
next version forward.

## Backporting changes

To backport a change from `main` to a release branch:

1. Confirm the change is appropriate to backport. Bug fixes and
   security fixes are appropriate. New features are not, unless the
   release line is still in active development and a maintainer
   approves the exception.
2. Cherry-pick the commit:

       git checkout release/0.2
       git cherry-pick -x <commit-sha>

   The `-x` flag records the original commit hash in the message.

3. Resolve conflicts.
4. Push to the release branch and open a PR for review.
5. After merge, the release branch is ready for the next patch tag.

## Communications

Release announcements should include:

- The version.
- A one-paragraph summary of what changed.
- The link to the GitHub Release.
- The link to the CHANGELOG section for that version.
- For security releases, a link to the advisory and a clear
  recommendation to upgrade.

Channels:

- Project's announcement channel (when one exists).
- Memryzed.com release notes page (linked from the landing page).
- Relevant developer communities, only for significant releases.

Avoid hype language. The goal is to inform users so they can decide
whether to upgrade.

## Post-release

After every release:

- Update the project's roadmap (`docs/roadmap.md`) to reflect what
  shipped.
- Triage any issues filed against the release.
- Capture any retrospective items for the next release.

## Common pitfalls

- Forgetting to bump every member crate's version. Use a script.
- Tagging from a stale local branch. Always `git pull --ff-only`
  first.
- Forgetting to move CHANGELOG entries out of `[Unreleased]`. The
  release commit catches this if reviewed.
- Releasing a binary that has not been smoke-tested on every
  target. The pipeline does this automatically; trust it but verify
  the smoke-test results in the workflow output.
- Cutting a release while CI on `main` is red. Wait for CI green.
