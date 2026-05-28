# Contributing to Memryzed

Thank you for your interest in contributing. Memryzed is an open-source
project under the Apache-2.0 license, and we welcome contributions of
code, documentation, bug reports, and design feedback.

This document covers the rules of the road. For technical setup, see
`docs/development/setup.md`.

## Before you start

For anything beyond a small bug fix or documentation correction, please
open an issue first to discuss the change. This avoids wasted work and
helps the design conversation happen before code.

By submitting a contribution you agree that your work is licensed under
Apache-2.0 and that you have the right to submit it. We do not require a
separate Contributor License Agreement.

## How to contribute

1. Fork the repository and clone your fork.
2. Create a topic branch from `main`. See
   `docs/development/branching.md` for the branch-naming convention.
3. Make your change. Follow the coding conventions in
   `docs/development/setup.md`.
4. Add or update tests. See `docs/development/testing.md`.
5. Update documentation if your change affects user-visible behavior.
6. Add an entry to `CHANGELOG.md` under the `[Unreleased]` section. See
   `docs/development/changelog-conventions.md` for the format.
7. Open a pull request against `main`.

## Pull request expectations

A pull request is ready to merge when:

- The change is described clearly in the PR description, including
  motivation, approach, and any tradeoffs.
- All tests pass in CI.
- The change is covered by tests where it makes sense to test.
- Documentation is updated where the change affects it.
- The CHANGELOG has an entry.
- At least one maintainer has approved.

Small, focused pull requests are easier to review than large ones. If
your change is large, consider splitting it.

## Reporting bugs

Open a GitHub issue with:

- The version of Memryzed (`memryzed --version`).
- Your operating system and architecture.
- The MCP client or clients involved, if relevant.
- The steps to reproduce.
- What you expected to happen.
- What actually happened, including any error output.

For a vulnerability, do not open a public issue. Follow the process in
`SECURITY.md`.

## Proposing features

Open a GitHub issue describing the feature, the use case, and why it
belongs in Memryzed rather than in a downstream tool. Include any
alternatives you considered. We try to keep the surface area small;
not every good idea ships in core.

## Documentation contributions

Documentation fixes are welcome and do not need a prior issue. The
project's documentation is in the `docs/` tree. The README, CHANGELOG,
and other top-level files live at the repository root.

## Code of conduct

All contributors are expected to follow the `CODE_OF_CONDUCT.md`. Report
violations through the channel described in that document.

## Maintainership

Maintainers are listed in `MAINTAINERS.md` once that file exists. The
initial maintainer is the project author. New maintainers are added
based on sustained, high-quality contributions and on consensus among
existing maintainers.
