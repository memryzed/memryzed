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
//! v0.1.0-alpha.5 wires four of the eight tools from
//! `docs/mcp-reference.md`:
//!
//! - `recall` — hybrid retrieval over stored memories.
//! - `remember` — write a new memory.
//! - `forget` — archive a memory by id.
//! - `list_memories` — list memories without retrieval ranking.
//!
//! Sessions, project auto-detection, and the remaining tools land in
//! later releases.

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
use memryzed_core::Database;

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
        Ok(Self {
            inner: Arc::new(Inner {
                db: Mutex::new(db),
                embedder,
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
            }),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl MemryzedServer {
    /// Find memories relevant to a query.
    #[tool(
        description = "Find memories relevant to a query. Returns top-K hybrid-ranked memories."
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
            summary: format!(
                "Memryzed: {} fact{} found",
                results.len(),
                if results.len() == 1 { "" } else { "s" }
            ),
        };
        Ok(CallToolResult::success(vec![Content::text(json_string(
            &payload,
        )?)]))
    }

    /// Store a new memory.
    #[tool(description = "Store a new memory. Auto-approved (user is in the loop).")]
    async fn remember(
        &self,
        Parameters(args): Parameters<RememberArgs>,
    ) -> Result<CallToolResult, McpError> {
        let scope = parse_scope(&args.scope)?;
        if scope == Scope::Session {
            return Err(McpError::invalid_params(
                "session-scoped memories require an active session; sessions land in v0.2.0",
                None,
            ));
        }
        // v0.1.0-alpha.5 supports global memories via remember; project
        // scope requires a project id which the caller must supply later
        // as the project-tools layer lands.
        if scope == Scope::Project && args.scope_id.is_none() {
            return Err(McpError::invalid_params(
                "project-scoped remember requires scope_id; project auto-detect lands in beta.1",
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
}

#[tool_handler]
impl ServerHandler for MemryzedServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "Memryzed: persistent memory for AI coding agents. \
Tools: recall (hybrid search), remember (store), forget (archive), \
list_memories (transparency). v0.1.0-alpha.5; sessions and project \
auto-detection land in later releases."
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

// ------------------------ response types ------------------------

#[derive(Debug, Serialize)]
struct RecallResponse {
    results: Vec<RecallHit>,
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
}
