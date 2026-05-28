# Security Policy

## Reporting a vulnerability

If you believe you have found a security vulnerability in Memryzed, do
not open a public issue. Report it privately so that a fix can be
prepared and released before details become public.

Send your report to `security@memryzed.com`. If you prefer encrypted
communication, request a PGP key in your initial message and one will
be provided.

Include in your report:

- A description of the issue.
- The Memryzed version and operating system where it was observed.
- Steps to reproduce, including any proof-of-concept code or input.
- The impact you believe it has.
- Whether you have shared this information with anyone else.

## What to expect

- An acknowledgment within three business days.
- A triage update within seven days, including our initial assessment
  of severity and a tentative timeline.
- Coordinated disclosure: we will work with you on a timeline that
  gives users a chance to update before details are published.
- Credit in the release notes if you wish to be credited.

We do not currently run a paid bug bounty program.

## Scope

In scope:

- The Memryzed binary and supporting code in this repository.
- The install scripts published at `memryzed.com`.
- Official release artifacts (binaries, archives, package manifests).

Out of scope:

- Third-party MCP clients that integrate with Memryzed. Report those to
  their respective vendors.
- Vulnerabilities in third-party dependencies that have already been
  publicly disclosed; please report those upstream first. We track and
  patch these through normal release cycles.
- Issues that require an attacker to already have shell access as the
  user running Memryzed are generally not in scope, since at that point
  the attacker can read or modify any data the user can.

## Severity guidelines

We use these rough categories to triage reports:

- Critical: remote attacker can read or modify another user's memories,
  bypass scope isolation, or execute code without local access.
- High: local attacker can escalate privilege via Memryzed; install
  scripts can be tricked into installing a malicious binary.
- Medium: information disclosure with limited impact; denial of service.
- Low: hardening improvements; defense in depth.

## Patch and disclosure timing

For confirmed vulnerabilities:

- Critical and high: target a fix and release within 14 days of
  triage. Publish an advisory and the fix together.
- Medium: target a fix in the next scheduled release.
- Low: address in normal development.

The release process for security patches is documented in
`docs/development/release-process.md`. The incident response procedure
is in `docs/development/incident-response.md`.

## Public advisories

Security advisories are published on the repository's GitHub Security
Advisories page and noted in the `Security` section of `CHANGELOG.md`
for the release that contains the fix.
