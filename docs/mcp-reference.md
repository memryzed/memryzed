# MCP reference

This document describes the Model Context Protocol surface that
Memryzed exposes to clients. It covers the tools, their parameters,
their return shapes, and the conventions clients should follow.

For guidance on how to integrate cleanly into a coding agent, also
read `docs/for-agent-authors.md`.

## Transport

Memryzed runs as an MCP server over stdio. Clients spawn the
`memryzed serve` process and communicate with it using the standard
MCP framing.

The future `--transport sse` mode is reserved for cases where the
client cannot spawn a child process. It is not implemented in v1.

## Server identity

The server reports the following metadata at handshake:

    name:     memryzed
    version:  the running binary version, for example "0.1.0"
    protocol: the MCP protocol version supported

## Tools

Memryzed exposes eight tools. The set is intentionally small. Clients
should not assume additional tools will be added without a major
version bump.

### `recall`

Find memories relevant to a query.

Parameters:

    query     string, required.    The natural-language query.
    scope     string, optional.    "global" | "project" | "session" | "all". Default: "all".
    limit     integer, optional.   Maximum number of results. Default: 10.

Returns:

    {
      "results": [
        {
          "id": "mem_x1y2z3",
          "content": "User prefers pnpm over npm",
          "scope_kind": "global",
          "scope_id": null,
          "kind": "preference",
          "confidence": 1.0,
          "pinned": true,
          "score": 0.91,
          "created_at": "2026-05-20T10:30:00Z"
        }
      ],
      "summary": "Memryzed: 1 fact found in global scope"
    }

The `summary` field is a one-line, user-readable description that
clients are encouraged to render in the agent's response so users see
that memory was used.

### `remember`

Store a new memory.

Parameters:

    content    string, required.    The fact to remember.
    scope      string, required.    "global" | "project" | "session".
    kind       string, optional.    "preference" | "fact" | "decision" | "todo". Default: "fact".
    ttl_days   integer, optional.   Expire after this many days.

Returns:

    {
      "id": "mem_q4r5s6",
      "status": "approved",
      "summary": "Memryzed: stored 1 new fact in project scope"
    }

Calls to `remember` are auto-approved, since the user is explicitly in
the loop when the agent calls this tool.

### `forget`

Archive a memory by ID.

Parameters:

    id    string, required.

Returns:

    {
      "id": "mem_x1y2z3",
      "status": "archived",
      "summary": "Memryzed: archived 1 fact"
    }

Archived memories are not retrieved but are preserved for audit. To
delete permanently, the user must run `memryzed forget <id> --hard`
from the CLI.

### `list_memories`

List memories without retrieval-style ranking, for transparency or
debugging.

Parameters:

    scope    string, optional.    "global" | "project" | "session" | "all". Default: "all".
    limit    integer, optional.   Default: 50.

Returns:

    {
      "memories": [ /* same shape as recall.results, without score */ ]
    }

### `checkpoint`

Save the current task's session state.

Parameters:

    title    string, optional.    Human-readable title.
    state    object, optional.    Free-form JSON state blob.

If no session is active for the current project, a new one is
created. If a session is active, the state is updated.

Returns:

    {
      "session_id": "sess_t7u8v9",
      "summary": "Memryzed: checkpointed session"
    }

### `resume`

Load a session's state.

Parameters:

    session_id    string, optional.    If absent, resumes the most
                                       recent session for the current
                                       project.

Returns:

    {
      "session": {
        "id": "sess_t7u8v9",
        "title": "Refactoring payments module",
        "project_id": "proj_a1b2c3",
        "state": { /* the saved state blob */ },
        "created_at": "2026-05-27T09:00:00Z",
        "updated_at": "2026-05-27T18:30:00Z"
      },
      "summary": "Memryzed: resumed session from 2026-05-27"
    }

If no sessions exist for the current project, returns:

    {
      "session": null,
      "summary": "Memryzed: no prior sessions in this project"
    }

### `list_sessions`

List sessions for the current project.

Parameters:

    project_id    string, optional.    Default: current project.
    limit         integer, optional.   Default: 10.

Returns:

    {
      "sessions": [
        {
          "id": "sess_t7u8v9",
          "title": "Refactoring payments module",
          "status": "paused",
          "updated_at": "2026-05-27T18:30:00Z"
        }
      ]
    }

### `end_session`

Mark a session as completed.

Parameters:

    session_id    string, required.

Returns:

    {
      "session_id": "sess_t7u8v9",
      "status": "completed",
      "summary": "Memryzed: session ended"
    }

## Errors

All errors follow the MCP error format. The `data` field includes a
machine-readable error code from the following set:

    not_found            The referenced ID does not exist.
    invalid_scope        The scope value is not recognized.
    invalid_argument     A parameter is malformed or missing.
    storage_error        A database or filesystem error occurred.
    not_initialized      The data directory has not been initialized.
    rate_limited         The client is calling tools too frequently.

## Rate limiting

To prevent runaway loops on the agent side, Memryzed applies soft
limits per client per minute:

    recall:         60 calls
    remember:       30 calls
    other tools:    60 calls

When a limit is exceeded, the next call returns `rate_limited` and the
audit log notes the event. Limits reset on a sliding window.

## Project identity

When a client spawns `memryzed serve`, the working directory passed by
the MCP framework determines the project. Memryzed identifies a
project by:

1. The SHA-256 of the git remote URL when one is available
   (`git config --get remote.origin.url`), truncated to 12 hex chars.
2. Otherwise, a hash of the absolute path to the working directory.

Clients do not need to manage project IDs. Memryzed handles this
internally.

## Where MCP client configs live

For reference, the standard locations Memryzed reads when running
`memryzed install`:

    Claude Code:    ~/.claude/mcp.json
    Kiro CLI:       ~/.kiro/settings/mcp.json
    Cursor:         ~/.cursor/mcp.json
    Codex CLI:      ~/.codex/mcp.json
    Copilot CLI:    varies, see the gh-copilot extension docs
    Continue:       ~/.continue/config.json

The exact entry written for each client is shown by:

    memryzed install --client <name> --print

## Versioning of the tool surface

Tools, their parameters, and their return shapes are part of the
public contract. Backward-incompatible changes will only happen on
major versions. See `docs/development/versioning.md` for details.
