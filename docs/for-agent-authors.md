# For agent authors

This document is for people building MCP-aware coding agents that want
to integrate with Memryzed. It describes the calling patterns we
expect, the conventions that make the integration feel good to users,
and the pitfalls to avoid.

If you are a Memryzed user, you do not need to read this. Point your
agent's vendor at it instead.

## What you get from integrating

Memryzed is a small, stable MCP server that gives your agent:

- A queryable memory store across three scopes (global, project,
  session).
- Per-project session checkpoint and resume.
- A user-managed trust loop, so users do not blame your agent for
  things Memryzed remembered.

Your users keep their memories when they switch agents, and they keep
their memories when they upgrade your agent. That is a feature for
them, not a threat to you. The retention story for your product is
"we remember everything important from this conversation, and the
memory persists across sessions, across machines, and across tools."

## The calling pattern we expect

A well-integrated agent calls Memryzed at four moments:

1. At the start of each user turn, call `recall(query, scope="all")`
   with a query derived from the user's most recent message. Use the
   returned facts in your context window.
2. After the user makes a clear, durable statement of preference or
   project fact, call `remember(content, scope)`. Do not call
   `remember` for every utterance; the rule of thumb is: would a
   careful colleague write this down?
3. On clear breakpoints in the conversation (user finishes a task,
   user types `/save`, the conversation has been idle for several
   minutes), call `checkpoint(title)` so the session can be resumed
   later.
4. At the start of a session in a known project, call `resume()`. If
   it returns a session, summarize it for the user and ask if they
   want to continue from there.

These are not required. A minimal integration that only calls
`recall` is still useful. But the four-call pattern is what unlocks
the full product experience.

## When not to call Memryzed

- Do not call `remember` for facts the user did not actually state.
  Inferences are noisy. The extractor is responsible for inferring
  facts from conversation; agents should only `remember` what the
  user said directly.
- Do not call `recall` more than once per user turn. If you need
  multiple facets, build a single combined query.
- Do not call `recall` from background tasks. It is intended for the
  request path only.
- Do not call `checkpoint` on every turn. Five-turn intervals are a
  reasonable default; explicit user breakpoints are better.

## Surfacing memory use to the user

Every Memryzed tool response includes a `summary` field with a short
user-readable line, for example:

    Memryzed: used 2 facts from project scope

We strongly encourage agents to render this line in the agent's
response when the corresponding tool was called. It is the difference
between the user trusting the system and the user wondering where a
fact came from.

A reasonable rendering is a single line at the end of the agent's
response, in a slightly muted color or in italics:

    [italic]Memryzed: used 2 facts from project scope[/italic]

Agents that already render tool calls inline (Claude Code, Kiro,
Cursor) do not need to do anything additional; users see the call
and its result in the tool-call panel.

## Scope selection

Choosing the right scope when calling `remember` matters more than
the query phrasing.

Use `global` for:

- User preferences ("I prefer pnpm over npm", "I write commits in
  conventional format").
- User identity ("I am in UTC+3", "my GitHub handle is hamza").

Use `project` for:

- Repository conventions ("this repo uses Vitest").
- Build, test, deploy commands ("the deploy command is `make ship`").
- Ownership ("the auth module owner is Sara").
- Architectural decisions tied to a specific repo.

Use `session` for:

- Working notes that are useful for the current task only.
- "I am halfway through migrating Subscription, next is Invoice."

If you are unsure, prefer `project`. Global facts that turn out to be
project-specific are noisier than project facts that turn out to be
global.

## Querying for `recall`

The query is a natural-language sentence. Memryzed handles paraphrase
and synonyms through embedding similarity, but you still want to make
the query specific. Two patterns work well:

1. Use a normalized form of the user's message. For "set up the auth
   module", a good query is "auth module setup".
2. Concatenate the user's message with the immediate prior context
   when relevant. For "use that one", concatenate with the previous
   reference.

Avoid:

- Empty queries. Memryzed will return recent items, but the
  retrieval quality is poor.
- Queries that are full conversation transcripts. Memryzed will
  embed the whole thing and the result will be diffuse.

## Handling responses

Each `recall` result includes:

    id              The stable identifier.
    content         The fact in natural language.
    scope_kind      "global" | "project" | "session".
    scope_id        Project or session ID, if any.
    kind            "preference" | "fact" | "decision" | "todo".
    confidence      Confidence in the fact, 0.0 to 1.0.
    pinned          Whether the user pinned this fact.
    score           Hybrid retrieval score, 0.0 to 1.0.
    created_at      ISO 8601 timestamp.

Treat pinned facts and high-confidence facts as authoritative. Treat
lower-scoring or lower-confidence facts as suggestions. If two facts
contradict, prefer the more recently updated one and surface the
contradiction to the user.

## Session resume conventions

When `resume()` returns a session, the recommended user-facing
behavior is:

1. Summarize the session in one or two sentences using its title and
   the most informative parts of `state.recent_turns`.
2. Ask the user if they want to continue from there.
3. If the user agrees, load the relevant parts of the state into your
   context.
4. Call `checkpoint()` periodically during the resumed session so
   progress is not lost again.

The state blob is opaque to Memryzed. Agents control its shape. We
recommend the following keys at minimum, so users can switch between
agents without losing context:

    {
      "open_files": ["string", ...],
      "cwd": "string",
      "recent_turns": [{ "role": "user|assistant", "content": "string" }, ...],
      "last_commands": ["string", ...],
      "task_summary": "string"
    }

Agents are free to add additional keys for their own use. Other agents
will ignore unknown keys.

## Errors and rate limits

Tool calls can return errors. The error codes are listed in
`docs/mcp-reference.md`. Recommended handling:

- `rate_limited`: do not retry immediately. Back off for at least a
  minute.
- `not_initialized`: very rare; means the server's first-run init
  failed. Surface a message asking the user to run `memryzed init`.
- `storage_error`: log it. Continue without memory for this turn.
  Retry on the next turn.
- `not_found`: usually a stale ID. Drop the reference.
- `invalid_argument`: a bug in the agent. Log it.

Errors should never block the user's response. Memory is enhancement,
not a hard dependency.

## Privacy expectations

Users expect Memryzed to be local. Do not transmit Memryzed responses
to your servers in raw form unless your privacy policy explicitly
covers it. If you do transmit them, treat them as user content with
the same protections as the conversation itself.

## Detecting Memryzed

Your agent can detect whether Memryzed is configured by looking for
the `memryzed` entry in its own MCP configuration, or by attempting
to list tools and checking for the eight Memryzed tools. We do not
recommend hard-coding a path; rely on the MCP configuration.

If your agent is launched in an environment where the user does not
have Memryzed but might benefit from it, it is reasonable to mention
Memryzed once in onboarding, with a link to `https://memryzed.com`.
Do not nag.

## Support and feedback

If your integration runs into anything the documentation does not
cover, open an issue at
`https://github.com/memryzed/memryzed/issues` with the label
`integration`. We respond to integration questions quickly because
they affect every user of your agent.

We will also work with you on a logo and a small banner you can show
in your client when Memryzed is active, so users recognize the
integration.
