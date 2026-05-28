# Incident response

This document describes how the Memryzed maintainers respond to
incidents that affect users. An incident is anything that prevents
users from successfully installing, running, or trusting Memryzed.

For ordinary bugs, the regular issue and patch flow applies. This
document covers events that need a coordinated response.

## What counts as an incident

An incident is any of the following:

- The install endpoint at `memryzed.com` is down or serving the
  wrong content.
- A released binary is broken on at least one supported platform.
- A released binary corrupts user data.
- A confirmed security vulnerability that requires a coordinated
  patch release.
- The release pipeline produces unsigned or wrong-checksum
  artifacts.
- Sustained unavailability of the GitHub Releases assets that
  users' install scripts depend on.

Routine bugs in features, missed CI runs, or single-user
configuration issues are not incidents.

## Severity levels

We use three severity levels.

### Severity 1: critical

The product is broken for many users in a way that is not their
fault, or there is an active threat to user data or trust. Examples:

- The install script returns success but installs nothing.
- A released binary deletes the user's data on first run.
- A vulnerability allowing memory disclosure across scope
  boundaries.

Response: drop other work, page maintainers, ship a fix as quickly
as it can be done correctly.

### Severity 2: major

A meaningful subset of users is affected; the product is unusable
on a platform or in a configuration that is supposed to work.
Examples:

- The Windows binary in the latest release crashes on startup on
  Windows 10.
- `memryzed install` rewrites a config file in a way that breaks
  the client.
- A regression in retrieval that produces empty results for many
  reasonable queries.

Response: prioritize a fix, ship within a day or two, communicate
to affected users.

### Severity 3: minor

The product still works for most users but a known issue exists.
Examples:

- A specific edge case produces a confusing error message.
- A documented platform performs worse than its target latency.

Response: file an issue, fix in the next regular release.

## Roles during an incident

- Incident commander: a maintainer who coordinates the response.
  Decides scope, severity, and when the incident is resolved. Makes
  the call to ship.
- Communicator: drafts and posts user-facing communications.
- Implementer: writes the fix.

For solo or two-person maintainership, one person fills multiple
roles. The roles are separated here so the responsibilities are
clear, not because each requires a different person.

## Response phases

### Detect

How an incident is detected:

- A user reports it through GitHub Issues, the security email, or
  another support channel.
- A maintainer notices it during routine use.
- A monitoring or smoke-test failure surfaces it.

The first maintainer to confirm the report writes a brief
acknowledgment in the issue or report and assigns themselves as
incident commander pending another maintainer's response.

### Triage

The incident commander:

1. Assigns a severity (1, 2, or 3).
2. Decides the response scope: which users are affected, on which
   platforms, and on which versions.
3. Opens an internal tracking issue (or the existing one is
   relabeled `incident:active`) and links to all relevant artifacts.
4. For Sev 1 and Sev 2, posts an initial public acknowledgment in
   the affected issue or in a pinned project notice.

For security incidents, follow the disclosure process in
`SECURITY.md`. The acknowledgment is private until the advisory is
ready.

### Mitigate

Mitigation is anything that reduces user harm before a full fix is
shipped. Examples:

- Marking a broken release as a pre-release on GitHub so install
  scripts skip it.
- Reverting a change on the install endpoint.
- Posting a workaround in a pinned issue.
- Pausing the auto-update notice until the new release is ready.

The incident commander explicitly authorizes mitigations that
modify production state.

### Fix

The implementer prepares a fix:

1. Branches from the appropriate base (usually `main`, sometimes a
   release branch for a backport).
2. Writes the fix and a test that demonstrates it.
3. Opens a PR with `incident: <id>` in the title.
4. Reviewer signs off.
5. PR merges.

For Sev 1, every step is fast-tracked. CI still runs to completion;
do not bypass tests.

### Release

Cut a patch release using the patch-release flow in
`release-process.md`.

Special considerations during incidents:

- The CHANGELOG entry should clearly describe the fix and reference
  the incident.
- For security fixes, publish the advisory and the release together.
- Verify on multiple platforms before announcing the fix as
  resolved.

### Communicate

Once the fix is released:

1. Post an update in the original issue or report, linking to the
   release.
2. For Sev 1 and Sev 2, post a public notice on the project's
   announcement channels.
3. Update any pinned issues. Unpin once the situation is settled.

Communications should be:

- Factual: state what happened, what was affected, what was done.
- Timely: prefer a partial update now over a complete update
  later.
- Honest: do not minimize impact or deflect responsibility. If we
  caused the issue, say so.
- Brief: one or two paragraphs is enough for most updates.

A good post-incident message looks like:

    Memryzed 0.3.1 has been released to fix an installation issue
    that affected new users on macOS Apple Silicon in 0.3.0. Users
    who installed 0.3.0 successfully are unaffected; users who saw
    "binary cannot be opened" should reinstall using the standard
    install command. We are sorry for the disruption.

Avoid:

- Excuses, blame-shifting, or vague language.
- Marketing voice.
- Claims of "no impact" without verification.

### Resolve

The incident is resolved when:

- The fix is released and verified in production.
- Affected users have been notified.
- The tracking issue is closed.
- A short retrospective note is added to the issue or to a separate
  log.

## Retrospectives

After every Sev 1 or Sev 2 incident, the incident commander writes a
short retrospective. The format:

    Incident: <one-line summary>
    Severity: 1 | 2
    Detected: <date and time>
    Resolved: <date and time>

    Timeline:
    - <time>: <event>
    - <time>: <event>
    ...

    What went well:
    - <bullet>
    ...

    What went wrong:
    - <bullet>
    ...

    Action items:
    - <bullet, with owner>
    ...

The retrospective is committed to the repository under
`docs/incidents/YYYY-MM-DD-<slug>.md` and linked from the closed
incident issue. Retrospectives are public unless they contain
information that should remain private (security details before the
advisory is published, user PII).

The goal of a retrospective is to learn, not to assign blame. Action
items should be concrete: change in process, documentation, or
code.

## Special cases

### Install endpoint outage

If `memryzed.com/install.sh` is unreachable:

1. Confirm with multiple checks from different networks.
2. Post a notice with the workaround: download the binary directly
   from GitHub Releases.
3. Investigate. Most likely causes: hosting provider issue, DNS
   issue, propagation issue after a deployment.
4. Once resolved, monitor for an additional hour to confirm.

### Compromised release artifact

If a published release artifact has been tampered with or has a
signature mismatch:

1. Mark the release as broken on GitHub.
2. Update the install scripts to refuse the affected version.
3. Investigate the source: pipeline configuration, signing key,
   hosting infrastructure.
4. Rotate signing keys if compromise is confirmed.
5. Cut a clean patch release.
6. Publish a security advisory describing the timeline and
   mitigations.

This is the most serious case Memryzed can encounter. Treat it as
Sev 1 unconditionally.

### Persistent platform-specific failure

If a single platform fails persistently across multiple releases:

1. Open a Sev 2 tracking issue.
2. Pause new feature work that depends on that platform.
3. Add CI smoke tests that would have caught the failure.
4. Resolve through normal release flow once a fix is in.

## On-call

There is no formal on-call rotation for Memryzed in v1. Maintainers
respond when available. As the user base grows and we add staffed
coverage, this document will be updated with the rotation rules.

If you discover an incident and no maintainer is responding within a
few hours, escalate by:

- Opening a GitHub issue with `incident:active` and `severity:N`
  labels.
- Mentioning maintainers explicitly.
- Posting in any other channels listed in `SUPPORT.md` if that file
  exists.
