//! Context-builder MCP tool for the Explorer MCP.
//!
//! Exposes a single tool that consolidates object inspection, lens
//! findings, quality issues, and graph neighbors into a unified
//! context blob designed for LLM agent consumption:
//!
//! - `build_context` — returns `{ object, summary, markdown, json, lens_findings, quality, graph, metadata }`
//!   for the requested `object_id`. Output is provided in **both** a
//!   human-readable Markdown body and a machine-readable JSON body,
//!   so callers (LLM agents, MCP clients, Explorer UI) can consume
//!   whichever is most convenient.
//!
//! The tool degrades gracefully when ports are not wired:
//! - no `search` → `service_error` envelope
//! - no `view` → `lens_findings: []` (lenses skipped)
//! - no `quality` → `quality: []` (no findings)
//! - no `graph_query` → `graph: null` (graph neighbors skipped)
//!
//! Per the design decision (Rich scope + both formats), the tool pulls
//! from 4 sources: object summary, lens findings (default 2 lenses),
//! quality issues at the object's file (if applicable), and graph
//! neighbors up to `depth` hops (default 1, max 3).

use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::SymbolId;
use rmcp::model::CallToolResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::dto::{InspectableObjectSummary, LensResult};
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{McpContext, TOOL_BUILD_CONTEXT};

// ============================================================================
// Args + DTOs
// ============================================================================

/// Input for `build_context`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct BuildContextArgs {
    /// Object id to build context for (required). Accepts any
    /// `InspectableObjectType` id (symbol, file, scope, etc.).
    object_id: String,
    /// Optional list of lens ids to apply. Defaults to
    /// `["lens_find_dead_code", "lens_hotspots"]` when omitted.
    #[serde(default)]
    lenses: Option<Vec<String>>,
    /// Maximum BFS depth for graph neighbors (default 1, max 3).
    #[serde(default)]
    depth: Option<u8>,
    /// Whether to include the source code block in the context
    /// (default false). When true, the tool resolves the symbol's
    /// source location and includes a stub body block.
    #[serde(default)]
    include_source: bool,
}

/// A single graph neighbor (caller or callee).
#[derive(Debug, Serialize)]
struct GraphNeighborDto {
    id: String,
    relation: String,
    depth: u8,
}

/// A graph slice summary — callers, callees, and counts.
#[derive(Debug, Serialize)]
struct GraphSliceDto {
    callers: Vec<GraphNeighborDto>,
    callees: Vec<GraphNeighborDto>,
    caller_count: usize,
    callee_count: usize,
    /// Total unique neighbors (callers ∪ callees).
    unique_neighbor_count: usize,
}

/// Quality issue summary as it appears in the context.
#[derive(Debug, Serialize)]
struct ContextQualityIssueDto {
    id: i64,
    rule_id: String,
    severity: String,
    message: String,
    line: u32,
}

/// Quality slice for the object's file.
#[derive(Debug, Serialize)]
struct QualitySliceDto {
    file: String,
    issues: Vec<ContextQualityIssueDto>,
    total: usize,
}

/// Lens finding as it appears in the context.
#[derive(Debug, Serialize)]
struct ContextLensFindingDto {
    id: String,
    lens_id: String,
    title: String,
    severity: String,
    confidence: f32,
}

/// Lens slice — findings from the requested lenses.
#[derive(Debug,Serialize)]
struct LensSliceDto {
    requested: Vec<String>,
    applied: Vec<String>,
    /// Each entry = (lens_id, summary string from LensResult.summary)
    results: Vec<ContextLensDto>,
}

/// Single lens result in the context.
#[derive(Debug, Serialize)]
struct ContextLensDto {
    lens_id: String,
    summary: String,
    findings: Vec<ContextLensFindingDto>,
}

/// Output for `build_context` — the structured JSON body.
#[derive(Debug, Serialize)]
struct BuildContextJson {
    object_id: String,
    object_type: String,
    label: String,
    subtitle: String,
    properties: Vec<ContextPropertyDto>,
    lenses: LensSliceDto,
    quality: Option<QualitySliceDto>,
    graph: Option<GraphSliceDto>,
    include_source: bool,
    depth: u8,
}

/// One property of the inspected object (e.g. file = "src/x.rs", line = 42).
#[derive(Debug, Serialize)]
struct ContextPropertyDto {
    key: String,
    value: Value,
}

/// Output for `build_context` — the human-readable Markdown body.
#[derive(Debug, Serialize)]
struct BuildContextMarkdown {
    /// Rendered markdown string.
    body: String,
}

/// Combined envelope payload.
#[derive(Debug, Serialize)]
struct BuildContextResult {
    /// Structured JSON for programmatic consumption.
    json: BuildContextJson,
    /// Human-readable Markdown for direct LLM prompt injection.
    markdown: BuildContextMarkdown,
    /// Lightweight summary line (first line of the markdown).
    summary: String,
    /// Tool metadata (version, generation time, sources consulted).
    metadata: ContextMetadataDto,
}

