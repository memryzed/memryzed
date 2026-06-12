# Memryzed documentation

This directory contains the full documentation set for Memryzed.

## For users

Start here if you are using Memryzed:

- `getting-started.md` - install, initialize, wire it into your agent.
- `concepts.md` - the memory model, scopes, and sessions.
- `cli-reference.md` - every command and flag.
- `mcp-reference.md` - every MCP tool, parameter, and return shape.
- `configuration.md` - every option in `config.toml`.
- `troubleshooting.md` - what to do when something is wrong.
- `faq.md` - common questions and clear answers.

## For agent and client authors

If you are building an MCP-aware coding agent and want to integrate
with Memryzed cleanly:

- `for-agent-authors.md` - calling patterns, scope selection, error
  handling, privacy expectations.

## For contributors and operators

- `architecture.md` - how Memryzed is built internally.
- `data-model.md` - the on-disk format and database schema.
- `roadmap.md` - what is planned next.

## Specifications

The original design specification, kept as a historical record (the
shipped product has evolved since; `CHANGELOG.md` and the user docs
are authoritative):

- `specs/v1.md`
- `specs/benchmarks.md` - the quality-benchmarks plan: which
  datasets we run, which metrics we publish, and the honesty
  principles that govern those numbers.

## Development and operations

For contributors working on Memryzed and for maintainers running
releases:

- `development/README.md` - index for the development docs.
- `development/setup.md` - dev environment setup.
- `development/branching.md` - git workflow and branch naming.
- `development/testing.md` - how tests are organized and run.
- `development/release-process.md` - cutting and shipping a release.
- `development/changelog-conventions.md` - how to write CHANGELOG entries.
- `development/versioning.md` - semver policy and what counts as a
  breaking change.
- `development/incident-response.md` - what to do when something
  breaks in production.

## Reading order

If this is your first time:

1. `../README.md` (project root)
2. `getting-started.md`
3. `concepts.md`
4. `cli-reference.md` and `mcp-reference.md` (skim, refer back as
   needed)

If you are evaluating Memryzed for use in your product:

1. `../README.md`
2. `concepts.md`
3. `for-agent-authors.md`
4. `architecture.md`

If you are contributing:

1. `../CONTRIBUTING.md`
2. `development/setup.md`
3. `architecture.md`
4. `data-model.md`
5. `specs/v1.md`
