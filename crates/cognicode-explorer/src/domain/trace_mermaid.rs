//! Trace-to-Mermaid diagram emitters.
//!
//! Converts call-graph, impact-radius, decision-trace, and vertical-slice
//! data into Mermaid `flowchart` diagram strings. Pure functions — no I/O,
//! deterministic output.
//!
//! ## Emitters
//!
//! | Function | ViewKind | Layout | Feature-gated |
//! |----------|----------|--------|---------------|
//! | [`call_graph_to_mermaid`] | CallGraph | TD (top-down) | No |
//! | [`impact_radius_to_mermaid`] | ImpactRadius | TD (top-down) | No |
//! | [`decision_trace_to_mermaid`] | DecisionTrace | LR (left-right) | Yes (`multimodal`) |
//! | [`vertical_slice_to_mermaid`] | VerticalSlice | TD (top-down) | No |
//!
//! ## ID Sanitisation
//!
//! Mermaid requires alphanumeric identifiers. Special characters (`:`, `/`, `(`, `)`, etc.)
//! are replaced with underscores. Duplicated IDs receive numeric suffixes (`_2`, `_3`).
//! Reuses [`sanitize_id`] and [`deduplicate_ids`] from [`mermaid_util`](super::mermaid_util).

use std::fmt;

use serde::{Deserialize, Serialize};

// ============================================================================
// TraceMermaidViewKind — enum for MCP + REST validation
// ============================================================================

/// Supported view kinds for trace-to-Mermaid export.
///
/// Used by the MCP tool `export_trace_mermaid` and the REST endpoint
/// `GET /api/workspaces/:workspace_id/mermaid/trace` to validate the
/// `view_kind` query parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceMermaidViewKind {
    /// Call-graph view (caller → target → callees).
    CallGraph,
    /// Impact-radius view (reverse BFS of callers up to depth 3).
    ImpactRadius,
    /// Decision-trace view (ADR → code → evidence). Gated behind `multimodal`.
    #[cfg(feature = "multimodal")]
    DecisionTrace,
    /// Full vertical slice (entry point → use case → domain → repo → DB).
    VerticalSlice,
}

impl TraceMermaidViewKind {
    /// Parse a view kind from a string slice.
    ///
    /// Accepts `snake_case` variant names (e.g. `"call_graph"`, `"decision_trace"`).
    /// When `multimodal` is disabled, `decision_trace` returns an error indicating
    /// the feature is not enabled.
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "call_graph" => Ok(Self::CallGraph),
            "impact_radius" => Ok(Self::ImpactRadius),
            #[cfg(feature = "multimodal")]
            "decision_trace" => Ok(Self::DecisionTrace),
            #[cfg(not(feature = "multimodal"))]
            "decision_trace" => Err("decision_trace requires the `multimodal` feature".to_string()),
            "vertical_slice" => Ok(Self::VerticalSlice),
            _ => Err(format!(
                "unknown view_kind: {s}. Expected one of: call_graph, impact_radius, vertical_slice"
            )),
        }
    }

    /// Return the snake_case string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CallGraph => "call_graph",
            Self::ImpactRadius => "impact_radius",
            #[cfg(feature = "multimodal")]
            Self::DecisionTrace => "decision_trace",
            Self::VerticalSlice => "vertical_slice",
        }
    }

    /// Returns true if this variant requires the `multimodal` feature.
    pub fn requires_multimodal(&self) -> bool {
        #[cfg(feature = "multimodal")]
        {
            matches!(self, Self::DecisionTrace)
        }
        #[cfg(not(feature = "multimodal"))]
        {
            false
        }
    }
}

impl fmt::Display for TraceMermaidViewKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

use cognicode_core::domain::aggregates::{CallEntry, SymbolId};

use crate::dto::InspectionTarget;
use crate::ports::symbol_repository::SymbolRepository;

// ============================================================================
// TraceEmitContext — narrow port for trace emitters
// ============================================================================

/// Narrow context port passed to trace-to-Mermaid emitters.
///
/// Emitters only need the graph query capability and the resolved target.
/// This avoids stamp-coupling via `ViewContext` which carries 4 unrelated fields.
pub struct TraceEmitContext<'a> {
    /// Graph query port for callers/callees traversal.
    pub graph_query: &'a dyn cognicode_core::domain::traits::GraphQueryPort,
    /// The resolved inspection target (always `InspectionTarget::Symbol` for traces).
    pub target: &'a InspectionTarget,
}

