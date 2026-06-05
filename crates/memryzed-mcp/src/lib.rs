// Copyright 2026 Memryzed contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! MCP server for Memryzed.
//!
//! Exposes the Memryzed tools to MCP-aware clients over stdio. The
//! tool surface is documented in `docs/mcp-reference.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use memryzed_core::clock::now_epoch_seconds;
use memryzed_core::embedder::{make_default, Embedder};
use memryzed_core::memory::{
    archive, insert_with_embedder, list as list_memories_core, ListFilter, Memory, NewMemory,
    Scope, Status,
};
use memryzed_core::retrieval::{search as retrieval_search, SearchOptions};
use memryzed_core::{extractor, projects, sessions, Database};

/// Spawn the background capture-and-index engine on a dedicated OS
/// thread.
///
/// This is what makes Memryzed work with no commands beyond install:
/// the agent spawns `serve`, which calls this, and the engine then
/// runs entirely on its own thread with its own SQLite connection.
/// Keeping it off the async MCP runtime is deliberate: embedding is
/// blocking CPU work, and running it here means it can never stall
/// tool calls or saturate the protocol threads.
///
/// The engine is gentle by design. Embedding is the expensive part,
/// so each batch is small and followed by a sleep, capping CPU use at
/// a fraction of one core. A first-run backfill therefore takes
/// longer but stays invisible: no fan spin, no contention. Capture
/// (mining new turns) runs only periodically. Everything is
/// idempotent and resumable, so a restart simply continues.
///
/// The thread is detached; it exits when the process does. Errors are
/// logged, never propagated.
pub fn spawn_engine(data_dir: memryzed_core::DataDir, home: std::path::PathBuf) {
    std::thread::Builder::new()
        .name("memryzed-engine".into())
        .spawn(move || engine_loop(data_dir, home))
        .ok();
}

fn engine_loop(data_dir: memryzed_core::DataDir, home: std::path::PathBuf) {
    use memryzed_core::engine::{resolve_profile, EngineLock};
    use memryzed_core::mining::{mine_all, MineOptions, Source};
    use std::time::{Duration, Instant};

    // Single-instance lock: every agent session spawns its own serve,
    // but only one may run the embedding engine, otherwise CPU
    // multiplies with the number of open sessions. If another process
    // holds the lock, this serve still answers tool calls but does no
    // background work.
    let _lock = match EngineLock::acquire(&data_dir.engine_lock()) {
        Some(lock) => lock,
        None => {
            tracing::info!(
                target: "memryzed::engine",
                "another serve already runs the engine; skipping background work here"
            );
            return;
        }
    };

    // Throughput profile (gentle by default, configurable to go
    // faster). Resolved once at startup from env / config.
    let profile = resolve_profile(&data_dir.config_file());
    let reindex_batch = profile.batch();
    let embed_pause_ms = profile.pause_ms();
    tracing::info!(
        target: "memryzed::engine",
        profile = profile.as_str(),
        batch = reindex_batch,
        pause_ms = embed_pause_ms,
        "background engine started"
    );

    // Capture new transcript content at most this often.
    const CAPTURE_EVERY: Duration = Duration::from_secs(30);
    // Poll interval when there is nothing to embed.
    const IDLE: Duration = Duration::from_secs(15);

    // The engine owns its own connection and embedder, independent of
    // the MCP server. WAL mode allows this second connection to the
    // same database file.
    let mut db = match Database::open(&data_dir.db_file()) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(target: "memryzed::engine", error = %e, "engine could not open db");
            return;
        }
    };
    let embedder = make_default(&data_dir.models_dir()).ok();

    let mut last_capture = Instant::now()
        .checked_sub(CAPTURE_EVERY)
        .unwrap_or_else(Instant::now);

    loop {
        // 1. Capture new conversation text, periodically and text-only
        //    (instant). Idempotent and incremental.
        if last_capture.elapsed() >= CAPTURE_EVERY {
            let opts = MineOptions {
                source: Source::Auto,
                threshold: 0.85,
                dry_run: false,
                force: false,
                incremental: true,
            };
            let noop = memryzed_core::NoopEmbedder;
            if let Err(e) = mine_all(&mut db, &noop, &home, &opts, now_epoch_seconds()) {
                tracing::warn!(target: "memryzed::engine", error = %e, "capture failed");
            }
            last_capture = Instant::now();
        }

        // 2. Embed one small batch, then pause. When nothing was
        //    embedded, idle longer.
        let embedded = match &embedder {
            Some(e) if e.is_active() => {
                match memryzed_core::episodes::reindex_pending(&mut db, e.as_ref(), reindex_batch) {
                    Ok(n) => n,
                    Err(err) => {
                        tracing::warn!(target: "memryzed::engine", error = %err, "reindex failed");
                        0
                    }
                }
            }
            _ => 0,
        };

        if embedded > 0 {
            std::thread::sleep(Duration::from_millis(embed_pause_ms));
        } else {
            std::thread::sleep(IDLE);
        }
    }
}

