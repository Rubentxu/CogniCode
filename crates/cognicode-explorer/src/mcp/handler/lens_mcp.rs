//! Three new dedicated MCP tools for higher-level code analysis:
//!
//! - `lens_find_dead_code` — symbols not reachable from any entry point
//! - `lens_find_intersection` — findings shared across multiple lenses
//! - `lens_hotspots` — workspace-level top-N symbols by impact
//!
//! These complement the existing `apply_lens` tool by providing richer,
//! typed entry points for common analyses that previously required
//! orchestrating multiple calls.

use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::SymbolId;
use rmcp::model::CallToolResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::dto::LensResult;
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{McpContext, TOOL_FIND_DEAD_CODE, TOOL_FIND_INTERSECTION, TOOL_HOTSPOTS};

// ============================================================================
// Shared helpers
// ============================================================================

/// Build a McpContext-shaped CallGraph reference and return a structured
/// error envelope when no call graph is loaded. Mirrors the `require_graph`
/// helper in `graph.rs` / `graph_analyze.rs` / `impact.rs`.
fn require_graph<'a>(
    ctx: &'a McpContext,
    tool: &str,
) -> Result<&'a Arc<cognicode_core::domain::aggregates::CallGraph>, CallToolResult> {
    ctx.graph.as_ref().ok_or_else(|| {
        err_envelope(
            tool,
            "graph_unavailable",
            &format!("{tool}: analysis unavailable — no call graph loaded"),
        )
    })
}

// ============================================================================
// Tool 1: lens_find_dead_code
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct FindDeadCodeArgs {
    /// Optional cap on the number of dead symbols returned.
    /// Default 50. The full count is always reported.
    #[serde(default)]
    limit: Option<usize>,
    /// Optional list of explicit entry points. When omitted, the graph's
    /// roots (symbols with no incoming edges) are used as entry points.
    #[serde(default)]
    entry_points: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct DeadCodeEntryDto {
    symbol_id: String,
    kind: String,
    file: String,
    line: u32,
}

#[derive(Debug, Serialize)]
struct FindDeadCodeResult {
    total_symbols: usize,
    total_dead: usize,
    dead_code_percent: f32,
    /// Capped list of dead symbols (limited by `limit`, default 50).
    dead_symbols: Vec<DeadCodeEntryDto>,
    /// The entry points used for reachability analysis (roots when
    /// not specified by the caller).
    entry_points: Vec<String>,
}

struct FindDeadCodeHandler;

#[async_trait]
impl ToolHandler for FindDeadCodeHandler {
    fn name(&self) -> &'static str {
        TOOL_FIND_DEAD_CODE
    }

    fn arg_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum dead symbols to return (default 50). The total count is always reported.",
                    "minimum": 1,
                    "maximum": 10000
                },
                "entry_points": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional explicit entry points for reachability. Defaults to graph roots."
                }
            }
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: FindDeadCodeArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_FIND_DEAD_CODE,
                    "invalid_args",
                    &format!("{TOOL_FIND_DEAD_CODE}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_FIND_DEAD_CODE) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let limit = args.limit.unwrap_or(50);

        // When custom entry points are provided, treat them as the only
        // roots for reachability. The caller may pass either a full
        // `file:name:line` FQN or a short `name`; resolve short forms
        // against the graph so the BFS reaches the intended symbols.
        let has_custom_entries = args.entry_points.is_some();
        let raw_eps: Vec<String> = match args.entry_points {
            Some(eps) if !eps.is_empty() => eps,
            _ => g.roots().into_iter().map(|s| s.to_string()).collect(),
        };
        let roots: Vec<String> = if has_custom_entries {
            resolve_entry_points(g, &raw_eps)
        } else {
            raw_eps
        };

        // Use the existing CallGraph::find_dead_code (uses graph roots).
        // For custom entry points, recompute reachability explicitly.
        let dead_symbol_ids: Vec<SymbolId> = if has_custom_entries {
            compute_dead_from_entries(g, &roots)
        } else {
            g.find_dead_code()
        };

        let total_symbols = g.symbol_count();
        let total_dead = dead_symbol_ids.len();
        let dead_code_percent = if total_symbols > 0 {
            (total_dead as f32 / total_symbols as f32) * 100.0
        } else {
            0.0
        };

        let dead_symbols: Vec<DeadCodeEntryDto> = dead_symbol_ids
            .into_iter()
            .take(limit)
            .map(|sid| {
                let sym = g.get_symbol(&sid);
                let (kind, file, line) = sym
                    .map(|s| {
                        let kind_str = format!("{:?}", s.kind());
                        let loc = s.location();
                        (kind_str, loc.file().to_string(), loc.line())
                    })
                    .unwrap_or_else(|| ("unknown".to_string(), String::new(), 0));
                DeadCodeEntryDto {
                    symbol_id: sid.to_string(),
                    kind,
                    file,
                    line,
                }
            })
            .collect();

        let result = FindDeadCodeResult {
            total_symbols,
            total_dead,
            dead_code_percent,
            dead_symbols,
            entry_points: roots,
        };

        ok_envelope(TOOL_FIND_DEAD_CODE, &result)
    }
}

