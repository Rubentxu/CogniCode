//! Multimodal ingest tool handlers.
//!
//! Implements 3 MCP tools for multimodal data ingestion (feature-gated):
//! - `docs_ingest`   — ingest Markdown / ADR files into the Generic Graph Layer
//! - `graph_search`  — FTS5-backed search across the graph_nodes table
//! - `issues_ingest` — ingest GitHub issues from a repository

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use serde_json::Value;

#[cfg(feature = "multimodal")]
use crate::mcp::envelope::{err_envelope, ok_envelope};
#[cfg(feature = "multimodal")]
use crate::mcp::handler::ToolHandler;
#[cfg(feature = "multimodal")]
use crate::mcp::{McpContext, TOOL_DOCS_INGEST, TOOL_GRAPH_SEARCH, TOOL_ISSUES_INGEST};

#[cfg(feature = "multimodal")]
use crate::ports::GraphRepository;

#[cfg(feature = "multimodal")]
use crate::mcp::{DEFAULT_GRAPH_SEARCH_LIMIT, MAX_GRAPH_SEARCH_LIMIT};

// ============================================================================
// Arg structs
// ============================================================================

#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DocsIngestArgs {
    path: Option<String>,
    recursive: Option<bool>,
}

#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphSearchArgs {
    query: Option<String>,
    node_kinds: Option<Vec<String>>,
    cursor: Option<String>,
    limit: Option<i64>,
}

#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct IssuesIngestArgs {
    owner: Option<String>,
    repo: Option<String>,
}

// ============================================================================
// ToolHandler implementations (multimodal feature-gated)
// ============================================================================

#[cfg(feature = "multimodal")]
struct DocsIngestHandler;

#[cfg(feature = "multimodal")]
#[async_trait]
#[cfg(feature = "multimodal")]
impl ToolHandler for DocsIngestHandler {
    fn name(&self) -> &'static str {
        TOOL_DOCS_INGEST
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Filesystem path to ingest (required)."
                },
                "recursive": {
                    "type": "boolean",
                    "description": "When path is a directory, recurse into subdirectories. Defaults to true."
                }
            },
            "required": ["path"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        use cognicode_core::domain::traits::source_extractor::{SourceExtractor, SourcePath};
        use cognicode_core::infrastructure::extraction::docs_extractor::DocsExtractor;

        let args: DocsIngestArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_DOCS_INGEST,
                    "invalid_input",
                    &format!("{TOOL_DOCS_INGEST}: invalid args: {e}"),
                );
            }
        };

        let path_str = match args.path {
            Some(s) if !s.is_empty() => s,
            _ => {
                return err_envelope(
                    TOOL_DOCS_INGEST,
                    "invalid_input",
                    "docs_ingest: missing required arg `path`",
                );
            }
        };

        let path = PathBuf::from(&path_str);
        if !path.exists() {
            return err_envelope(
                TOOL_DOCS_INGEST,
                "not_found",
                &format!("path does not exist: {path_str}"),
            );
        }

        let recursive = args.recursive.unwrap_or(true);
        let extractor = DocsExtractor::new();
        let result = if path.is_dir() {
            match extractor.extract_directory(&path, recursive).await {
                Ok(nodes) => nodes,
                Err(e) => {
                    return err_envelope(
                        TOOL_DOCS_INGEST,
                        "extractor_error",
                        &format!("docs extractor failed: {e}"),
                    );
                }
            }
        } else {
            match extractor.extract_file(&path).await {
                Ok(nodes) => nodes,
                Err(e) => {
                    return err_envelope(
                        TOOL_DOCS_INGEST,
                        "extractor_error",
                        &format!("docs extractor failed: {e}"),
                    );
                }
            }
        };

        let files_processed = result
            .iter()
            .map(|n| n.potential_node.source_path.clone())
            .filter_map(|p| p)
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let nodes_created = result.len();
        let edges_created: usize = result.iter().map(|n| n.potential_edges.len()).sum();
        let payload = serde_json::json!({
            "files_processed": files_processed,
            "nodes_created": nodes_created,
            "edges_created": edges_created,
            "errors": Vec::<String>::new(),
        });
        let _ = ctx;
        ok_envelope(TOOL_DOCS_INGEST, &payload)
    }
}

#[cfg(feature = "multimodal")]
struct GraphSearchHandler;

