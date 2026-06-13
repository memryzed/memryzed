# Distribution artifacts

This directory holds the installation tooling for Memryzed.

## Install scripts

- `install.sh` — macOS, Linux, WSL. Served from
  `https://memryzed.com/install.sh`.
- `install.ps1` — Windows PowerShell. Served from
  `https://memryzed.com/install.ps1`.
- `install.cmd` — Windows Command Prompt shim that bootstraps
  PowerShell. Served from `https://memryzed.com/install.cmd`.

These are the canonical, hand-written installers. Each one detects the
platform, downloads the matching release archive **from
`memryzed.com/releases/`** (resolving the version from
`releases/latest.txt`), verifies its SHA-256 checksum, installs the
binary to the standard location, and updates PATH. They install
atomically, so an upgrade does not need the user to quit a running
agent.

They are committed here so the install logic is version-controlled
next to the code it installs and is auditable by anyone who runs
`curl ... | bash`.

### Keeping the served copy in sync

`memryzed.com` serves its own copy of these files (from the website
project's `public/` directory). This directory is the source; when a
script changes, the website copy must be updated too. Treat the two
as one logical artifact and update them together to avoid drift.

## Release pipeline

The release workflow lives at `.github/workflows/release.yml`. It
triggers on a version tag (`vX.Y.Z`, or a pre-release `vX.Y.Z-...`)
and:

- Builds the `memryzed` binary for every supported target on a native
  runner (no cross-compiling).
- Smoke-tests each binary (`memryzed --version`) on its own
  architecture.
- Packages each into a `.tar.gz` (Unix) or `.zip` (Windows) with a
  `.sha256` sidecar, named `memryzed-<target>.{tar.gz,zip}` so the
  install scripts find them.
- Generates a build-provenance attestation for each artifact.
- Publishes everything to a GitHub Release.

The pipeline has **no AWS or deploy credentials**. Promoting a release
to `memryzed.com/releases/` is a separate manual step performed from a
trusted machine; see `docs/development/release-process.md`.

## Supported targets

    x86_64-unknown-linux-gnu     (glibc 2.35+, self-contained binary)
    aarch64-unknown-linux-gnu    (glibc 2.27+, ships libonnxruntime.so)
    aarch64-apple-darwin         (Apple Silicon)
    x86_64-pc-windows-msvc

Most targets are a single self-contained binary. aarch64 Linux is the
exception: pyke's prebuilt ONNX Runtime for ARM requires glibc 2.38+,
so we instead link Microsoft's official ARM build (glibc ~2.27) and
ship its `libonnxruntime.so` next to the binary. The install scripts
place that library alongside the executable.

Not built:

- Intel macOS (x86_64-apple-darwin): GitHub's free macos-13 runners
  sit queued indefinitely. Apple Silicon covers current Macs.
- musl (Alpine) and Windows on ARM: the embedding runtime requires
  glibc, and Windows-ARM ONNX support is unverified.

## Cutting a release

See `docs/development/release-process.md` for the full procedure. The
short version, once the repo and tags exist:

    git tag -a v0.7.0 -m "Memryzed 0.7.0"
    git push origin v0.7.0

CI builds, attests, and publishes the GitHub Release. You then promote
the artifacts to `memryzed.com` from a trusted machine.
