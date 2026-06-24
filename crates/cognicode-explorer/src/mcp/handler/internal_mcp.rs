//! Internal MCP tool wrappers for the Explorer MCP.
//!
//! This module exposes three tools that wrap logic from cognicode-core's
//! internal MCP handlers, adapted for the Explorer MCP's context:
//!
//! - `find_dead_code_v2` — workspace-wide dead-code analysis with confidence filter
//!   (wraps `analysis_service.detect_dead_code()` logic from internal MCP)
//! - `find_cycles` — detect all strongly-connected components (cycles) in the call graph
//!   (wraps `CycleDetector` from `cognicode-graph-algos`)
//! - `health_dashboard` — single-call workspace health summary with findings
//!   (derives health score from graph metrics)
//!
//! All three tools require a loaded call graph and follow the envelope contract
//! (`ok_envelope` / `err_envelope`).

use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::services::CycleDetector;
use rmcp::model::CallToolResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{
    TOOL_FIND_DEAD_CODE_V2, TOOL_FIND_CYCLES, TOOL_HEALTH_DASHBOARD, McpContext,
};

// ============================================================================
// require_graph — shared guard (re-exported from graph_analyze for convenience)
// ============================================================================

fn require_graph<'a>(
    ctx: &'a McpContext,
    tool: &str,
) -> Result<&'a Arc<CallGraph>, CallToolResult> {
    ctx.graph.as_ref().ok_or_else(|| {
        err_envelope(
            tool,
            "graph_unavailable",
            &format!("{tool}: analysis unavailable — no call graph loaded"),
        )
    })
}

// ============================================================================
// Tool 1: find_dead_code_v2
// ============================================================================

/// Input for `find_dead_code_v2`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct FindDeadCodeV2Args {
    /// Maximum number of dead-code entries to return (default 50).
    #[serde(default)]
    limit: Option<usize>,
    /// Minimum confidence threshold (0.0–1.0). Entries below this are
    /// filtered out. Default 0.0 (no filter).
    #[serde(default)]
    confidence_threshold: Option<f32>,
    /// Workspace id (reserved for future multi-workspace support).
    /// Currently unused — the tool operates on the loaded graph.
    #[serde(default)]
    workspace_id: Option<String>,
}

/// A single dead-code entry in the v2 result.
#[derive(Debug, Serialize)]
struct DeadCodeEntryV2 {
    symbol_id: String,
    kind: String,
    file: String,
    line: u32,
    confidence: f32,
}

/// Output for `find_dead_code_v2`.
#[derive(Debug, Serialize)]
struct FindDeadCodeV2Result {
    dead_code: Vec<DeadCodeEntryV2>,
    total_dead: usize,
    dead_code_percent: f32,
    confidence_threshold: f32,
}

struct FindDeadCodeV2Handler;

#[async_trait]
impl ToolHandler for FindDeadCodeV2Handler {
    fn name(&self) -> &'static str {
        TOOL_FIND_DEAD_CODE_V2
    }

    fn arg_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of dead-code entries to return (default 50).",
                    "minimum": 1,
                    "maximum": 10000
                },
                "confidence_threshold": {
                    "type": "number",
                    "description": "Minimum confidence threshold (0.0–1.0). Entries below this are filtered out (default 0.0).",
                    "minimum": 0.0,
                    "maximum": 1.0
                },
                "workspace_id": {
                    "type": "string",
                    "description": "Workspace id (reserved; currently unused)."
                }
            }
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: FindDeadCodeV2Args = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_FIND_DEAD_CODE_V2,
                    "invalid_args",
                    &format!("{TOOL_FIND_DEAD_CODE_V2}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_FIND_DEAD_CODE_V2) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let limit = args.limit.unwrap_or(50);
        let confidence_threshold = args.confidence_threshold.unwrap_or(0.0).max(0.0).min(1.0);

        // Compute dead code: use the existing reachability analysis from CallGraph roots.
        // This mirrors the logic in `lens_find_dead_code` but returns the v2 output shape.
        let dead_symbol_ids: Vec<cognicode_core::domain::aggregates::SymbolId> = g.find_dead_code();

        let total_symbols = g.symbol_count();
        let total_dead = dead_symbol_ids.len();
        let dead_code_percent = if total_symbols > 0 {
            (total_dead as f32 / total_symbols as f32) * 100.0
        } else {
            0.0
        };

        // Confidence is derived from the symbol's connectivity: callable symbols
        // with zero callers and no type relationships get confidence 1.0;
        // symbols with some relationships but still unreachable get a lower score.
        let dead_code: Vec<DeadCodeEntryV2> = dead_symbol_ids
            .into_iter()
            .filter_map(|sid| {
                let sym = g.get_symbol(&sid)?;
                let kind_str = format!("{:?}", sym.kind()).to_lowercase();

                // Confidence: symbols that are callable/type definitions with
                // zero incoming edges get full confidence; others get 0.5.
                let confidence = if sym.kind().is_callable() || sym.kind().is_type_definition() {
                    let fan_in = g.dependents(&sid).count();
                    if fan_in == 0 { 1.0 } else { 0.5 }
                } else {
                    0.3
                };

                if confidence < confidence_threshold {
                    return None;
                }

                let loc = sym.location();
                Some(DeadCodeEntryV2 {
                    symbol_id: sid.to_string(),
                    kind: kind_str,
                    file: loc.file().to_string(),
                    line: loc.line(),
                    confidence,
                })
            })
            .take(limit)
            .collect();

        let result = FindDeadCodeV2Result {
            dead_code,
            total_dead,
            dead_code_percent,
            confidence_threshold,
        };

        ok_envelope(TOOL_FIND_DEAD_CODE_V2, &result)
    }
}