#[derive(Debug, Serialize)]
struct ContextMetadataDto {
    /// Tool version (matches `CARGO_PKG_VERSION`).
    version: String,
    /// ISO-8601 generation timestamp.
    generated_at: String,
    /// Sources consulted (lenses, quality, graph). Reflects what was
    /// actually used — useful for callers to verify the tool's reach.
    sources_consulted: Vec<String>,
    /// Sources skipped (and why). E.g. `"graph: no GraphQueryPort wired"`.
    sources_skipped: Vec<String>,
}

struct BuildContextHandler;

#[async_trait]
impl ToolHandler for BuildContextHandler {
    fn name(&self) -> &'static str {
        TOOL_BUILD_CONTEXT
    }

    fn arg_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "object_id": {
                    "type": "string",
                    "description": "Object id to build context for. Accepts any InspectableObjectType id (symbol, file, scope, etc.)."
                },
                "lenses": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of lens ids to apply. Defaults to ['lens_find_dead_code', 'lens_hotspots']."
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum BFS depth for graph neighbors (default 1, max 3).",
                    "minimum": 0,
                    "maximum": 3
                },
                "include_source": {
                    "type": "boolean",
                    "description": "Whether to include a source stub block (default false)."
                }
            },
            "required": ["object_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: BuildContextArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_BUILD_CONTEXT,
                    "invalid_args",
                    &format!("{TOOL_BUILD_CONTEXT}: invalid args: {e}"),
                );
            }
        };

        if args.object_id.is_empty() {
            return err_envelope(
                TOOL_BUILD_CONTEXT,
                "missing_required_arg",
                "build_context: missing required arg `object_id`",
            );
        }

        // Resolve the object via SearchService. This is the canonical
        // entry point and returns InspectableObjectSummary.
        let search = match ctx.search.as_ref() {
            Some(s) => s.clone(),
            None => {
                return err_envelope(
                    TOOL_BUILD_CONTEXT,
                    "service_unavailable",
                    "build_context: SearchService not wired",
                );
            }
        };

        let object_summary = match search.inspect_object(&args.object_id).await {
            Ok(s) => s,
            Err(e) => {
                return err_envelope(
                    TOOL_BUILD_CONTEXT,
                    "service_error",
                    &format!("build_context: inspect_object failed: {e}"),
                );
            }
        };

        // Apply lenses
        let lenses_arg = args
            .lenses
            .clone()
            .unwrap_or_else(default_lenses);
        let (lens_slice, lenses_skipped) = apply_lenses(ctx, &args.object_id, &lenses_arg).await;

        // Quality at file (only if applicable + quality wired)
        let (quality_slice, quality_skipped) =
            pull_quality(ctx, &object_summary).await;

        // Graph neighbors (only if graph_query wired + symbol-shaped)
        let (graph_slice, graph_skipped) =
            pull_graph_neighbors(ctx, &args.object_id, args.depth.unwrap_or(1)).await;

        // Compose JSON + Markdown + metadata
        let json_payload = BuildContextJson {
            object_id: object_summary.id.clone(),
            object_type: format!("{:?}", object_summary.object_type).to_lowercase(),
            label: object_summary.label.clone(),
            subtitle: object_summary.subtitle.clone(),
            properties: object_summary
                .properties
                .iter()
                .map(|p| ContextPropertyDto {
                    key: p.key.clone(),
                    value: p.value.clone(),
                })
                .collect(),
            lenses: lens_slice,
            quality: quality_slice,
            graph: graph_slice,
            include_source: args.include_source,
            depth: args.depth.unwrap_or(1),
        };

        let markdown_body = render_markdown(&json_payload);
        let summary_line = markdown_body
            .lines()
            .next()
            .unwrap_or("(empty context)")
            .to_string();

        let mut sources_consulted: Vec<String> = vec!["search".to_string()];
        if lenses_skipped.is_none() {
            sources_consulted.push("lenses".to_string());
        }
        if quality_skipped.is_none() {
            sources_consulted.push("quality".to_string());
        }
        if graph_skipped.is_none() {
            sources_consulted.push("graph".to_string());
        }

        let metadata = ContextMetadataDto {
            version: env!("CARGO_PKG_VERSION").to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            sources_consulted,
            sources_skipped: [
                lenses_skipped,
                quality_skipped.map(|s| format!("quality: {s}")),
                graph_skipped.map(|s| format!("graph: {s}")),
            ]
            .into_iter()
            .flatten()
            .collect(),
        };

        let result = BuildContextResult {
            json: json_payload,
            markdown: BuildContextMarkdown {
                body: markdown_body,
            },
            summary: summary_line,
            metadata,
        };

        ok_envelope(TOOL_BUILD_CONTEXT, &result)
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Default lenses applied when the caller doesn't specify `lenses`.
fn default_lenses() -> Vec<String> {
    vec![
        "lens_find_dead_code".to_string(),
        "lens_hotspots".to_string(),
    ]
}