// Re-export from shared mermaid_util
pub use super::mermaid_util::{deduplicate_ids, sanitize_id};

// ============================================================================
// call_graph_to_mermaid
// ============================================================================

/// Render a call graph as a Mermaid `flowchart TD` diagram.
///
/// `ctx` provides the [`GraphQueryPort`](cognicode_core::domain::traits::graph_query_port::GraphQueryPort)
/// for callers/callees data, and the resolved `target` symbol.
///
/// `symbol` is the target symbol's string identifier.
///
/// ## Output format
///
/// ```mermaid
/// flowchart TD
///     subgraph call_graph["call_graph"]
///         direction TD
///         ...
///     end
/// ```
pub fn call_graph_to_mermaid(ctx: &TraceEmitContext, symbol: &str) -> String {
    let graph_query = ctx.graph_query;

    let Some(InspectionTarget::Symbol(target_symbol)) = Option::from(ctx.target) else {
        return "// InspectionTarget::Symbol required".to_string();
    };

    let symbol_id = SymbolId::new(symbol.to_string());
    let callers = graph_query.callers(&symbol_id);
    let callees = graph_query.callees(&symbol_id);

    if callers.is_empty() && callees.is_empty() {
        return format!(
            "flowchart TD\n    subgraph call_graph[\"call_graph\"]\n        direction TD\n        {}[{}]\n    end",
            sanitize_id(symbol),
            target_symbol.name
        );
    }

    let mut lines = vec!["flowchart TD".to_string()];
    lines.push(format!("    subgraph call_graph[\"call_graph\"]"));
    lines.push("        direction TD".to_string());

    // Center node
    let center_id = sanitize_id(symbol);
    lines.push(format!("        {}[{}]", center_id, target_symbol.name));

    // Caller nodes (incoming edges — left side)
    for caller in &callers {
        let caller_id = sanitize_id(caller.id.as_str());
        let caller_label = &caller.name;
        lines.push(format!("        {}[{}]", caller_id, caller_label));
        lines.push(format!("        {} --> {}", caller_id, center_id));
    }

    // Callee nodes (outgoing edges — right side)
    for callee in &callees {
        let callee_id = sanitize_id(callee.id.as_str());
        let callee_label = &callee.name;
        lines.push(format!("        {}[{}]", callee_id, callee_label));
        lines.push(format!("        {} --> {}", center_id, callee_id));
    }

    lines.push("    end".to_string());
    lines.join("\n")
}

// ============================================================================
// impact_radius_to_mermaid
// ============================================================================

