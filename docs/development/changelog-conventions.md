# Changelog conventions

This document describes how `CHANGELOG.md` entries are written. Every
pull request that has user-visible effects must add an entry. Internal
refactors, dependency bumps with no user-visible change, and pure CI
changes do not require an entry.

If you are unsure whether a change is user-visible, add an entry. It
is easier to remove an entry during release than to add one
afterwards.

## File format

`CHANGELOG.md` follows the Keep a Changelog format
(https://keepachangelog.com/en/1.1.0/) with the conventions in this
document.

The file structure is:

    # Changelog
    ...
    ## [Unreleased]
    ### Added
    ### Changed
    ### Deprecated
    ### Removed
    ### Fixed
    ### Security

    ## [0.1.0] - 2026-MM-DD
    ### Added
    ...

Released versions are listed newest first. The `[Unreleased]` section
at the top accumulates entries between releases.

## Section meanings

Each entry goes under exactly one of:

- Added: a new feature, command, configuration option, or
  user-visible behavior that did not exist before.
- Changed: an intentional change to existing behavior. If the change
  is backward-incompatible, also note this in the entry; see
  `versioning.md`.
- Deprecated: a feature that still works but will be removed in a
  later major version. The entry should say which version is
  expected to remove it.
- Removed: a feature that previously existed and no longer does.
- Fixed: a bug fix.
- Security: a vulnerability fix or security-relevant change.

A change can in rare cases warrant entries in two sections (for
example, a deprecation that also fixes a bug). Use judgment.

## Entry format

Each entry is a single bullet point. The format:

    - <Imperative summary of the change>. (#<PR number>)

Examples:

    - Add `memryzed pin` command to mark a memory as never-expiring. (#42)
    - Fix `recall` returning archived memories when scope is global. (#57)
    - Bump rate limit on `recall` to 60 calls per minute. (#61)

Rules:

- Use imperative mood ("Add", "Fix", "Remove"), not past tense ("Added",
  "Fixed").
- Capitalize the first letter. End with a period.
- Reference the pull request in parentheses at the end. Use the PR
  number, not the issue number.
- Keep the summary under 100 characters when possible. If more
  context is needed, write a second sentence after the period.
- Use backticks for code identifiers (`recall`, `--json`,
  `[memory] auto_approve_threshold`).

## What is user-visible

A change is user-visible if any of these is true:

- It changes the output of any CLI command, including help text,
  error messages, or exit codes.
- It changes the parameters or return shape of any MCP tool.
- It changes any file that Memryzed reads or writes, including
  `config.toml`, the database schema, the audit log format, or the
  export format.
- It changes the install scripts or any artifact produced by the
  release pipeline.
- It changes any documented behavior.

A change is not user-visible if it only:

- Refactors internals without changing externally observable
  behavior.
- Adjusts logging at debug level or below.
- Changes test code.
- Bumps a dependency without changing behavior.
- Changes CI configuration.

## Examples of good entries

Added:

    - Add `memryzed export --pretty` flag to produce indented JSON. (#103)

    - Add `--client` filter to `memryzed log` for filtering events
      by source MCP client. (#118)

Changed:

    - Default value of `[retrieval] vector_weight` increased from
      0.5 to 0.6 based on retrieval-quality measurements on a
      reference dataset. (#127)

    - `memryzed install` now backs up each modified config file to
      `<file>.memryzed.bak` before writing. (#131)

Deprecated:

    - Deprecate the `--legacy-format` flag on `memryzed export`. It
      remains available in the 0.x series and will be removed in
      1.0. (#142)

Removed:

    - Remove the `MEMRYZED_LEGACY_PATHS` environment variable that
      was deprecated in 0.3.0. (#145)

Fixed:

    - Fix a panic in `recall` when the FTS index was not yet built
      on a freshly initialized database. (#157)

    - Fix `memryzed sessions` showing archived sessions when no
      `--status` flag was passed. (#163)

Security:

    - Patch CVE-2026-XXXX in the `sqlite-vec` extension. Updating
      requires no user action. (#174)

## What not to write

- Do not write entries describing internal refactors. "Refactor the
  retrieval module" is not a user-visible change.
- Do not write entries for documentation-only PRs unless the
  documentation describes new behavior that did not previously have
  documentation. Documentation fixes for existing behavior do not
  belong in the changelog.
- Do not write multiple bullets for one logical change. Combine.
- Do not include emojis or marketing language. The changelog is a
  factual record.

## Workflow during a PR

When you open a pull request:

1. Edit `CHANGELOG.md`.
2. Find the `[Unreleased]` section.
3. Find the appropriate subsection (Added, Changed, etc.). If the
   subsection is empty, the convention is to leave a `(none)` line
   in place and replace it with your entry.
4. Add your entry.
5. Reference the PR number after the period. If you do not yet know
   the number, leave a placeholder like `(#xxx)` and update it
   before merge.

The CI does not currently fail on missing changelog entries, but
reviewers will request one. We may add automated enforcement in the
future.

## Workflow at release time

The release workflow renames `[Unreleased]` to the new version and
adds a date. It also creates a fresh empty `[Unreleased]` section
above. See `release-process.md`.

Before tagging a release, the maintainer cutting it reviews every
entry under `[Unreleased]`:

- Removes entries for changes that were reverted before release.
- Reorders entries within a section by significance, most
  significant first.
- Combines duplicate or near-duplicate entries.
- Fixes typos and tightens phrasing.

Once the release is tagged, the version section is frozen. Errata go
in the `[Unreleased]` section of the next version.

## Errata

If a changelog entry is materially wrong (claims behavior that does
not exist, omits a breaking change), file a documentation fix:

1. Add the corrected entry under the next version's `[Unreleased]`
   section, prefixed with `Errata for vX.Y.Z:`.
2. Do not modify previously released sections, except to add a
   pointer to the errata entry.

This keeps the historical record honest without rewriting it.