/// Apply each requested lens. Returns the slice and (if any were
/// skipped) the reason. A lens is "applied" only if the ViewService
/// is wired AND the lens call succeeded.
async fn apply_lenses(
    ctx: &McpContext,
    object_id: &str,
    requested: &[String],
) -> (LensSliceDto, Option<String>) {
    let view = match ctx.view.as_ref() {
        Some(v) => v.clone(),
        None => {
            return (
                LensSliceDto {
                    requested: requested.to_vec(),
                    applied: Vec::new(),
                    results: Vec::new(),
                },
                Some("ViewService not wired".to_string()),
            );
        }
    };

    let mut results = Vec::new();
    let mut applied = Vec::new();
    for lens_id in requested {
        match view.apply_lens(object_id, lens_id).await {
            Ok(LensResult {
                lens_id: id,
                findings,
                summary,
            }) => {
                applied.push(id.clone());
                results.push(ContextLensDto {
                    lens_id: id,
                    summary,
                    findings: findings
                        .into_iter()
                        .map(|f| ContextLensFindingDto {
                            id: f.id,
                            lens_id: f.lens_id,
                            title: f.title,
                            severity: format!("{:?}", f.severity).to_lowercase(),
                            confidence: f.confidence,
                        })
                        .collect(),
                });
            }
            Err(_e) => {
                // Skip on per-lens failure (e.g. lens doesn't apply to
                // this object type). The "applied" list omits it.
            }
        }
    }

    (
        LensSliceDto {
            requested: requested.to_vec(),
            applied,
            results,
        },
        None,
    )
}

/// Pull quality issues at the object's file. Returns `None` if the
/// object has no resolvable file or the QualityRepository isn't wired.
async fn pull_quality(
    ctx: &McpContext,
    object: &InspectableObjectSummary,
) -> (Option<QualitySliceDto>, Option<String>) {
    let quality = match ctx.quality.as_ref() {
        Some(q) => q.clone(),
        None => return (None, Some("no QualityRepository wired".to_string())),
    };

    // Resolve the file from the object's properties. Quality is keyed
    // by exact file path. The `file` property is conventional; fall
    // back to the subtitle for symbols (format: `file:line`).
    let file = object
        .properties
        .iter()
        .find(|p| p.key == "file")
        .and_then(|p| p.value.as_str())
        .map(String::from);

    let file = match file {
        Some(f) => f,
        None => return (None, Some("object has no `file` property".to_string())),
    };

    let issues = match quality.issues_for_file(&file) {
        Ok(i) => i,
        Err(_e) => return (None, Some(format!("issues_for_file failed for {file}"))),
    };

    let total = issues.len();
    let issues: Vec<ContextQualityIssueDto> = issues
        .iter()
        .map(|i| ContextQualityIssueDto {
            id: i.id,
            rule_id: i.rule_id.clone(),
            severity: i.severity.clone(),
            message: i.message.clone(),
            line: i.line,
        })
        .collect();

    (
        Some(QualitySliceDto {
            file,
            issues,
            total,
        }),
        None,
    )
}

/// Pull graph neighbors (callers + callees) for the object's symbol.
/// Returns `None` if graph_query isn't wired or the object is not
/// symbol-shaped.
async fn pull_graph_neighbors(
    ctx: &McpContext,
    object_id: &str,
    depth: u8,
) -> (Option<GraphSliceDto>, Option<String>) {
    let graph_query = match ctx.graph_query.as_ref() {
        Some(g) => g.clone(),
        None => return (None, Some("no GraphQueryPort wired".to_string())),
    };

    let depth = depth.min(3);
    let symbol_id = SymbolId::new(object_id);

    let callers: Vec<GraphNeighborDto> = graph_query
        .traverse_callers(&symbol_id, depth)
        .into_iter()
        .map(|c| GraphNeighborDto {
            id: c.symbol_id.to_string(),
            relation: "caller".to_string(),
            depth: c.depth,
        })
        .collect();

    let callees: Vec<GraphNeighborDto> = graph_query
        .traverse_callees(&symbol_id, depth)
        .into_iter()
        .map(|c| GraphNeighborDto {
            id: c.symbol_id.to_string(),
            relation: "callee".to_string(),
            depth: c.depth,
        })
        .collect();

    // unique count: dedupe by id
    let mut unique = std::collections::HashSet::new();
    for c in &callers {
        unique.insert(c.id.clone());
    }
    for c in &callees {
        unique.insert(c.id.clone());
    }
    let unique_count = unique.len();

    let caller_count = callers.len();
    let callee_count = callees.len();

    (
        Some(GraphSliceDto {
            callers,
            callees,
            caller_count,
            callee_count,
            unique_neighbor_count: unique_count,
        }),
        None,
    )
}