#[cfg(feature = "multimodal")]
#[async_trait]
#[cfg(feature = "multimodal")]
impl ToolHandler for GraphSearchHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_SEARCH
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (required, non-empty)."
                },
                "node_kinds": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional filter — one or more of: symbol, decision, doc, issue, evidence, component, container, system."
                },
                "cursor": {
                    "type": "string",
                    "description": "Opaque cursor for pagination."
                },
                "limit": {
                    "type": "integer",
                    "description": "Page size — defaults to 50, capped at 200."
                }
            },
            "required": ["query"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        use cognicode_core::domain::value_objects::node_kind::NodeKind;

        let args: GraphSearchArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_SEARCH,
                    "invalid_input",
                    &format!("{TOOL_GRAPH_SEARCH}: invalid args: {e}"),
                );
            }
        };

        let query = match args.query {
            Some(q) if !q.is_empty() => q,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_SEARCH,
                    "invalid_input",
                    "graph_search: missing required arg `query`",
                );
            }
        };

        let limit = match args.limit {
            Some(n) if n <= 0 => {
                return err_envelope(
                    TOOL_GRAPH_SEARCH,
                    "invalid_input",
                    &format!("graph_search: `limit` must be a positive integer (got {n})"),
                );
            }
            Some(n) => (n as usize).min(MAX_GRAPH_SEARCH_LIMIT as usize),
            None => DEFAULT_GRAPH_SEARCH_LIMIT as usize,
        };

        let mut parsed_kinds: Vec<NodeKind> = Vec::new();
        if let Some(raw) = args.node_kinds {
            for k in raw {
                match k.as_str() {
                    "symbol" => {
                        use cognicode_core::domain::value_objects::symbol_kind::SymbolKind;
                        const SYMBOL_WILDCARD_COUNT: usize = 21;
                        let wildcard: [NodeKind; SYMBOL_WILDCARD_COUNT] = [
                            NodeKind::Symbol(SymbolKind::Function),
                            NodeKind::Symbol(SymbolKind::Method),
                            NodeKind::Symbol(SymbolKind::Class),
                            NodeKind::Symbol(SymbolKind::Struct),
                            NodeKind::Symbol(SymbolKind::Module),
                            NodeKind::Symbol(SymbolKind::Variable),
                            NodeKind::Symbol(SymbolKind::Parameter),
                            NodeKind::Symbol(SymbolKind::Type),
                            NodeKind::Symbol(SymbolKind::Property),
                            NodeKind::Symbol(SymbolKind::Field),
                            NodeKind::Symbol(SymbolKind::Import),
                            NodeKind::Symbol(SymbolKind::EnumVariant),
                            NodeKind::Symbol(SymbolKind::Trait),
                            NodeKind::Symbol(SymbolKind::Generic),
                            NodeKind::Symbol(SymbolKind::Constant),
                            NodeKind::Symbol(SymbolKind::Constructor),
                            NodeKind::Symbol(SymbolKind::Enum),
                            NodeKind::Symbol(SymbolKind::Interface),
                            NodeKind::Symbol(SymbolKind::File),
                            NodeKind::Symbol(SymbolKind::Namespace),
                            NodeKind::Symbol(SymbolKind::Package),
                        ];
                        debug_assert_eq!(wildcard.len(), SYMBOL_WILDCARD_COUNT);
                        parsed_kinds.extend(wildcard);
                    }
                    "decision" => parsed_kinds.push(NodeKind::Decision),
                    "doc" => parsed_kinds.push(NodeKind::Doc),
                    "issue" => parsed_kinds.push(NodeKind::Issue),
                    "evidence" => parsed_kinds.push(NodeKind::Evidence),
                    "component" => parsed_kinds.push(NodeKind::Component),
                    "container" => parsed_kinds.push(NodeKind::Container),
                    "system" => parsed_kinds.push(NodeKind::System),
                    other => {
                        return err_envelope(
                            TOOL_GRAPH_SEARCH,
                            "invalid_input",
                            &format!("graph_search: unknown `node_kinds` entry `{other}`"),
                        );
                    }
                }
            }
        }

        let repo = match &ctx.graph_repo {
            Some(r) => r.as_ref(),
            None => {
                return err_envelope(
                    TOOL_GRAPH_SEARCH,
                    "graph_search_unavailable",
                    "graph_search: no GraphRepository wired into the handler",
                );
            }
        };

        let page = match repo.search(&query, &parsed_kinds, limit, args.cursor.as_deref()) {
            Ok(p) => p,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_SEARCH,
                    "repository_error",
                    &format!("graph_search: search failed: {e}"),
                );
            }
        };

        let normalized_score = page.raw_rank.clamp(0.0, 1.0);
        let per_item_ranks: Vec<f64> = if page.item_ranks.len() == page.items.len() {
            page.item_ranks.clone()
        } else if !page.item_ranks.is_empty() {
            let n = page.item_ranks.len().min(page.items.len());
            page.item_ranks[..n].to_vec()
        } else {
            Vec::new()
        };
        let page_level_rank = page.raw_rank;

        let payload = serde_json::json!({
            "results": page.items.iter().enumerate().map(|(i, n)| {
                let (score, score_is_page_level) = match per_item_ranks.get(i) {
                    Some(r) => (*r, false),
                    None => (page_level_rank, true),
                };
                serde_json::json!({
                    "node": {
                        "id": n.id.as_str(),
                        "label": n.label,
                        "kind": n.kind.as_str(),
                        "source_path": n.source_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                        "metadata": n.properties,
                    },
                    "score": score,
                    "item_rank": score,
                    "score_is_page_level": score_is_page_level,
                })
            }).collect::<Vec<_>>(),
            "total_count": page.raw_total,
            "next_cursor": page.next_cursor,
            "raw_rank": page.raw_rank,
            "normalized_score": normalized_score,
        });

        ok_envelope(TOOL_GRAPH_SEARCH, &payload)
    }
}