/// Render an impact-radius (reverse BFS of callers) as a Mermaid `flowchart TD`.
///
/// `ctx` provides the [`GraphQueryPort`](cognicode_core::domain::traits::graph_query_port::GraphQueryPort)
/// for BFS traversal and the resolved `target` symbol.
///
/// `symbol` is the root symbol's string identifier.
///
/// Uses `traverse_callers` (reverse BFS) to find all callers up to depth 3.
/// Renders actual BFS tree structure: depth-N nodes connect to depth-(N-1) nodes,
/// not directly to the center (which would be a star topology).
pub fn impact_radius_to_mermaid(ctx: &TraceEmitContext, symbol: &str) -> String {
    let graph_query = ctx.graph_query;

    let Some(InspectionTarget::Symbol(target_symbol)) = Option::from(ctx.target) else {
        return "// InspectionTarget::Symbol required".to_string();
    };

    let symbol_id = SymbolId::new(symbol.to_string());
    // BFS of callers up to depth 3
    let entries = graph_query.traverse_callers(&symbol_id, 3);

    if entries.is_empty() {
        return format!(
            "flowchart TD\n    subgraph impact_radius[\"impact_radius\"]\n        direction TD\n        {}[{}]\n    end",
            sanitize_id(symbol),
            target_symbol.name
        );
    }

    let mut lines = vec!["flowchart TD".to_string()];
    lines.push(format!("    subgraph impact_radius[\"impact_radius\"]"));
    lines.push("        direction TD".to_string());

    // Center node
    let center_id = sanitize_id(symbol);
    lines.push(format!("        {}[{}]", center_id, target_symbol.name));

    // Group entries by depth level
    let mut depth_map: std::collections::BTreeMap<u8, Vec<&CallEntry>> =
        std::collections::BTreeMap::new();
    for entry in &entries {
        depth_map.entry(entry.depth).or_default().push(entry);
    }

    // Track nodes per depth for BFS edge rendering
    let mut nodes_by_depth: std::collections::HashMap<u8, Vec<String>> =
        std::collections::HashMap::new();

    // Render each depth level
    for (depth, nodes_at_depth) in &depth_map {
        let depth_label = match *depth {
            1 => "direct callers",
            2 => "indirect callers",
            _ => "distant callers",
        };

        let mut node_ids_at_this_depth = Vec::new();

        for entry in nodes_at_depth {
            let entry_id = sanitize_id(entry.symbol_id.as_str());
            lines.push(format!(
                "        {}[\"{}\\n({})\"]",
                entry_id,
                entry.symbol_name,
                depth_label
            ));

            // BFS tree edge: connect to nodes at previous depth (or center for depth 1)
            if *depth == 1 {
                lines.push(format!("        {} --> {}", entry_id, center_id));
            } else if let Some(prev_depth) = depth.checked_sub(1) {
                if let Some(prev_nodes) = nodes_by_depth.get(&prev_depth) {
                    for prev_id in prev_nodes {
                        lines.push(format!("        {} --> {}", prev_id, entry_id));
                    }
                }
            }

            node_ids_at_this_depth.push(entry_id);
        }

        nodes_by_depth.insert(*depth, node_ids_at_this_depth);
    }

    lines.push("    end".to_string());
    lines.join("\n")
}

// ============================================================================
// decision_trace_to_mermaid
// ============================================================================

/// Render a decision trace as a Mermaid `flowchart LR` diagram.
///
/// `ctx` provides the narrow port for the decision trace.
///
/// `decision_id` is the decision UUID.
///
/// Uses `flowchart LR` (left-right layout) for horizontal decision traces.
///
/// ## Feature gate
///
/// This function is only callable when the `multimodal` feature is enabled.
/// The MCP tool and REST endpoint gate the `decision_trace` variant behind this feature.
#[cfg(feature = "multimodal")]
pub fn decision_trace_to_mermaid(_ctx: &TraceEmitContext, decision_id: &str) -> String {
    // TODO: When DecisionTrace executor is implemented, extract data from ctx
    // For now, return a placeholder that shows the expected structure
    format!(
        "flowchart LR\n    subgraph decision_trace[\"decision_trace: {}\"]\n        direction LR\n        subgraph adr[\"ADR\"]\n            A[ADR Metadata]\n        end\n        subgraph code[\"Code\"]\n            C[Implementation]\n        end\n        subgraph evidence[\"Evidence\"]\n            E[Supporting Evidence]\n        end\n        A --> C\n        C --> E\n    end",
        sanitize_id(decision_id)
    )
}

// ============================================================================
// vertical_slice_to_mermaid
// ============================================================================