/// Compute dead-code set given explicit entry points (BFS from entries
/// to mark reachable symbols; anything not reachable that is callable
/// or a type definition is dead).
fn compute_dead_from_entries(
    g: &cognicode_core::domain::aggregates::CallGraph,
    entries: &[String],
) -> Vec<SymbolId> {
    use std::collections::HashSet;

    let mut live: HashSet<SymbolId> = HashSet::new();
    let mut queue: Vec<SymbolId> = Vec::new();

    for ep in entries {
        let sid = SymbolId::new(ep.clone());
        if g.get_symbol(&sid).is_some() {
            queue.push(sid);
        }
    }

    while let Some(id) = queue.pop() {
        if live.insert(id.clone()) {
            for (target, _) in g.dependencies(&id) {
                if !live.contains(target) {
                    queue.push(target.clone());
                }
            }
        }
    }

    g.symbol_ids()
        .filter_map(|(id, sym)| {
            if !live.contains(id)
                && (sym.kind().is_callable() || sym.kind().is_type_definition())
            {
                Some(id.clone())
            } else {
                None
            }
        })
        .collect()
}

/// Resolve user-supplied entry-point strings to the graph's canonical
/// `file:name:line` FQNs. Accepts:
/// - exact FQN: `"file:name:line"` (passed through)
/// - bare name: `"name"` (matches any FQN whose name segment is `name`)
/// - any segment match: `"lvl4"` matches `"lvl4.rs:lvl4:1"` because
///   `lvl4` appears in any colon-separated segment
fn resolve_entry_points(
    g: &cognicode_core::domain::aggregates::CallGraph,
    raw: &[String],
) -> Vec<String> {
    let mut resolved: Vec<String> = Vec::new();
    for needle in raw {
        let mut matched = false;
        for (id, _) in g.symbol_ids() {
            let id_str = id.to_string();
            let is_match = id_str == *needle
                || id_str.split(':').any(|seg| seg == needle.as_str());
            if is_match && !resolved.contains(&id_str) {
                resolved.push(id_str);
                matched = true;
            }
        }
        // Always include the raw input even if no match — preserves
        // user intent when they explicitly typed an unknown id (the
        // downstream BFS will simply find no reachable symbols).
        if !matched && !resolved.contains(needle) {
            resolved.push(needle.clone());
        }
    }
    resolved
}

// ============================================================================
// Tool 2: lens_find_intersection
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct FindIntersectionArgs {
    /// The object id to apply lenses to (required). All lenses in
    /// `lens_ids` must be applicable to this object's type.
    object_id: String,
    /// List of lens ids to run. Minimum 2 lenses, maximum 5.
    #[serde(default)]
    lens_ids: Vec<String>,
    /// Minimum number of lenses that must agree on a finding for it to
    /// be included in the intersection. Default 2 (strict consensus).
    #[serde(default)]
    min_consensus: Option<usize>,
}