/// State shared across every tool call.
///
/// The database is wrapped in a tokio `Mutex` because rusqlite
/// connections are `Send` but not `Sync`; serializing access is
/// sufficient for v1 single-user workloads.
#[derive(Clone)]
pub struct MemryzedServer {
    inner: Arc<Inner>,
    #[allow(dead_code)]
    tool_router: ToolRouter<MemryzedServer>,
}

struct Inner {
    db: Mutex<Database>,
    embedder: Arc<dyn Embedder>,
    /// Project id for the working directory the server was launched
    /// in. Resolved once at startup; session tools operate on it.
    project_id: Mutex<Option<String>>,
}

impl MemryzedServer {
    /// Construct a server backed by the given data directory.
    ///
    /// Opens the database (running migrations) and loads the
    /// embedder. The embedder is the same one CLI commands use, so
    /// `MEMRYZED_DISABLE_EMBEDDING` honors the same environment
    /// variable.
    pub fn open(data_dir: &memryzed_core::DataDir) -> anyhow::Result<Self> {
        let db = Database::open(&data_dir.db_file())?;
        let embedder = make_default(&data_dir.models_dir())?;
        // Resolve the project for the current working directory and
        // record it, so session tools have a scope without the agent
        // needing to pass one.
        let project_id = std::env::current_dir()
            .ok()
            .and_then(|cwd| projects::ensure_for_cwd(&db, &cwd, now_epoch_seconds()).ok())
            .map(|p| p.id);
        Ok(Self {
            inner: Arc::new(Inner {
                db: Mutex::new(db),
                embedder,
                project_id: Mutex::new(project_id),
            }),
            tool_router: Self::tool_router(),
        })
    }

    /// Convenience constructor used by tests with an in-memory database.
    pub fn from_parts(db: Database, embedder: Arc<dyn Embedder>) -> Self {
        Self {
            inner: Arc::new(Inner {
                db: Mutex::new(db),
                embedder,
                project_id: Mutex::new(None),
            }),
            tool_router: Self::tool_router(),
        }
    }

    /// Run the background capture-and-index engine forever.
    ///
    /// This is what makes Memryzed work with no commands beyond
    /// install: the agent spawns `serve`, and this loop runs behind
    /// it. Each cycle it (1) mines every detected agent transcript
    /// directory incrementally and text-only, so new conversation is
    /// captured instantly, then (2) embeds a bounded batch of
    /// not-yet-embedded episodes. Both steps are cheap per cycle and
    /// fully resumable, so nothing ever blocks and a restart just
    /// continues where it left off.
    ///
    /// Errors are logged, never fatal: the agent's session must not
    /// be affected by a background hiccup.
    pub async fn run_background_engine(&self, home: std::path::PathBuf) {
        let _ = home;
        // Superseded by `spawn_engine`, which runs the engine on a
        // dedicated OS thread with its own database connection so the
        // blocking embedding work never touches the async MCP runtime.
        // Kept as a no-op for API stability.
    }

    /// Test helper: set the active project id.
    #[doc(hidden)]
    pub async fn set_project_for_test(&self, project_id: impl Into<String>) {
        *self.inner.project_id.lock().await = Some(project_id.into());
    }

    async fn require_project(&self) -> Result<String, McpError> {
        self.inner.project_id.lock().await.clone().ok_or_else(|| {
            McpError::invalid_params(
                "no project for the current working directory; sessions require a project",
                None,
            )
        })
    }
}