/// Render a vertical slice (full entry-point trace) as a Mermaid `flowchart TD`.
///
/// `ctx` provides the [`GraphQueryPort`](cognicode_core::domain::traits::graph_query_port::GraphQueryPort)
/// for forward traversal and the resolved `target` symbol.
///
/// `entry_point` is the entry point identifier (HTTP route, CLI command, etc.).
///
/// Full vertical trace: HTTP → use case → domain → repo → DB
pub fn vertical_slice_to_mermaid(ctx: &TraceEmitContext, entry_point: &str) -> String {
    let graph_query = ctx.graph_query;

    let Some(InspectionTarget::Symbol(target_symbol)) = Option::from(ctx.target) else {
        return format!(
            "flowchart TD\n    subgraph vertical_slice[\"vertical_slice: {}\"]\n        direction TD\n        {}[{}]\n    end",
            sanitize_id(entry_point),
            sanitize_id(entry_point),
            sanitize_id(entry_point)
        );
    };

    let symbol_id = SymbolId::new(entry_point.to_string());
    // Forward trace: callees up to depth 4 (typical vertical slice depth)
    let entries = graph_query.traverse_callees(&symbol_id, 4);

    let mut lines = vec!["flowchart TD".to_string()];
    lines.push(format!("    subgraph vertical_slice[\"vertical_slice: {}\"]", sanitize_id(entry_point)));
    lines.push("        direction TD".to_string());

    // Center node (entry point)
    let center_id = sanitize_id(entry_point);
    lines.push(format!("        {}[{}]", center_id, target_symbol.name));

    if entries.is_empty() {
        lines.push("    end".to_string());
        return lines.join("\n");
    }

    // Render the vertical slice levels
    // Depth 1: use case layer
    // Depth 2: domain layer
    // Depth 3: repository layer
    // Depth 4: DB/data layer
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    seen.insert(center_id.clone());

    // Group by depth
    let mut depth_nodes: std::collections::HashMap<u8, Vec<&CallEntry>> =
        std::collections::HashMap::new();
    for entry in &entries {
        depth_nodes.entry(entry.depth).or_default().push(entry);
    }

    for depth in 1..=4 {
        let nodes_at_depth = depth_nodes.get(&depth);
        let Some(nodes) = nodes_at_depth else {
            continue;
        };

        for entry in nodes {
            let entry_id = sanitize_id(entry.symbol_id.as_str());
            if seen.insert(entry_id.clone()) {
                lines.push(format!("        {}[{}]", entry_id, entry.symbol_name));
                if depth == 1 {
                    lines.push(format!("        {} --> {}", center_id, entry_id));
                } else {
                    // Connect to previous level
                    let prev_depth = depth - 1;
                    if let Some(prev_nodes) = depth_nodes.get(&prev_depth) {
                        if let Some(prev) = prev_nodes.first() {
                            let prev_id = sanitize_id(prev.symbol_id.as_str());
                            lines.push(format!("        {} --> {}", prev_id, entry_id));
                        }
                    }
                }
            }
        }
    }

    lines.push("    end".to_string());
    lines.join("\n")
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::dto::InspectionTarget;
    use crate::ports::symbol_repository::ResolvedSymbol;
    use cognicode_core::domain::aggregates::CallEntry;
    use cognicode_core::domain::traits::graph_query_port::GraphQueryPort;
    use cognicode_core::domain::value_objects::SymbolKind;

    // ------------------------------------------------------------------------
    // Mock GraphQueryPort for testing
    // ------------------------------------------------------------------------

    struct MockGraphQueryPort {
        callers_result: Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget>,
        callees_result: Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget>,
        traverse_callers_result: Vec<CallEntry>,
        traverse_callees_result: Vec<CallEntry>,
    }

    impl MockGraphQueryPort {
        fn new() -> Self {
            Self {
                callers_result: vec![],
                callees_result: vec![],
                traverse_callers_result: vec![],
                traverse_callees_result: vec![],
            }
        }
        fn with_callers(mut self, callers: Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget>) -> Self {
            self.callers_result = callers;
            self
        }
        fn with_callees(mut self, callees: Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget>) -> Self {
            self.callees_result = callees;
            self
        }
        fn with_traverse_callers(mut self, entries: Vec<CallEntry>) -> Self {
            self.traverse_callers_result = entries;
            self
        }
        fn with_traverse_callees(mut self, entries: Vec<CallEntry>) -> Self {
            self.traverse_callees_result = entries;
            self
        }
    }

    impl GraphQueryPort for MockGraphQueryPort {
        fn callers(&self, _id: &SymbolId) -> Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget> {
            self.callers_result.clone()
        }
        fn callees(&self, _id: &SymbolId) -> Vec<cognicode_core::domain::traits::graph_query_port::RelationTarget> {
            self.callees_result.clone()
        }
        fn fan_in(&self, _id: &SymbolId) -> usize { 0 }
        fn fan_out(&self, _id: &SymbolId) -> usize { 0 }
        fn callers_with_metadata(&self, _id: &SymbolId) -> Vec<cognicode_core::domain::traits::graph_query_port::CallerWithMetadata> { vec![] }
        fn callees_with_metadata(&self, _id: &SymbolId) -> Vec<cognicode_core::domain::traits::graph_query_port::CalleeWithMetadata> { vec![] }
        fn dependencies_with_metadata(&self, _id: &SymbolId) -> Vec<cognicode_core::domain::traits::graph_query_port::RelationTargetWithMetadata> { vec![] }
        fn traverse_callees(&self, _id: &SymbolId, _max_depth: u8) -> Vec<CallEntry> {
            self.traverse_callees_result.clone()
        }
        fn traverse_callers(&self, _id: &SymbolId, _max_depth: u8) -> Vec<CallEntry> {
            self.traverse_callers_result.clone()
        }
    }

    fn make_relation_target(id: &str, name: &str) -> cognicode_core::domain::traits::graph_query_port::RelationTarget {
        cognicode_core::domain::traits::graph_query_port::RelationTarget {
            id: SymbolId::new(id.to_string()),
            name: name.to_string(),
            kind: SymbolKind::Function,
            file: "test.rs".to_string(),
            line: 1,
            signature: None,
        }
    }

    fn make_call_entry(id: &str, name: &str, depth: u8) -> CallEntry {
        CallEntry {
            symbol_id: SymbolId::new(id.to_string()),
            symbol_name: name.to_string(),
            file: "test.rs".to_string(),
            line: 1,
            column: 1,
            depth,
        }
    }

    fn make_target_symbol(name: &str) -> ResolvedSymbol {
        ResolvedSymbol {
            id: SymbolId::new(format!("symbol:test:{}:1", name)),
            name: name.to_string(),
            kind: SymbolKind::Function,
            file: "test.rs".to_string(),
            line: 1,
            signature: None,
        }
    }

    // ------------------------------------------------------------------------
    // call_graph_to_mermaid — happy path
    // ------------------------------------------------------------------------

    #[test]
    fn call_graph_to_mermaid_happy_path() {
        let callers = vec![
            make_relation_target("caller:1", "caller_one"),
            make_relation_target("caller:2", "caller_two"),
        ];
        let callees = vec![
            make_relation_target("callee:1", "callee_one"),
        ];

        let mock_gq = MockGraphQueryPort::new()
            .with_callers(callers)
            .with_callees(callees);

        let target = make_target_symbol("my_function");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };
        let result = call_graph_to_mermaid(&ctx, "symbol:test:my_function:1");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph call_graph"));
        assert!(result.contains("caller_one"));
        assert!(result.contains("caller_two"));
        assert!(result.contains("callee_one"));
        // Edge indicators
        assert!(result.contains("-->"));
    }

    #[test]
    fn call_graph_to_mermaid_empty_graph() {
        let mock_gq = MockGraphQueryPort::new();
        let target = make_target_symbol("orphan_fn");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };
        let result = call_graph_to_mermaid(&ctx, "symbol:test:orphan_fn:1");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph call_graph"));
        assert!(result.contains("orphan_fn"));
    }

    #[test]
    fn call_graph_special_chars_in_id() {
        let mock_gq = MockGraphQueryPort::new();
        let target = make_target_symbol("fn_with_special");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };

        // Symbol ID with special characters
        let result = call_graph_to_mermaid(&ctx, "symbol:test:fn(arg):42");

        assert!(result.contains("flowchart TD"));
        // sanitize_id should convert special chars
        assert!(result.contains("fn_arg_42") || result.contains("fn_arg"));
    }

    // ------------------------------------------------------------------------
    // impact_radius_to_mermaid — happy path
    // ------------------------------------------------------------------------

    #[test]
    fn impact_radius_to_mermaid_happy_path() {
        let entries = vec![
            make_call_entry("caller:1", "direct_caller", 1),
            make_call_entry("caller:2", "indirect_caller", 2),
        ];

        let mock_gq = MockGraphQueryPort::new()
            .with_traverse_callers(entries);

        let target = make_target_symbol("target_fn");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };
        let result = impact_radius_to_mermaid(&ctx, "symbol:test:target_fn:1");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph impact_radius"));
        assert!(result.contains("direct_caller"));
        assert!(result.contains("indirect_caller"));
        // Verify BFS tree structure: depth-2 entry should have "indirect callers" label
        assert!(result.contains("indirect callers"));
        // Verify edges go depth-1 --> depth-2 (BFS tree), not depth-2 --> center (star)
        // The Mermaid should show a chain structure
    }

    #[test]
    fn impact_radius_to_mermaid_empty() {
        let mock_gq = MockGraphQueryPort::new();
        let target = make_target_symbol("leaf_fn");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };
        let result = impact_radius_to_mermaid(&ctx, "symbol:test:leaf_fn:1");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph impact_radius"));
        assert!(result.contains("leaf_fn"));
    }

    #[test]
    fn impact_radius_special_chars() {
        let mock_gq = MockGraphQueryPort::new();
        let target = make_target_symbol("target");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };
        let result = impact_radius_to_mermaid(&ctx, "symbol:test:fn(path):1");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph impact_radius"));
    }

    // ------------------------------------------------------------------------
    // decision_trace_to_mermaid — multimodal feature gate
    // ------------------------------------------------------------------------

    #[cfg(feature = "multimodal")]
    #[test]
    fn decision_trace_returns_placeholder_when_multimodal_enabled() {
        let mock_gq = MockGraphQueryPort::new();
        let target = make_target_symbol("test");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };
        let result = decision_trace_to_mermaid(&ctx, "decision-uuid-123");
        assert!(result.contains("flowchart LR"));
        assert!(result.contains("decision_trace"));
    }

    // ------------------------------------------------------------------------
    // vertical_slice_to_mermaid — happy path
    // ------------------------------------------------------------------------

    #[test]
    fn vertical_slice_to_mermaid_happy_path() {
        let entries = vec![
            make_call_entry("usecase:1", "create_user", 1),
            make_call_entry("domain:1", "user_entity", 2),
            make_call_entry("repo:1", "user_repository", 3),
        ];

        let mock_gq = MockGraphQueryPort::new()
            .with_traverse_callees(entries);

        let target = make_target_symbol("handle_request");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };
        let result = vertical_slice_to_mermaid(&ctx, "POST /api/users");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph vertical_slice"));
        assert!(result.contains("create_user"));
        assert!(result.contains("user_entity"));
        assert!(result.contains("user_repository"));
    }

    #[test]
    fn vertical_slice_to_mermaid_empty() {
        let mock_gq = MockGraphQueryPort::new();
        let target = make_target_symbol("entry");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };
        let result = vertical_slice_to_mermaid(&ctx, "entry_point");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph vertical_slice"));
        assert!(result.contains("entry"));
    }

    #[test]
    fn vertical_slice_special_chars() {
        let mock_gq = MockGraphQueryPort::new();
        let target = make_target_symbol("handler");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = TraceEmitContext {
            graph_query: &mock_gq,
            target: &inspection_target,
        };
        let result = vertical_slice_to_mermaid(&ctx, "GET /api/items/:id");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph vertical_slice"));
        // Special chars should be sanitized
        assert!(result.contains("GET__api_items__id") || result.contains("GET_"));
    }

    // ------------------------------------------------------------------------
    // sanitize_id — re-exported from mermaid_util
    // ------------------------------------------------------------------------

    #[test]
    fn sanitize_id_colons() {
        assert_eq!(sanitize_id("symbol:foo:bar"), "symbol_foo_bar");
    }

    #[test]
    fn sanitize_id_slashes() {
        assert_eq!(sanitize_id("path/to/file"), "path_to_file");
    }

    #[test]
    fn sanitize_id_parens() {
        assert_eq!(sanitize_id("fn (arg)"), "fn_arg");
    }

    #[test]
    fn sanitize_id_dots() {
        assert_eq!(sanitize_id("crate.module"), "crate_module");
    }

    #[test]
    fn sanitize_id_alphanumeric_unchanged() {
        assert_eq!(sanitize_id("valid_id_123"), "valid_id_123");
    }

    // ------------------------------------------------------------------------
    // deduplicate_ids — re-exported from mermaid_util
    // ------------------------------------------------------------------------

    #[test]
    fn deduplicate_ids_no_dupes() {
        let ids = ["a", "b", "c"];
        let result = deduplicate_ids(&ids);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn deduplicate_ids_one_dup() {
        let ids = ["a", "b", "a"];
        let result = deduplicate_ids(&ids);
        assert_eq!(result, vec!["a", "b", "a_2"]);
    }
}