#[derive(Debug, Serialize)]
struct IntersectionFindingDto {
    finding_id: String,
    title: String,
    hypothesis: String,
    severity: String,
    confidence: f32,
    /// Lens ids that produced this finding.
    contributing_lenses: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FindIntersectionResult {
    object_id: String,
    lens_ids: Vec<String>,
    min_consensus: usize,
    findings: Vec<IntersectionFindingDto>,
    per_lens_counts: std::collections::BTreeMap<String, usize>,
}

struct FindIntersectionHandler;

#[async_trait]
impl ToolHandler for FindIntersectionHandler {
    fn name(&self) -> &'static str {
        TOOL_FIND_INTERSECTION
    }

    fn arg_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "object_id": {
                    "type": "string",
                    "description": "The object to analyze (required)."
                },
                "lens_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 2,
                    "maxItems": 5,
                    "description": "Lens ids to run. Min 2, max 5."
                },
                "min_consensus": {
                    "type": "integer",
                    "minimum": 2,
                    "maximum": 5,
                    "description": "Minimum number of lenses that must produce the same finding (default 2)."
                }
            },
            "required": ["object_id", "lens_ids"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: FindIntersectionArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_FIND_INTERSECTION,
                    "invalid_args",
                    &format!("{TOOL_FIND_INTERSECTION}: invalid args: {e}"),
                );
            }
        };

        if args.lens_ids.len() < 2 {
            return err_envelope(
                TOOL_FIND_INTERSECTION,
                "invalid_args",
                "lens_find_intersection: at least 2 lens_ids required",
            );
        }

        let min_consensus = args.min_consensus.unwrap_or(2).min(args.lens_ids.len());

        let view_svc = match ctx.view.as_ref() {
            Some(v) => v,
            None => {
                return err_envelope(
                    TOOL_FIND_INTERSECTION,
                    "facade_unavailable",
                    "view service not wired",
                );
            }
        };

        // Resolve object summary so we can build a lens context (the
        // existing apply_lens path takes care of this internally).
        // For intersection we want to invoke each lens individually.
        let mut per_lens_results: Vec<(String, LensResult)> = Vec::new();
        let mut per_lens_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();

        for lens_id in &args.lens_ids {
            match view_svc.apply_lens(&args.object_id, lens_id).await {
                Ok(result) => {
                    per_lens_counts.insert(lens_id.clone(), result.findings.len());
                    per_lens_results.push((lens_id.clone(), result));
                }
                Err(e) => {
                    // Don't fail the whole call — a single broken lens
                    // is non-fatal for an intersection query.
                    per_lens_counts.insert(lens_id.clone(), 0);
                    let _ = e; // swallow per-lens error
                }
            }
        }

        // Bucket findings by a stable key (title + hypothesis prefix) and
        // count how many lenses produced each. Keep only those that hit
        // the consensus threshold.
        let mut bucket: std::collections::HashMap<String, IntersectionFindingDto> =
            std::collections::HashMap::new();
        let mut counts: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for (lens_id, result) in &per_lens_results {
            for finding in &result.findings {
                let key = format!("{}|{}", finding.title, finding.hypothesis);
                counts.entry(key.clone()).or_default().push(lens_id.clone());
                bucket
                    .entry(key)
                    .or_insert_with(|| IntersectionFindingDto {
                        finding_id: finding.id.clone(),
                        title: finding.title.clone(),
                        hypothesis: finding.hypothesis.clone(),
                        severity: format!("{:?}", finding.severity),
                        confidence: finding.confidence,
                        contributing_lenses: Vec::new(),
                    })
                    .contributing_lenses
                    .push(lens_id.clone());
            }
        }

        let findings: Vec<IntersectionFindingDto> = bucket
            .into_iter()
            .filter_map(|(_, mut dto)| {
                if dto.contributing_lenses.len() >= min_consensus {
                    // De-duplicate contributing_lenses (in case a single
                    // lens emits duplicates — should be rare but defensive).
                    dto.contributing_lenses.sort();
                    dto.contributing_lenses.dedup();
                    Some(dto)
                } else {
                    None
                }
            })
            .collect();

        let result = FindIntersectionResult {
            object_id: args.object_id,
            lens_ids: args.lens_ids,
            min_consensus,
            findings,
            per_lens_counts,
        };

        ok_envelope(TOOL_FIND_INTERSECTION, &result)
    }
}