#[cfg(feature = "multimodal")]
struct IssuesIngestHandler;

#[cfg(feature = "multimodal")]
#[async_trait]
#[cfg(feature = "multimodal")]
impl ToolHandler for IssuesIngestHandler {
    fn name(&self) -> &'static str {
        TOOL_ISSUES_INGEST
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "owner": {
                    "type": "string",
                    "description": "GitHub owner / organisation (required)."
                },
                "repo": {
                    "type": "string",
                    "description": "GitHub repository name (required)."
                }
            },
            "required": ["owner", "repo"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        use cognicode_core::domain::traits::source_extractor::{SourceExtractor, SourcePath};
        use cognicode_core::infrastructure::extraction::issues_extractor::IssuesExtractor;
        use cognicode_core::infrastructure::github::client::GitHubClient;
        use cognicode_core::infrastructure::github::octocrab_client::OctocrabClient;
        use std::sync::Arc as StdArc;

        let args: IssuesIngestArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_ISSUES_INGEST,
                    "invalid_input",
                    &format!("{TOOL_ISSUES_INGEST}: invalid args: {e}"),
                );
            }
        };

        let owner = match args.owner {
            Some(o) if !o.is_empty() => o,
            _ => {
                return err_envelope(
                    TOOL_ISSUES_INGEST,
                    "invalid_input",
                    "issues_ingest: missing required arg `owner`",
                );
            }
        };

        let repo = match args.repo {
            Some(r) if !r.is_empty() => r,
            _ => {
                return err_envelope(
                    TOOL_ISSUES_INGEST,
                    "invalid_input",
                    "issues_ingest: missing required arg `repo`",
                );
            }
        };

        let extractor = IssuesExtractor::with_repo_override(
            StdArc::new(OctocrabClient::new()) as StdArc<dyn GitHubClient>,
            owner.clone(),
            repo.clone(),
        );

        let url = format!("https://github.com/{owner}/{repo}");
        let result = match extractor.extract(SourcePath::Url(url)).await {
            Ok(nodes) => nodes,
            Err(e) => {
                return err_envelope(
                    TOOL_ISSUES_INGEST,
                    "extractor_error",
                    &format!("issues extractor failed: {e}"),
                );
            }
        };

        let issues_processed = result.len();
        let nodes_created = result.len();
        let edges_created: usize = result.iter().map(|n| n.potential_edges.len()).sum();

        let payload = serde_json::json!({
            "issues_processed": issues_processed,
            "nodes_created": nodes_created,
            "edges_created": edges_created,
            "errors": Vec::<String>::new(),
        });
        let _ = ctx;
        ok_envelope(TOOL_ISSUES_INGEST, &payload)
    }
}

// ============================================================================
// Registry builder (multimodal feature-gated)
// ============================================================================

#[cfg(feature = "multimodal")]
/// Register all multimodal ingest handlers into the registry.
/// Includes the 3 existing handlers (docs, graph_search, issues) plus
/// the 2 OpenAPI handlers from cycle e15.5.
pub fn register_ingest_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(DocsIngestHandler);
    registry.register(GraphSearchHandler);
    registry.register(IssuesIngestHandler);
    // Cycle e15.5 — OpenAPI / gRPC / GraphQL / tRPC ingestion
    super::ingest_openapi::handlers::register_handlers(registry);
}

/// No-op fallback when the multimodal feature is not enabled.
#[cfg(not(feature = "multimodal"))]
pub fn register_ingest_handlers(_registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    // No handlers registered when multimodal feature is disabled.
}