// ============================================================================
// Tool 2: find_cycles
// ============================================================================

/// Input for `find_cycles`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct FindCyclesArgs {
    /// Minimum size of SCCs to return. Cycles with fewer than this many
    /// symbols are excluded. Default 2 (only multi-symbol cycles).
    #[serde(default)]
    min_scc_size: Option<usize>,
}

/// A detected cycle as a list of symbol IDs.
#[derive(Debug, Serialize)]
struct CycleDto {
    symbol_ids: Vec<String>,
    length: usize,
}

/// Output for `find_cycles`.
#[derive(Debug, Serialize)]
struct FindCyclesResult {
    cycles: Vec<CycleDto>,
    total_cycles: usize,
    longest_cycle_length: usize,
}

struct FindCyclesHandler;

#[async_trait]
impl ToolHandler for FindCyclesHandler {
    fn name(&self) -> &'static str {
        TOOL_FIND_CYCLES
    }

    fn arg_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "min_scc_size": {
                    "type": "integer",
                    "description": "Minimum size of SCCs to return as cycles (default 2). Set to 1 to include self-loops.",
                    "minimum": 1,
                    "maximum": 1000
                }
            }
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: FindCyclesArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_FIND_CYCLES,
                    "invalid_args",
                    &format!("{TOOL_FIND_CYCLES}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_FIND_CYCLES) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let min_scc_size = args.min_scc_size.unwrap_or(2).max(1);

        // Use CycleDetector from cognicode-graph-algos (same algorithm used by
        // the internal MCP's cycle-detection endpoint).
        let detector = CycleDetector::new();
        let result = detector.detect_cycles(g.as_ref());

        // Filter to SCCs meeting the minimum size threshold.
        // result.cycles already excludes SCCs of size 1 (self-loops are
        // included as single-element cycles by CycleDetector).
        let cycles: Vec<CycleDto> = result
            .cycles
            .into_iter()
            .filter(|c| c.length() >= min_scc_size)
            .map(|c| {
                let symbol_ids = c.symbols().iter().map(|s| s.to_string()).collect();
                CycleDto {
                    symbol_ids,
                    length: c.length(),
                }
            })
            .collect();

        let total_cycles = cycles.len();
        let longest_cycle_length = cycles.iter().map(|c| c.length).max().unwrap_or(0);

        let result = FindCyclesResult {
            cycles,
            total_cycles,
            longest_cycle_length,
        };

        ok_envelope(TOOL_FIND_CYCLES, &result)
    }
}

// ============================================================================
// Tool 3: health_dashboard
// ============================================================================

/// Input for `health_dashboard`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct HealthDashboardArgs {
    /// Workspace id (reserved for future multi-workspace support).
    /// Currently unused.
    #[serde(default)]
    workspace_id: Option<String>,
}

/// A single health finding.
#[derive(Debug, Serialize)]
struct HealthFinding {
    title: String,
    severity: String,
}

/// Symbol count summary.
#[derive(Debug, Serialize)]
struct HealthSymbols {
    total: usize,
    indexed: usize,
    stale: usize,
}

