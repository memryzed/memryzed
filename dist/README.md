# Distribution artifacts

This directory holds the release and installation tooling for
Memryzed.

## Install scripts

- `install.sh` — macOS, Linux, WSL. Served from
  `https://memryzed.com/install.sh`.
- `install.ps1` — Windows PowerShell. Served from
  `https://memryzed.com/install.ps1`.
- `install.cmd` — Windows Command Prompt shim that bootstraps
  PowerShell. Served from `https://memryzed.com/install.cmd`.

These are the canonical hand-written installers. They download
release archives from GitHub Releases, verify SHA-256 checksums,
install the binary to the standard location, and update PATH.

To publish them, copy the three files to the website's `public/`
directory (or wherever `memryzed.com` serves static files from).
They are committed here so they are version-controlled alongside
the code they install.

## Release pipeline

The release workflow lives at `.github/workflows/release.yml`. It
triggers on a version tag (`vX.Y.Z` or a pre-release `vX.Y.Z-...`),
builds the binary for every supported target, packages each into a
`.tar.gz` (Unix) or `.zip` (Windows) with a `.sha256` sidecar, and
publishes them to a GitHub Release.

cargo-dist configuration lives in the root `Cargo.toml` under
`[workspace.metadata.dist]`. When the GitHub repository exists, run:

    cargo install cargo-dist
    cargo dist init

to regenerate `.github/workflows/release.yml` with the exact matrix
cargo-dist expects, then commit the result. The hand-written
workflow here is the interim reference and produces compatible
artifact names (`memryzed-<target>.tar.gz`, `.zip`) so the install
scripts work either way.

## Supported targets

    aarch64-apple-darwin
    x86_64-apple-darwin
    x86_64-unknown-linux-gnu
    x86_64-unknown-linux-musl
    aarch64-unknown-linux-gnu
    x86_64-pc-windows-msvc
    aarch64-pc-windows-msvc

## Cutting a release

See `docs/development/release-process.md` for the full procedure.
The short version, once the repo and tags exist:

    git tag -a v0.1.0 -m "Memryzed 0.1.0"
    git push origin v0.1.0

The workflow does the rest.