#[tool_router]
impl MemryzedServer {
    /// Find memories relevant to a query.
    #[tool(
        description = "Search the user's memory of past conversations and facts across every \
agent they use (this one and others). CALL THIS PROACTIVELY, without being asked, BEFORE answering \
whenever the user refers to earlier work, prior decisions, 'what we discussed', past sessions, or \
anything that may have been established before, and at the start of a task to load relevant \
context. \
\
WRITING A GOOD QUERY: include the concrete entities you are asking about, by name, the file, \
service, function, error, library, or decision, not just a vague topic. 'eks spot node group \
terraform' recalls far better than 'infrastructure'. Use the user's own words and identifiers. \
\
To get the most recent conversations instead of the most relevant (for 'what did we last \
discuss', 'what were we working on'), set order to 'recent'. \
\
Returns matching facts and verbatim conversation excerpts, each with its surrounding turns, \
source agent, and time, so you can read the exchange in context. When in doubt, call it: it is \
cheap and missing relevant memory gives the user a worse answer."
    )]
    async fn recall(
        &self,
        Parameters(args): Parameters<RecallArgs>,
    ) -> Result<CallToolResult, McpError> {
        let scope = parse_scope_opt(args.scope.as_deref())?;
        let opts = SearchOptions {
            scope,
            scope_id: None,
            limit: args.limit.unwrap_or(10).max(1) as usize,
            ..Default::default()
        };
        let db = self.inner.db.lock().await;
        let results = retrieval_search(&db, self.inner.embedder.as_ref(), &args.query, &opts)
            .map_err(core_to_mcp)?;
        // Also recall verbatim conversation turns (episodes) so a
        // conversation held in one agent surfaces in another. When
        // ordered "recent", return the latest turns by time instead.
        let now = now_epoch_seconds();
        let limit = args.limit.unwrap_or(10).max(1) as usize;
        let recent_mode = args
            .order
            .as_deref()
            .map(|o| o.eq_ignore_ascii_case("recent"))
            .unwrap_or(false);
        let episodes = if recent_mode {
            memryzed_core::episodes::recent(&db, limit, now)
        } else {
            memryzed_core::episodes::recall(
                &db,
                self.inner.embedder.as_ref(),
                &args.query,
                limit,
                now,
            )
        }
        .map_err(core_to_mcp)?;
        drop(db);

        let payload = RecallResponse {
            results: results
                .iter()
                .map(|r| RecallHit {
                    id: r.memory.id.clone(),
                    content: r.memory.content.clone(),
                    scope_kind: r.memory.scope.as_db_str().to_string(),
                    scope_id: r.memory.scope_id.clone(),
                    kind: r.memory.kind.as_db_str().to_string(),
                    confidence: r.memory.confidence,
                    pinned: r.memory.pinned,
                    score: r.score,
                    created_at: r.memory.created_at,
                })
                .collect(),
            episodes: episodes
                .iter()
                .map(|e| EpisodeRecallHit {
                    id: e.episode.id.clone(),
                    role: e.episode.role.clone(),
                    content: e.episode.content.clone(),
                    source_agent: e.episode.source_agent.clone(),
                    score: e.score,
                    created_at: e.episode.created_at,
                    excerpt: render_excerpt(&e.context, &e.episode),
                })
                .collect(),
            summary: render_summary(results.len(), &episodes, recent_mode),
        };
        Ok(CallToolResult::success(vec![Content::text(json_string(
            &payload,
        )?)]))
    }

    /// Store a new memory.
    #[tool(
        description = "Store a durable fact in the user's long-term memory. CALL THIS \
PROACTIVELY when the user states a lasting preference, decision, convention, or fact worth \
remembering across sessions (for example 'I prefer X', 'we use Y', 'the deploy command is Z'), \
without waiting to be asked. Auto-approved; the user stays in control via review."
    )]
    async fn remember(
        &self,
        Parameters(args): Parameters<RememberArgs>,
    ) -> Result<CallToolResult, McpError> {
        let scope = parse_scope(&args.scope)?;
        if scope == Scope::Session {
            return Err(McpError::invalid_params(
                "session-scoped memories require an active session; use the checkpoint tool",
                None,
            ));
        }
        if scope == Scope::Project && args.scope_id.is_none() {
            return Err(McpError::invalid_params(
                "project-scoped remember requires a scope_id",
                None,
            ));
        }
        let mut new = NewMemory::new(scope, args.content);
        new.scope_id = args.scope_id;
        if let Some(k) = args.kind {
            new.kind = k.parse().map_err(core_to_mcp)?;
        }
        if let Some(days) = args.ttl_days {
            new.expires_at = Some(now_epoch_seconds() + i64::from(days) * 86_400);
        }

        let mut db = self.inner.db.lock().await;
        let now = now_epoch_seconds();
        let memory = insert_with_embedder(&mut db, new, self.inner.embedder.as_ref(), now)
            .map_err(core_to_mcp)?;
        drop(db);

        let payload = RememberResponse {
            id: memory.id.clone(),
            status: memory.status.as_db_str().to_string(),
            summary: format!(
                "Memryzed: stored 1 fact in {} scope",
                memory.scope.as_db_str()
            ),
        };
        Ok(CallToolResult::success(vec![Content::text(json_string(
            &payload,
        )?)]))
    }

    /// Archive a memory by id.
    #[tool(description = "Archive a memory by id. Excluded from retrieval; recoverable.")]
    async fn forget(
        &self,
        Parameters(args): Parameters<ForgetArgs>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.inner.db.lock().await;
        let memory = archive(&db, &args.id, now_epoch_seconds()).map_err(core_to_mcp)?;
        drop(db);
        let payload = ForgetResponse {
            id: memory.id,
            status: memory.status.as_db_str().to_string(),
            summary: "Memryzed: archived 1 fact".into(),
        };
        Ok(CallToolResult::success(vec![Content::text(json_string(
            &payload,
        )?)]))
    }

    /// List memories without retrieval ranking.
    #[tool(
        description = "List memories without retrieval ranking. Used for transparency and debugging."
    )]
    async fn list_memories(
        &self,
        Parameters(args): Parameters<ListMemoriesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let scope = parse_scope_opt(args.scope.as_deref())?;
        let filter = ListFilter {
            scope,
            scope_id: None,
            statuses: vec![Status::Approved, Status::Pinned],
            limit: args.limit,
        };
        let db = self.inner.db.lock().await;
        let memories = list_memories_core(&db, &filter).map_err(core_to_mcp)?;
        drop(db);

        let payload = ListMemoriesResponse {
            memories: memories.iter().map(memory_summary).collect(),
        };
        Ok(CallToolResult::success(vec![Content::text(json_string(
            &payload,
        )?)]))
    }

    /// Save the current task's session state for this project.
    #[tool(
        description = "Save the current task's working state for this project. Creates or updates the active session."
    )]
    async fn checkpoint(
        &self,
        Parameters(args): Parameters<CheckpointArgs>,
    ) -> Result<CallToolResult, McpError> {
        let project_id = self.require_project().await?;
        let state = args.state.unwrap_or(serde_json::Value::Null);
        let db = self.inner.db.lock().await;
        let session =
            sessions::checkpoint(&db, &project_id, args.title, state, now_epoch_seconds())
                .map_err(core_to_mcp)?;
        drop(db);
        let payload = SessionResponse {
            session_id: session.id,
            status: session.status.as_db_str().to_string(),
            summary: "Memryzed: checkpointed session".into(),
        };
        Ok(CallToolResult::success(vec![Content::text(json_string(
            &payload,
        )?)]))
    }

    /// Load a session's state for this project.
    #[tool(
        description = "Load a session. Without an id, resumes the most recent session for this project."
    )]
    async fn resume(
        &self,
        Parameters(args): Parameters<ResumeArgs>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.inner.db.lock().await;
        let session = match args.session_id {
            Some(id) => sessions::get_by_id(&db, &id).map_err(core_to_mcp)?,
            None => {
                let project_id = self.require_project().await?;
                sessions::resume_latest(&db, &project_id).map_err(core_to_mcp)?
            }
        };
        drop(db);

        match session {
            Some(s) => {
                let payload = ResumeResponse {
                    session: Some(SessionDetail {
                        id: s.id,
                        title: s.title,
                        project_id: s.project_id,
                        status: s.status.as_db_str().to_string(),
                        state: s.state,
                        created_at: s.created_at,
                        updated_at: s.updated_at,
                    }),
                    summary: "Memryzed: resumed session".into(),
                };
                Ok(CallToolResult::success(vec![Content::text(json_string(
                    &payload,
                )?)]))
            }
            None => {
                let payload = ResumeResponse {
                    session: None,
                    summary: "Memryzed: no prior sessions in this project".into(),
                };
                Ok(CallToolResult::success(vec![Content::text(json_string(
                    &payload,
                )?)]))
            }
        }
    }

    /// List sessions for this project.
    #[tool(description = "List sessions for the current project, most recent first.")]
    async fn list_sessions(
        &self,
        Parameters(args): Parameters<ListSessionsArgs>,
    ) -> Result<CallToolResult, McpError> {
        let project_id = match args.project_id {
            Some(p) => p,
            None => self.require_project().await?,
        };
        let db = self.inner.db.lock().await;
        let list = sessions::list(&db, &project_id, Some(args.limit.unwrap_or(10).max(1)))
            .map_err(core_to_mcp)?;
        drop(db);
        let payload = ListSessionsResponse {
            sessions: list
                .into_iter()
                .map(|s| SessionSummary {
                    id: s.id,
                    title: s.title,
                    status: s.status.as_db_str().to_string(),
                    updated_at: s.updated_at,
                })
                .collect(),
        };
        Ok(CallToolResult::success(vec![Content::text(json_string(
            &payload,
        )?)]))
    }

    /// Mark a session completed.
    #[tool(description = "Mark a session completed and stop resuming it.")]
    async fn end_session(
        &self,
        Parameters(args): Parameters<EndSessionArgs>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.inner.db.lock().await;
        let session =
            sessions::end(&db, &args.session_id, now_epoch_seconds()).map_err(core_to_mcp)?;
        drop(db);
        let payload = SessionResponse {
            session_id: session.id,
            status: session.status.as_db_str().to_string(),
            summary: "Memryzed: session ended".into(),
        };
        Ok(CallToolResult::success(vec![Content::text(json_string(
            &payload,
        )?)]))
    }

    /// Extract candidate memories from a message.
    #[tool(
        description = "Scan a user message for facts and preferences. High-confidence candidates are stored; the rest are queued for review."
    )]
    async fn extract_from(
        &self,
        Parameters(args): Parameters<ExtractArgs>,
    ) -> Result<CallToolResult, McpError> {
        // Rule-based candidates always run. When the caller opts into
        // Ollama and it is reachable, its candidates are merged in;
        // if it is unreachable we silently use rule-based only.
        let mut candidates = extractor::extract(&args.message);
        if args.use_ollama.unwrap_or(false) {
            let cfg = memryzed_core::extractor::ollama::OllamaConfig::default();
            if let Some(llm) = memryzed_core::extractor::ollama::extract(&cfg, &args.message) {
                for c in llm {
                    if !candidates
                        .iter()
                        .any(|e| e.content.eq_ignore_ascii_case(&c.content))
                    {
                        candidates.push(c);
                    }
                }
            }
        }
        let threshold = args.auto_approve_threshold.unwrap_or(0.85);
        let project_id = self.inner.project_id.lock().await.clone();

        let now = now_epoch_seconds();
        let mut db = self.inner.db.lock().await;
        let mut stored = Vec::new();
        for cand in candidates {
            // Project-scoped candidates need a project; fall back to
            // global when the server has no project context.
            let (scope, scope_id) = match cand.scope {
                Scope::Project => match &project_id {
                    Some(pid) => (Scope::Project, Some(pid.clone())),
                    None => (Scope::Global, None),
                },
                other => (other, None),
            };
            let mut new = NewMemory::new(scope, cand.content.clone());
            new.scope_id = scope_id;
            new.kind = cand.kind;
            new.confidence = Some(cand.confidence);
            new.source_client = args.client.clone();

            if cand.confidence >= threshold {
                let m = insert_with_embedder(&mut db, new, self.inner.embedder.as_ref(), now)
                    .map_err(core_to_mcp)?;
                stored.push(ExtractHit {
                    id: m.id,
                    content: m.content,
                    status: m.status.as_db_str().to_string(),
                    confidence: cand.confidence,
                });
            } else {
                let m =
                    memryzed_core::memory::insert_pending(&db, new, now).map_err(core_to_mcp)?;
                stored.push(ExtractHit {
                    id: m.id,
                    content: m.content,
                    status: m.status.as_db_str().to_string(),
                    confidence: cand.confidence,
                });
            }
        }
        drop(db);

        let approved = stored.iter().filter(|h| h.status != "pending").count();
        let pending = stored.len() - approved;
        let payload = ExtractResponse {
            summary: format!("Memryzed: {approved} stored, {pending} queued for review"),
            candidates: stored,
        };
        Ok(CallToolResult::success(vec![Content::text(json_string(
            &payload,
        )?)]))
    }
}