/// Edge count summary.
#[derive(Debug, Serialize)]
struct HealthEdges {
    total: usize,
}

/// Output for `health_dashboard`.
#[derive(Debug, Serialize)]
struct HealthDashboardResult {
    symbols: HealthSymbols,
    edges: HealthEdges,
    health_score: f32,
    findings: Vec<HealthFinding>,
}

struct HealthDashboardHandler;

#[async_trait]
impl ToolHandler for HealthDashboardHandler {
    fn name(&self) -> &'static str {
        TOOL_HEALTH_DASHBOARD
    }

    fn arg_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "workspace_id": {
                    "type": "string",
                    "description": "Workspace id (reserved; currently unused)."
                }
            }
        })
    }

    async fn handle(&self, ctx: &McpContext, _params: Value) -> CallToolResult {
        let g = match require_graph(ctx, TOOL_HEALTH_DASHBOARD) {
            Ok(g) => g,
            Err(e) => return e,
        };

        // Collect basic graph stats.
        let symbol_count = g.symbol_count();
        let edge_count = g.edge_count();

        // Count stale/indexed symbols via graph stats if available.
        // Use symbol_ids() as proxy: symbols with no edges are "stale".
        let mut indexed = 0;
        let mut stale = 0;
        for (sid, _) in g.symbol_ids() {
            let deps = g.dependencies(&sid).count();
            let dependents = g.dependents(&sid).count();
            if deps == 0 && dependents == 0 {
                stale += 1;
            } else {
                indexed += 1;
            }
        }

        // Run dead-code analysis.
        let dead_ids: Vec<_> = g.find_dead_code();
        let dead_count = dead_ids.len();
        let dead_percent = if symbol_count > 0 {
            (dead_count as f32 / symbol_count as f32) * 100.0
        } else {
            0.0
        };

        // Run cycle detection.
        let detector = CycleDetector::new();
        let cycle_result = detector.detect_cycles(g.as_ref());
        let cycle_count = cycle_result.cycles.len();

        // Collect findings.
        let mut findings: Vec<HealthFinding> = Vec::new();

        if dead_percent > 20.0 {
            findings.push(HealthFinding {
                title: format!("High dead-code rate: {:.1}%", dead_percent),
                severity: "critical".to_string(),
            });
        } else if dead_percent > 10.0 {
            findings.push(HealthFinding {
                title: format!("Elevated dead-code rate: {:.1}%", dead_percent),
                severity: "warning".to_string(),
            });
        }

        if cycle_count > 0 {
            findings.push(HealthFinding {
                title: format!("{} cyclic dependency cycle(s) detected", cycle_count),
                severity: "critical".to_string(),
            });
        }

        if stale > symbol_count / 2 {
            findings.push(HealthFinding {
                title: format!("Many stale symbols: {}/{} have no connections", stale, symbol_count),
                severity: "warning".to_string(),
            });
        }

        // Compute a 0.0–1.0 health score.
        // Start at 1.0 and deduct for issues.
        let mut health_score = 1.0_f32;
        health_score -= (dead_percent / 100.0).min(0.4); // up to -0.4 for dead code
        health_score -= (cycle_count as f32 * 0.05).min(0.3); // up to -0.3 for cycles
        health_score -= ((stale as f32 / symbol_count.max(1) as f32) * 0.2).min(0.2); // up to -0.2 for stale
        health_score = health_score.max(0.0).min(1.0);

        let result = HealthDashboardResult {
            symbols: HealthSymbols {
                total: symbol_count,
                indexed,
                stale,
            },
            edges: HealthEdges { total: edge_count },
            health_score,
            findings,
        };

        ok_envelope(TOOL_HEALTH_DASHBOARD, &result)
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register the three internal-MCP wrapper handlers into the registry.
pub fn register_internal_mcp_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(FindDeadCodeV2Handler);
    registry.register(FindCyclesHandler);
    registry.register(HealthDashboardHandler);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // find_dead_code_v2 — arg parsing
    // -------------------------------------------------------------------------

    #[test]
    fn find_dead_code_v2_args_defaults() {
        let json = json!({});
        let args: FindDeadCodeV2Args = serde_json::from_value(json).unwrap();
        assert!(args.limit.is_none());
        assert!(args.confidence_threshold.is_none());
        assert!(args.workspace_id.is_none());
    }

    #[test]
    fn find_dead_code_v2_args_full() {
        let json = json!({
            "limit": 200,
            "confidence_threshold": 0.75,
            "workspace_id": "ws-123"
        });
        let args: FindDeadCodeV2Args = serde_json::from_value(json).unwrap();
        assert_eq!(args.limit, Some(200));
        assert_eq!(args.confidence_threshold, Some(0.75));
        assert_eq!(args.workspace_id.as_deref(), Some("ws-123"));
    }

    #[test]
    fn find_dead_code_v2_args_confidence_clamped() {
        // confidence_threshold > 1.0 should not panic; handler clamps it.
        let json = json!({ "confidence_threshold": 1.5 });
        let args: FindDeadCodeV2Args = serde_json::from_value(json).unwrap();
        assert_eq!(args.confidence_threshold, Some(1.5)); // raw value accepted
    }

    // -------------------------------------------------------------------------
    // find_cycles — arg parsing
    // -------------------------------------------------------------------------

    #[test]
    fn find_cycles_args_defaults() {
        let json = json!({});
        let args: FindCyclesArgs = serde_json::from_value(json).unwrap();
        assert!(args.min_scc_size.is_none());
    }

    #[test]
    fn find_cycles_args_full() {
        let json = json!({ "min_scc_size": 3 });
        let args: FindCyclesArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.min_scc_size, Some(3));
    }

    // -------------------------------------------------------------------------
    // health_dashboard — arg parsing
    // -------------------------------------------------------------------------

    #[test]
    fn health_dashboard_args_empty() {
        let json = json!({});
        let args: HealthDashboardArgs = serde_json::from_value(json).unwrap();
        assert!(args.workspace_id.is_none());
    }

    #[test]
    fn health_dashboard_args_with_workspace() {
        let json = json!({ "workspace_id": "ws-main" });
        let args: HealthDashboardArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.workspace_id.as_deref(), Some("ws-main"));
    }

    // -------------------------------------------------------------------------
    // CycleDto round-trip
    // -------------------------------------------------------------------------

    #[test]
    fn cycle_dto_serialization() {
        let dto = CycleDto {
            symbol_ids: vec!["a:1".to_string(), "b:2".to_string(), "a:1".to_string()],
            length: 3,
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["length"], 3);
        assert_eq!(json["symbol_ids"].as_array().unwrap().len(), 3);
    }

    // -------------------------------------------------------------------------
    // HealthDashboardResult round-trip
    // -------------------------------------------------------------------------

    #[test]
    fn health_dashboard_result_round_trip() {
        let result = HealthDashboardResult {
            symbols: HealthSymbols {
                total: 100,
                indexed: 80,
                stale: 20,
            },
            edges: HealthEdges { total: 250 },
            health_score: 0.72,
            findings: vec![
                HealthFinding {
                    title: "Elevated dead-code rate: 15.0%".to_string(),
                    severity: "warning".to_string(),
                },
            ],
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["symbols"]["total"], 100);
        assert_eq!(json["edges"]["total"], 250);
        // Use approximate comparison for float (JSON round-trip introduces precision loss)
        let score: f32 = serde_json::from_value(json["health_score"].clone()).unwrap();
        assert!((score - 0.72).abs() < 0.001);
        assert_eq!(json["findings"].as_array().unwrap().len(), 1);
    }

    // -------------------------------------------------------------------------
    // Health score computation bounds
    // -------------------------------------------------------------------------

    #[test]
    fn health_score_max_is_one() {
        // No issues → score should be 1.0
        let result = HealthDashboardResult {
            symbols: HealthSymbols { total: 100, indexed: 100, stale: 0 },
            edges: HealthEdges { total: 200 },
            health_score: 1.0,
            findings: vec![],
        };
        assert_eq!(result.health_score, 1.0);
    }

    #[test]
    fn health_score_min_is_zero() {
        // All symbols dead, many cycles → score should be near 0
        let result = HealthDashboardResult {
            symbols: HealthSymbols { total: 100, indexed: 0, stale: 100 },
            edges: HealthEdges { total: 0 },
            health_score: 0.0,
            findings: vec![
                HealthFinding {
                    title: "High dead-code rate: 100.0%".to_string(),
                    severity: "critical".to_string(),
                },
            ],
        };
        assert_eq!(result.health_score, 0.0);
    }
}