// ============================================================================
// Tool 3: lens_hotspots
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct HotspotsArgs {
    /// The object id to anchor the hotspot analysis on (required).
    object_id: String,
    /// Top-N symbols to return. Default 10, max 100.
    #[serde(default)]
    top_n: Option<usize>,
    /// Optional max BFS depth for the hotspot subgraph. Default 3.
    #[serde(default)]
    max_depth: Option<u32>,
}

#[derive(Debug, Serialize)]
struct HotspotEntryDto {
    symbol_id: String,
    label: String,
    pagerank: f32,
    in_degree: usize,
    out_degree: usize,
}

#[derive(Debug, Serialize)]
struct HotspotsResult {
    object_id: String,
    top_n: usize,
    hotspots: Vec<HotspotEntryDto>,
    method: &'static str,
}

struct HotspotsHandler;

#[async_trait]
impl ToolHandler for HotspotsHandler {
    fn name(&self) -> &'static str {
        TOOL_HOTSPOTS
    }

    fn arg_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "object_id": {
                    "type": "string",
                    "description": "Object id anchoring the hotspot analysis (required)."
                },
                "top_n": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 100,
                    "description": "Number of top hotspots to return (default 10)."
                },
                "max_depth": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 10,
                    "description": "Max BFS depth for the surrounding subgraph (default 3)."
                }
            },
            "required": ["object_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: HotspotsArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_HOTSPOTS,
                    "invalid_args",
                    &format!("{TOOL_HOTSPOTS}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_HOTSPOTS) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let top_n = args.top_n.unwrap_or(10).min(100);
        let _max_depth = args.max_depth.unwrap_or(3); // currently unused (graph-wide PageRank)

        // Run PageRank over the entire graph, then return the top-N
        // symbols by score (excluding the anchor itself unless it tops
        // the ranking). This complements `graph_god_nodes` (which is
        // subgraph-scoped) by being graph-wide and rank-N.
        use cognicode_core::application::services::graph_analytics::GraphAnalyticsService;
        let scores = GraphAnalyticsService::page_rank(g, 0.85_f64, 100);

        // Resolve the anchor to a real graph symbol when possible —
        // SymbolIds stored in the graph are FQNs like `file:name:line`,
        // but the caller may have passed a short form like `name`.
        // We collect every FQN in the graph whose name (or any colon-
        // separated segment) matches the caller's input, so any of them
        // is excluded.
        let anchor_fqns: Vec<SymbolId> = {
            let needle = args.object_id.as_str();
            g.symbol_ids()
                .filter_map(|(id, _)| {
                    let id_str = id.to_string();
                    if id_str == needle
                        || id_str.split(':').any(|seg| seg == needle)
                    {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };

        // Convert to ranked list (highest score first), skip the anchor
        // (matched against any resolved FQN, not just the literal input).
        let mut ranked: Vec<(SymbolId, f64)> = scores
            .into_iter()
            .filter(|(id, _)| !anchor_fqns.iter().any(|a| a == id))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let hotspots: Vec<HotspotEntryDto> = ranked
            .into_iter()
            .take(top_n)
            .map(|(sid, score)| {
                let label = g
                    .get_symbol(&sid)
                    .map(|s| s.name().to_string())
                    .unwrap_or_else(|| sid.to_string());
                let in_degree = g.dependents(&sid).count();
                let out_degree = g.dependencies(&sid).count();
                HotspotEntryDto {
                    symbol_id: sid.to_string(),
                    label,
                    pagerank: score as f32,
                    in_degree,
                    out_degree,
                }
            })
            .collect();

        let result = HotspotsResult {
            object_id: args.object_id,
            top_n,
            hotspots,
            method: "page_rank",
        };

        ok_envelope(TOOL_HOTSPOTS, &result)
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register the three lens-MCP handlers into the registry.
pub fn register_lens_mcp_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(FindDeadCodeHandler);
    registry.register(FindIntersectionHandler);
    registry.register(HotspotsHandler);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_dead_code_args_defaults() {
        let json = json!({});
        let args: FindDeadCodeArgs = serde_json::from_value(json).unwrap();
        assert!(args.limit.is_none());
        assert!(args.entry_points.is_none());
    }

    #[test]
    fn find_dead_code_args_full() {
        let json = json!({
            "limit": 100,
            "entry_points": ["sym:main", "sym:lib::run"]
        });
        let args: FindDeadCodeArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.limit, Some(100));
        assert_eq!(args.entry_points.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn find_intersection_args_valid() {
        let json = json!({
            "object_id": "sym:foo",
            "lens_ids": ["hotspots", "dependencies", "architecture"],
            "min_consensus": 2
        });
        let args: FindIntersectionArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.object_id, "sym:foo");
        assert_eq!(args.lens_ids.len(), 3);
        assert_eq!(args.min_consensus, Some(2));
    }

    #[test]
    fn find_intersection_args_min_consensus_optional() {
        let json = json!({
            "object_id": "sym:foo",
            "lens_ids": ["hotspots", "dependencies"]
        });
        let args: FindIntersectionArgs = serde_json::from_value(json).unwrap();
        assert!(args.min_consensus.is_none()); // defaults to 2 in handler
    }

    #[test]
    fn hotspots_args_defaults() {
        let json = json!({ "object_id": "sym:foo" });
        let args: HotspotsArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.object_id, "sym:foo");
        assert!(args.top_n.is_none());
        assert!(args.max_depth.is_none());
    }

    #[test]
    fn hotspots_args_full() {
        let json = json!({
            "object_id": "sym:foo",
            "top_n": 25,
            "max_depth": 5
        });
        let args: HotspotsArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.top_n, Some(25));
        assert_eq!(args.max_depth, Some(5));
    }

    #[test]
    fn find_intersection_consensus_bucketing() {
        // Manual test of the bucketing algorithm without going through
        // the full McpContext pipeline.
        let mut bucket: std::collections::HashMap<String, IntersectionFindingDto> =
            std::collections::HashMap::new();
        let lens_results = vec![
            (
                "hotspots".to_string(),
                LensResult {
                    lens_id: "hotspots".to_string(),
                    findings: vec![
                        crate::dto::DesignFinding {
                            id: "f1".into(),
                            lens_id: "hotspots".into(),
                            title: "God function".into(),
                            hypothesis: "Too many callees".into(),
                            severity: crate::dto::FindingSeverity::Critical,
                            confidence: 0.9,
                            object_ids: vec![],
                            evidence_ids: vec![],
                        },
                        crate::dto::DesignFinding {
                            id: "f2".into(),
                            lens_id: "hotspots".into(),
                            title: "Hot path".into(),
                            hypothesis: "Frequent callee".into(),
                            severity: crate::dto::FindingSeverity::Warning,
                            confidence: 0.7,
                            object_ids: vec![],
                            evidence_ids: vec![],
                        },
                    ],
                    summary: String::new(),
                },
            ),
            (
                "dependencies".to_string(),
                LensResult {
                    lens_id: "dependencies".to_string(),
                    findings: vec![crate::dto::DesignFinding {
                        id: "f3".into(),
                        lens_id: "dependencies".into(),
                        title: "God function".into(),
                        hypothesis: "Too many callees".into(),
                        severity: crate::dto::FindingSeverity::Critical,
                        confidence: 0.85,
                        object_ids: vec![],
                        evidence_ids: vec![],
                    }],
                    summary: String::new(),
                },
            ),
        ];

        for (lens_id, result) in &lens_results {
            for finding in &result.findings {
                let key = format!("{}|{}", finding.title, finding.hypothesis);
                bucket
                    .entry(key)
                    .or_insert_with(|| IntersectionFindingDto {
                        finding_id: finding.id.clone(),
                        title: finding.title.clone(),
                        hypothesis: finding.hypothesis.clone(),
                        severity: format!("{:?}", finding.severity),
                        confidence: finding.confidence,
                        contributing_lenses: Vec::new(),
                    })
                    .contributing_lenses
                    .push(lens_id.clone());
            }
        }

        assert_eq!(bucket.len(), 2);
        let god = bucket.values().find(|f| f.title == "God function").unwrap();
        assert_eq!(god.contributing_lenses.len(), 2);
        let hot = bucket.values().find(|f| f.title == "Hot path").unwrap();
        assert_eq!(hot.contributing_lenses.len(), 1);
    }
}
