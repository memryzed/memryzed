# Versioning

Memryzed follows Semantic Versioning 2.0.0 (https://semver.org/) with
the conventions in this document.

## Version format

Versions are of the form `MAJOR.MINOR.PATCH`, optionally with a
pre-release suffix:

    1.2.0
    1.2.1
    1.3.0-rc.1
    1.3.0-beta.1

The version printed by `memryzed --version` matches the version in
the workspace `Cargo.toml` and the most recent tag.

## What changes constitute which kind of bump

The contract Memryzed maintains is broader than just a public Rust
API, since the product is consumed via a CLI, an MCP tool surface,
and a database file. The version bump rules cover all of these:

### MAJOR (X+1.0.0)

Bump MAJOR when any of the following changes in a backward-
incompatible way:

- The MCP tool surface: removing a tool, removing or renaming a
  parameter, changing a parameter type, changing a return shape,
  introducing a new required parameter without a default.
- The CLI surface: removing a command, removing a required flag,
  changing the default behavior of a command, changing the meaning
  of an exit code.
- The database schema in a way that older binaries cannot open the
  newer database.
- The export file format in a way that older versions cannot import
  newer files.
- The configuration file format in a way that requires a manual
  migration.
- The data directory layout in a way that requires user
  intervention to upgrade.

A MAJOR release allows the project to clean up the surface area.
Major releases are infrequent and well-announced.

### MINOR (X.Y+1.0)

Bump MINOR when any of the following is true and no MAJOR-bump
condition is also true:

- A new MCP tool is added.
- A new CLI command is added.
- A new optional flag, parameter, or configuration option is added.
- New optional fields are added to the export format.
- A new platform target ships.
- Performance improvements that change observed latencies but not
  outputs.
- Behavior of an existing feature is intentionally changed in a
  backward-compatible way.

### PATCH (X.Y.Z+1)

Bump PATCH when the change is a bug fix, security fix, or
documentation correction with no behavior change other than fixing
the bug.

If a security fix requires a behavior change that would normally be
a MINOR, it can still be released as a PATCH if not making the change
is more harmful than the inconsistency. The decision is made at
release time and noted in `CHANGELOG.md` and the security advisory.

## Pre-1.0

The Semantic Versioning spec allows breaking changes during 0.x
releases. Memryzed will follow that allowance during the 0.x series
but will document every breaking change clearly in the CHANGELOG and
advance the MINOR version when a breaking change ships. We will not
hide breaking changes in patch releases during 0.x.

The first stable release is 1.0.0. Once 1.0.0 ships, the rules above
apply strictly.

## Backward compatibility windows

For releases at 1.0 and beyond:

- The MCP tool surface within a major version is fully backward
  compatible. Clients that work with 1.0.0 work with 1.x.x for any
  x.
- The database schema within a major version is forward and
  backward compatible for at least one minor version. An older
  binary in the same major series can open a newer database for at
  least one minor bump.
- The export format within a major version is forward compatible.
  An older binary cannot necessarily import a newer export, but it
  fails clearly with a version-mismatch error rather than silently
  losing data.
- Configuration files are forward and backward compatible within a
  major version. New options have defaults; removed options are
  ignored with a warning.

When a major version is released, the previous major is supported
with security and critical-bug-only patches for at least 12 months.

## Pre-releases

Pre-releases use the format `MAJOR.MINOR.PATCH-IDENT.NUM`:

    0.1.0-rc.1
    0.2.0-beta.1

Pre-releases are tagged in the same way as final releases. Install
scripts and Homebrew taps default to the latest non-prerelease.
Users who want to test pre-releases install them explicitly.

## Patch releases on older lines

Memryzed maintains the most recent minor version on each supported
major line. For example, while 1.3.x is the current line, 1.2.x may
also receive critical patches if the 1.2 line is still in active
use. Patch releases on older lines come from the corresponding
`release/X.Y` branch.

The release process for patches is documented in `release-process.md`.

## What does not bump the version

These changes do not require a version bump on their own, though
they typically ride along with a release that does:

- Documentation changes.
- Example or sample changes.
- Test changes.
- Internal refactors.
- Build, CI, or release-pipeline changes that do not affect the
  artifact.
- Comment-only changes.

Bumping a dependency that does not change behavior also does not
require a version bump on its own, but the change still warrants a
CHANGELOG entry under "Changed" if the dependency is user-relevant
(for example, the embedding model crate).

## How the version is recorded

Every release produces:

- A git tag of the form `vMAJOR.MINOR.PATCH` or
  `vMAJOR.MINOR.PATCH-IDENT.NUM`.
- A GitHub Release page summarizing the changes (generated from
  CHANGELOG).
- An entry in CHANGELOG.md with the version and the release date.
- The same version recorded in `Cargo.toml` of every published
  crate and in the binary itself.

`memryzed --version` prints the binary version. `memryzed doctor`
includes the version in its output.

## Aligning published crates

The internal crates `memryzed-core`, `memryzed-mcp`, `memryzed-cli`,
and `memryzed-tui` share a single version. They are versioned and
released in lockstep. We do not publish them as independent libraries
in v1; they exist primarily to keep the binary's source organized.

If we begin publishing them to crates.io for third-party consumption,
the rules above apply per crate. Until then, the version in the
workspace `Cargo.toml` is authoritative.

## Updating the version

The version is bumped only as part of the release process. See
`release-process.md`.