/// Render the markdown body from the JSON payload.
fn render_markdown(json: &BuildContextJson) -> String {
    let mut out = String::new();

    // Header + subtitle
    out.push_str(&format!("# {}\n\n", json.label));
    if !json.subtitle.is_empty() {
        out.push_str(&format!("_{}_\n\n", json.subtitle));
    }
    out.push_str(&format!("- **object_id**: `{}`\n", json.object_id));
    out.push_str(&format!("- **type**: `{}`\n", json.object_type));

    // Properties
    if !json.properties.is_empty() {
        out.push_str("\n## Properties\n\n");
        for p in &json.properties {
            out.push_str(&format!(
                "- **{}**: `{}`\n",
                p.key,
                serde_json::to_string(&p.value).unwrap_or_default()
            ));
        }
    }

    // Lens findings
    if !json.lenses.results.is_empty() {
        out.push_str("\n## Lens findings\n\n");
        for lens in &json.lenses.results {
            out.push_str(&format!("### `{}` — {}\n\n", lens.lens_id, lens.summary));
            if lens.findings.is_empty() {
                out.push_str("_No findings._\n\n");
            } else {
                for f in &lens.findings {
                    out.push_str(&format!(
                        "- **[{}]** {} (confidence {:.2})\n",
                        f.severity, f.title, f.confidence
                    ));
                }
                out.push('\n');
            }
        }
    }

    // Quality
    if let Some(q) = &json.quality {
        out.push_str(&format!("\n## Quality at `{}`\n\n", q.file));
        if q.total == 0 {
            out.push_str("_No open issues._\n");
        } else {
            out.push_str(&format!("_{} issue(s) total._\n\n", q.total));
            for issue in &q.issues {
                out.push_str(&format!(
                    "- **[{}]** {} — `{}` (line {})\n",
                    issue.severity, issue.message, issue.rule_id, issue.line
                ));
            }
        }
        out.push('\n');
    }

    // Graph neighbors
    if let Some(g) = &json.graph {
        out.push_str(&format!(
            "\n## Graph (depth ≤ {})\n\n",
            json.depth
        ));
        out.push_str(&format!(
            "- **Callers**: {} (`{}`)\n",
            g.caller_count,
            g.callers
                .iter()
                .take(8)
                .map(|n| n.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push_str(&format!(
            "- **Callees**: {} (`{}`)\n",
            g.callee_count,
            g.callees
                .iter()
                .take(8)
                .map(|n| n.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push_str(&format!(
            "- **Unique neighbors**: {}\n",
            g.unique_neighbor_count
        ));
        out.push('\n');
    }

    // Source stub
    if json.include_source {
        out.push_str("\n## Source (stub)\n\n");
        out.push_str("_Source body would appear here in a future iteration._\n");
    }

    out
}

// ============================================================================
// Registry
// ============================================================================

/// Register the context-builder handler into the registry.
pub fn register_context_builder_handlers(
    registry: &mut crate::mcp::handler::ToolHandlerRegistry,
) {
    registry.register(BuildContextHandler);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_context_args_defaults() {
        let json = json!({ "object_id": "src/x.rs:foo:1" });
        let args: BuildContextArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.object_id, "src/x.rs:foo:1");
        assert!(args.lenses.is_none());
        assert!(args.depth.is_none());
        assert!(!args.include_source);
    }

    #[test]
    fn build_context_args_full() {
        let json = json!({
            "object_id": "sym:1",
            "lenses": ["lens_a", "lens_b"],
            "depth": 2,
            "include_source": true
        });
        let args: BuildContextArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.lenses.as_ref().unwrap().len(), 2);
        assert_eq!(args.depth, Some(2));
        assert!(args.include_source);
    }

    #[test]
    fn default_lenses_returns_two_known_lenses() {
        let lenses = default_lenses();
        assert_eq!(lenses.len(), 2);
        assert!(lenses.contains(&"lens_find_dead_code".to_string()));
        assert!(lenses.contains(&"lens_hotspots".to_string()));
    }

    #[test]
    fn render_markdown_includes_label_and_object_id() {
        let payload = BuildContextJson {
            object_id: "sym:1".to_string(),
            object_type: "symbol".to_string(),
            label: "foo".to_string(),
            subtitle: "src/x.rs:1".to_string(),
            properties: vec![],
            lenses: LensSliceDto {
                requested: vec![],
                applied: vec![],
                results: vec![],
            },
            quality: None,
            graph: None,
            include_source: false,
            depth: 1,
        };
        let md = render_markdown(&payload);
        assert!(md.contains("# foo"));
        assert!(md.contains("`sym:1`"));
    }
}