#[tool_handler]
impl ServerHandler for MemryzedServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "Memryzed gives you persistent memory of the user's past conversations and \
facts across every agent they use, including other tools. \
\
IMPORTANT BEHAVIOR: Call the `recall` tool proactively, on your own initiative, BEFORE answering \
whenever a request might depend on prior context, references earlier work or decisions, mentions \
'what we discussed' or past sessions, or whenever loading relevant background would improve your \
answer. Also call `recall` at the start of a new task. Do not wait for the user to ask you to \
check memory; treat recall as a normal first step, like reading a file. When the user states a \
durable preference, decision, or fact, call `remember` to persist it. \
\
Tools: recall (search past conversations and facts), remember (store a fact), forget (archive), \
list_memories (transparency). Session tools: checkpoint, resume, list_sessions, end_session, \
scoped to the working directory's project."
                    .to_string(),
            )
    }
}

// ------------------------ tool argument types ------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RecallArgs {
    /// Natural-language query.
    query: String,
    /// Scope filter: global, project, session, or all (default).
    #[serde(default)]
    scope: Option<String>,
    /// Maximum number of results to return.
    #[serde(default)]
    limit: Option<u32>,
    /// Ordering: "relevant" (default, by similarity) or "recent"
    /// (latest conversations by time, for "what did we last discuss").
    #[serde(default)]
    order: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RememberArgs {
    /// The fact to remember.
    content: String,
    /// Scope: global, project, or session.
    scope: String,
    /// Project or session id (required for non-global scopes).
    #[serde(default)]
    scope_id: Option<String>,
    /// Kind: preference, fact, decision, or todo. Defaults to fact.
    #[serde(default)]
    kind: Option<String>,
    /// Expire after this many days.
    #[serde(default)]
    ttl_days: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ForgetArgs {
    /// Memory id (mem_xxxxxxxxxxxx).
    id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListMemoriesArgs {
    /// Scope filter: global, project, session, or all (default).
    #[serde(default)]
    scope: Option<String>,
    /// Maximum number of results.
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CheckpointArgs {
    /// Human-readable title for the session.
    #[serde(default)]
    title: Option<String>,
    /// Opaque working-state object (open files, recent turns, etc.).
    #[serde(default)]
    state: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ResumeArgs {
    /// Session id. If absent, resumes the most recent session for
    /// the current project.
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListSessionsArgs {
    /// Project id. Defaults to the current project.
    #[serde(default)]
    project_id: Option<String>,
    /// Maximum number of results.
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct EndSessionArgs {
    /// Session id to mark completed.
    session_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExtractArgs {
    /// The user message to scan for facts and preferences.
    message: String,
    /// Confidence threshold for auto-approval. Default 0.85.
    #[serde(default)]
    auto_approve_threshold: Option<f64>,
    /// Originating client identifier, recorded on stored memories.
    #[serde(default)]
    client: Option<String>,
    /// Also consult a local Ollama instance (off by default).
    #[serde(default)]
    use_ollama: Option<bool>,
}

// ------------------------ response types ------------------------

#[derive(Debug, Serialize)]
struct RecallResponse {
    results: Vec<RecallHit>,
    episodes: Vec<EpisodeRecallHit>,
    summary: String,
}

#[derive(Debug, Serialize)]
struct RecallHit {
    id: String,
    content: String,
    scope_kind: String,
    scope_id: Option<String>,
    kind: String,
    confidence: Option<f64>,
    pinned: bool,
    score: f32,
    created_at: i64,
}

/// A recalled conversation turn from a prior session, possibly with a
/// different agent. This is the cross-agent continuity payload.
#[derive(Debug, Serialize)]
struct EpisodeRecallHit {
    id: String,
    role: String,
    content: String,
    source_agent: Option<String>,
    score: f32,
    created_at: i64,
    /// The matched turn rendered with its neighbouring turns from the
    /// same conversation, in order, so the excerpt reads coherently.
    excerpt: String,
}

#[derive(Debug, Serialize)]
struct RememberResponse {
    id: String,
    status: String,
    summary: String,
}

#[derive(Debug, Serialize)]
struct ForgetResponse {
    id: String,
    status: String,
    summary: String,
}

#[derive(Debug, Serialize)]
struct ListMemoriesResponse {
    memories: Vec<MemorySummary>,
}

#[derive(Debug, Serialize)]
struct MemorySummary {
    id: String,
    content: String,
    scope_kind: String,
    scope_id: Option<String>,
    kind: String,
    pinned: bool,
    created_at: i64,
}

#[derive(Debug, Serialize)]
struct SessionResponse {
    session_id: String,
    status: String,
    summary: String,
}

#[derive(Debug, Serialize)]
struct ResumeResponse {
    session: Option<SessionDetail>,
    summary: String,
}

#[derive(Debug, Serialize)]
struct SessionDetail {
    id: String,
    title: Option<String>,
    project_id: String,
    status: String,
    state: serde_json::Value,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
struct ListSessionsResponse {
    sessions: Vec<SessionSummary>,
}

#[derive(Debug, Serialize)]
struct SessionSummary {
    id: String,
    title: Option<String>,
    status: String,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
struct ExtractResponse {
    candidates: Vec<ExtractHit>,
    summary: String,
}

#[derive(Debug, Serialize)]
struct ExtractHit {
    id: String,
    content: String,
    status: String,
    confidence: f64,
}

fn memory_summary(m: &Memory) -> MemorySummary {
    MemorySummary {
        id: m.id.clone(),
        content: m.content.clone(),
        scope_kind: m.scope.as_db_str().to_string(),
        scope_id: m.scope_id.clone(),
        kind: m.kind.as_db_str().to_string(),
        pinned: m.pinned,
        created_at: m.created_at,
    }
}

// ------------------------ helpers ------------------------

fn parse_scope(s: &str) -> Result<Scope, McpError> {
    s.parse::<Scope>().map_err(core_to_mcp)
}

fn parse_scope_opt(s: Option<&str>) -> Result<Option<Scope>, McpError> {
    match s {
        None => Ok(None),
        Some("all") => Ok(None),
        Some(other) => Ok(Some(parse_scope(other)?)),
    }
}

fn json_string<T: serde::Serialize>(value: &T) -> Result<String, McpError> {
    serde_json::to_string(value)
        .map_err(|e| McpError::internal_error(format!("failed to serialize response: {e}"), None))
}

/// Render a recalled hit's context window as a readable transcript
/// excerpt: each turn on its own line prefixed by role, with the
/// matched turn marked so a weaker agent knows which line matched.
fn render_excerpt(
    context: &[memryzed_core::episodes::Episode],
    matched: &memryzed_core::episodes::Episode,
) -> String {
    if context.is_empty() {
        return matched.content.clone();
    }
    context
        .iter()
        .map(|t| {
            let marker = if t.id == matched.id { "> " } else { "  " };
            format!("{marker}{}: {}", t.role, t.content.trim())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One-line, self-describing summary so even a weak agent knows what
/// it received and how to use it.
fn render_summary(
    facts: usize,
    episodes: &[memryzed_core::episodes::EpisodeHit],
    recent_mode: bool,
) -> String {
    let agents: std::collections::BTreeSet<&str> = episodes
        .iter()
        .filter_map(|e| e.episode.source_agent.as_deref())
        .collect();
    let from = if agents.is_empty() {
        String::new()
    } else {
        format!(
            " from {}",
            agents.into_iter().collect::<Vec<_>>().join(", ")
        )
    };
    let mode = if recent_mode {
        "most recent"
    } else {
        "most relevant"
    };
    format!(
        "Memryzed: {} fact{} and {} {} conversation excerpt{}{}. Each excerpt includes the \
surrounding turns; the matched line is marked with '>'.",
        facts,
        if facts == 1 { "" } else { "s" },
        episodes.len(),
        mode,
        if episodes.len() == 1 { "" } else { "s" },
        from,
    )
}

fn core_to_mcp(err: memryzed_core::Error) -> McpError {
    use memryzed_core::Error::*;
    match err {
        NotFound { kind, id } => McpError::invalid_params(format!("{kind} {id} not found"), None),
        Validation(msg) => McpError::invalid_params(msg, None),
        Storage(e) => McpError::internal_error(format!("storage error: {e}"), None),
        Io(e) => McpError::internal_error(format!("I/O error: {e}"), None),
        Migration(msg) => McpError::internal_error(format!("migration error: {msg}"), None),
        Config(msg) => McpError::internal_error(format!("configuration error: {msg}"), None),
    }
}

// ------------------------ public re-exports ------------------------

pub use rmcp::transport::stdio;

/// Configuration knob: the data directory the server should open.
///
/// Wraps a `PathBuf` so callers can hand it through without leaking
/// the wider `DataDir` type to the CLI.
pub struct ServerConfig {
    /// Where the database, models, and configuration live.
    pub data_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use memryzed_core::NoopEmbedder;

    fn fresh_server() -> MemryzedServer {
        let db = Database::open_in_memory().unwrap();
        MemryzedServer::from_parts(db, Arc::new(NoopEmbedder))
    }

    fn parse_text(result: &CallToolResult) -> serde_json::Value {
        let first = result.content.first().expect("at least one content item");
        let text = match &first.raw {
            RawContent::Text(t) => t.text.clone(),
            other => panic!("expected text content, got {other:?}"),
        };
        serde_json::from_str(&text).expect("response is JSON")
    }

    #[tokio::test]
    async fn remember_returns_an_id_and_approved_status() {
        let server = fresh_server();
        let result = server
            .remember(Parameters(RememberArgs {
                content: "I prefer pnpm".into(),
                scope: "global".into(),
                scope_id: None,
                kind: None,
                ttl_days: None,
            }))
            .await
            .unwrap();
        let v = parse_text(&result);
        assert!(v["id"].as_str().unwrap().starts_with("mem_"));
        assert_eq!(v["status"], "approved");
        assert!(v["summary"].as_str().unwrap().contains("global"));
    }

    #[tokio::test]
    async fn list_memories_includes_just_remembered_item() {
        let server = fresh_server();
        server
            .remember(Parameters(RememberArgs {
                content: "always run tests".into(),
                scope: "global".into(),
                scope_id: None,
                kind: None,
                ttl_days: None,
            }))
            .await
            .unwrap();
        let result = server
            .list_memories(Parameters(ListMemoriesArgs {
                scope: None,
                limit: None,
            }))
            .await
            .unwrap();
        let v = parse_text(&result);
        let arr = v["memories"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["content"], "always run tests");
    }

    #[tokio::test]
    async fn forget_archives_a_memory() {
        let server = fresh_server();
        let r = server
            .remember(Parameters(RememberArgs {
                content: "to forget".into(),
                scope: "global".into(),
                scope_id: None,
                kind: None,
                ttl_days: None,
            }))
            .await
            .unwrap();
        let id = parse_text(&r)["id"].as_str().unwrap().to_string();

        let result = server
            .forget(Parameters(ForgetArgs { id: id.clone() }))
            .await
            .unwrap();
        let v = parse_text(&result);
        assert_eq!(v["status"], "archived");
        assert_eq!(v["id"], id);
    }

    #[tokio::test]
    async fn recall_rejects_empty_query() {
        let server = fresh_server();
        let err = server
            .recall(Parameters(RecallArgs {
                query: "  ".into(),
                scope: None,
                limit: None,
                order: None,
            }))
            .await
            .unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.to_lowercase().contains("empty"));
    }

    #[tokio::test]
    async fn forget_unknown_id_returns_invalid_params() {
        let server = fresh_server();
        let err = server
            .forget(Parameters(ForgetArgs {
                id: "mem_doesnotexist".into(),
            }))
            .await
            .unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn invalid_scope_returns_invalid_params() {
        let server = fresh_server();
        let err = server
            .remember(Parameters(RememberArgs {
                content: "x".into(),
                scope: "wing".into(),
                scope_id: None,
                kind: None,
                ttl_days: None,
            }))
            .await
            .unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.to_lowercase().contains("scope"));
    }

    #[tokio::test]
    async fn checkpoint_then_resume_round_trips() {
        let server = fresh_server();
        // Seed a project row so the session FK is satisfiable, then
        // point the server at it.
        {
            let db = server.inner.db.lock().await;
            let tmp = tempfile::tempdir().unwrap();
            let p = memryzed_core::projects::ensure_for_cwd(&db, tmp.path(), 100).unwrap();
            drop(db);
            server.set_project_for_test(p.id).await;
        }

        let r = server
            .checkpoint(Parameters(CheckpointArgs {
                title: Some("Refactor".into()),
                state: Some(serde_json::json!({"open_files": ["a.rs"]})),
            }))
            .await
            .unwrap();
        let cv = parse_text(&r);
        assert!(cv["session_id"].as_str().unwrap().starts_with("sess_"));
        assert_eq!(cv["status"], "active");

        let r = server
            .resume(Parameters(ResumeArgs { session_id: None }))
            .await
            .unwrap();
        let rv = parse_text(&r);
        assert_eq!(rv["session"]["title"], "Refactor");
        assert_eq!(rv["session"]["state"]["open_files"][0], "a.rs");
    }

    #[tokio::test]
    async fn resume_with_no_sessions_returns_null() {
        let server = fresh_server();
        {
            let db = server.inner.db.lock().await;
            let tmp = tempfile::tempdir().unwrap();
            let p = memryzed_core::projects::ensure_for_cwd(&db, tmp.path(), 100).unwrap();
            drop(db);
            server.set_project_for_test(p.id).await;
        }
        let r = server
            .resume(Parameters(ResumeArgs { session_id: None }))
            .await
            .unwrap();
        let rv = parse_text(&r);
        assert!(rv["session"].is_null());
        assert!(rv["summary"].as_str().unwrap().contains("no prior"));
    }

    #[tokio::test]
    async fn end_session_marks_completed() {
        let server = fresh_server();
        let pid = {
            let db = server.inner.db.lock().await;
            let tmp = tempfile::tempdir().unwrap();
            let p = memryzed_core::projects::ensure_for_cwd(&db, tmp.path(), 100).unwrap();
            p.id
        };
        server.set_project_for_test(pid).await;

        let r = server
            .checkpoint(Parameters(CheckpointArgs {
                title: None,
                state: None,
            }))
            .await
            .unwrap();
        let sid = parse_text(&r)["session_id"].as_str().unwrap().to_string();

        let r = server
            .end_session(Parameters(EndSessionArgs { session_id: sid }))
            .await
            .unwrap();
        assert_eq!(parse_text(&r)["status"], "completed");
    }

    #[tokio::test]
    async fn session_tools_error_without_project() {
        let server = fresh_server();
        // No project set on the test server.
        let err = server
            .checkpoint(Parameters(CheckpointArgs {
                title: None,
                state: None,
            }))
            .await
            .unwrap_err();
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("project"));
    }

    #[tokio::test]
    async fn extract_from_auto_approves_high_confidence() {
        let server = fresh_server();
        let r = server
            .extract_from(Parameters(ExtractArgs {
                message: "remember that I deploy with cargo-dist".into(),
                auto_approve_threshold: None,
                client: Some("kiro".into()),
                use_ollama: None,
            }))
            .await
            .unwrap();
        let v = parse_text(&r);
        let cands = v["candidates"].as_array().unwrap();
        assert_eq!(cands.len(), 1);
        // "remember that ..." is confidence 1.0 -> approved + stored.
        assert_eq!(cands[0]["status"], "approved");
        assert!(v["summary"].as_str().unwrap().contains("1 stored"));
    }

    #[tokio::test]
    async fn extract_from_queues_below_threshold() {
        let server = fresh_server();
        // Force everything to the pending queue with a threshold of 1.01.
        let r = server
            .extract_from(Parameters(ExtractArgs {
                message: "I prefer pnpm over npm".into(),
                auto_approve_threshold: Some(1.01),
                client: None,
                use_ollama: None,
            }))
            .await
            .unwrap();
        let v = parse_text(&r);
        let cands = v["candidates"].as_array().unwrap();
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0]["status"], "pending");
        assert!(v["summary"].as_str().unwrap().contains("1 queued"));
    }

    #[tokio::test]
    async fn extract_from_no_match_stores_nothing() {
        let server = fresh_server();
        let r = server
            .extract_from(Parameters(ExtractArgs {
                message: "what time is it".into(),
                auto_approve_threshold: None,
                client: None,
                use_ollama: None,
            }))
            .await
            .unwrap();
        let v = parse_text(&r);
        assert_eq!(v["candidates"].as_array().unwrap().len(), 0);
    }
}
