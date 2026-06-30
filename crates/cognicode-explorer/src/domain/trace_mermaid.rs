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

use std::fmt::{self, Write};

use cognicode_core::domain::aggregates::{CallEntry, SymbolId};

use crate::dto::{InspectionTarget, ViewContext};
use crate::ports::symbol_repository::SymbolRepository;

// Re-export from shared mermaid_util
pub use super::mermaid_util::{deduplicate_ids, sanitize_id};

// ============================================================================
// call_graph_to_mermaid
// ============================================================================

/// Render a call graph as a Mermaid `flowchart TD` diagram.
///
/// `ctx` provides the [`SymbolRepository`] for resolving symbol IDs to display
/// names, and the [`GraphQueryPort`](cognicode_core::domain::traits::graph_query_port::GraphQueryPort)
/// for callers/callees data.
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
///
/// Returns a placeholder comment when no graph query is available.
pub fn call_graph_to_mermaid(ctx: &ViewContext, symbol: &str) -> String {
    let Some(graph_query) = ctx.graph_query else {
        return "// graph_query not available".to_string();
    };

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
/// `ctx` provides the [`SymbolRepository`] for resolving symbol IDs to display
/// names, and the [`GraphQueryPort`](cognicode_core::domain::traits::graph_query_port::GraphQueryPort)
/// for BFS traversal.
///
/// `symbol` is the root symbol's string identifier.
///
/// Uses `traverse_callers` (reverse BFS) to find all callers up to depth 3.
pub fn impact_radius_to_mermaid(ctx: &ViewContext, symbol: &str) -> String {
    let Some(graph_query) = ctx.graph_query else {
        return "// graph_query not available".to_string();
    };

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

    // Deduplicate and render reachable nodes
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    seen.insert(center_id.clone());

    // Group entries by depth (depth is implicit in the BFS ordering)
    // We render level-by-level: depth 1 callers, then depth 2, etc.
    let mut current_depth_nodes: Vec<&cognicode_core::domain::aggregates::CallEntry> = Vec::new();
    let mut next_depth_nodes: Vec<&cognicode_core::domain::aggregates::CallEntry> = Vec::new();
    let mut current_depth = 1;

    // First pass: collect depth-1 callers
    for entry in &entries {
        if entry.depth == 1 {
            current_depth_nodes.push(entry);
        }
    }

    // Render depth 1 (direct callers) on the left
    for entry in &current_depth_nodes {
        let entry_id = sanitize_id(entry.symbol_id.as_str());
        if seen.insert(entry_id.clone()) {
            lines.push(format!("        {}[{}]", entry_id, entry.symbol_name));
            lines.push(format!("        {} --> {}", entry_id, center_id));
        }
    }

    // For higher depths, we process remaining entries
    // Note: traverse_callers returns CallEntry with depth information
    let mut max_depth_shown = 1;
    for entry in &entries {
        if entry.depth > max_depth_shown && entry.depth <= 3 {
            max_depth_shown = entry.depth;
            let entry_id = sanitize_id(entry.symbol_id.as_str());
            if seen.insert(entry_id.clone()) {
                lines.push(format!("        {}[{}]", entry_id, entry.symbol_name));
                lines.push(format!("        {} -.-> {}", entry_id, center_id));
            }
        }
    }

    lines.push("    end".to_string());
    lines.join("\n")
}

// ============================================================================
// decision_trace_to_mermaid
// ============================================================================

/// Render a decision trace as a Mermaid `flowchart LR` diagram.
///
/// `ctx` provides ADR/graph data for the decision trace.
///
/// `decision_id` is the decision UUID.
///
/// Uses `flowchart LR` (left-right layout) for horizontal decision traces.
///
/// ## Feature gate
///
/// Requires the `multimodal` feature to be enabled.
#[cfg(feature = "multimodal")]
pub fn decision_trace_to_mermaid(ctx: &ViewContext, decision_id: &str) -> String {
    // TODO: When DecisionTrace executor is implemented, extract data from ctx
    // For now, return a placeholder that shows the expected structure
    format!(
        "flowchart LR\n    subgraph decision_trace[\"decision_trace: {}\"]\n        direction LR\n        subgraph adr[\"ADR\"]\n            A[ADR Metadata]\n        end\n        subgraph code[\"Code\"]\n            C[Implementation]\n        end\n        subgraph evidence[\"Evidence\"]\n            E[Supporting Evidence]\n        end\n        A --> C\n        C --> E\n    end",
        sanitize_id(decision_id)
    )
}

#[cfg(not(feature = "multimodal"))]
pub fn decision_trace_to_mermaid(_ctx: &ViewContext, _decision_id: &str) -> String {
    "// decision_trace_to_mermaid requires the `multimodal` feature".to_string()
}

// ============================================================================
// vertical_slice_to_mermaid
// ============================================================================

/// Render a vertical slice (full entry-point trace) as a Mermaid `flowchart TD`.
///
/// `ctx` provides the data for building the vertical slice.
///
/// `entry_point` is the entry point identifier (HTTP route, CLI command, etc.).
///
/// Full vertical trace: HTTP → use case → domain → repo → DB
pub fn vertical_slice_to_mermaid(ctx: &ViewContext, entry_point: &str) -> String {
    // Vertical slice traces the full path from entry point through all layers.
    // When we have the target symbol, we can trace its call graph.
    let Some(graph_query) = ctx.graph_query else {
        return format!(
            "flowchart TD\n    subgraph vertical_slice[\"vertical_slice: {}\"]\n        direction TD\n        {}[{}]\n    end",
            sanitize_id(entry_point),
            sanitize_id(entry_point),
            sanitize_id(entry_point)
        );
    };

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
    let mut depth_nodes: std::collections::HashMap<u8, Vec<&cognicode_core::domain::aggregates::CallEntry>> =
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

    use crate::dto::{InspectionTarget, ViewContext};
    use crate::error::ExplorerResult;
    use crate::ports::symbol_repository::{GraphStats, ResolvedSymbol};
    use crate::ports::source_reader::SourceReader;
    use cognicode_core::domain::aggregates::CallEntry;
    use cognicode_core::domain::traits::graph_query_port::GraphQueryPort;
    use cognicode_core::domain::value_objects::SymbolKind;
    use std::sync::Arc;

    // ------------------------------------------------------------------------
    // Mock implementations for testing
    // ------------------------------------------------------------------------

    struct NoopSourceReader;

    impl SourceReader for NoopSourceReader {
        fn read_source(&self, _file: &str) -> ExplorerResult<String> {
            Ok(String::new())
        }
        fn read_lines(&self, _file: &str, _start: u32, _end: u32) -> ExplorerResult<Vec<(u32, String)>> {
            Ok(vec![])
        }
    }

    struct MockSymbolRepository;

    impl SymbolRepository for MockSymbolRepository {
        fn resolve(&self, _id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(None)
        }
        fn find_symbols_by_name(&self, _name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(vec![])
        }
        fn find_symbols_by_file(&self, _file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(vec![])
        }
        fn module_list(&self) -> Vec<String> {
            vec![]
        }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(vec![])
        }
        fn graph_stats(&self) -> GraphStats {
            GraphStats {
                symbol_count: 0,
                relation_count: 0,
            }
        }
    }

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
        let ctx = ViewContext {
            target: &inspection_target,
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: Some(&mock_gq),
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
        let ctx = ViewContext {
            target: &inspection_target,
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: Some(&mock_gq),
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
        let ctx = ViewContext {
            target: &inspection_target,
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: Some(&mock_gq),
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
        let ctx = ViewContext {
            target: &inspection_target,
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: Some(&mock_gq),
        };

        let result = impact_radius_to_mermaid(&ctx, "symbol:test:target_fn:1");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph impact_radius"));
        assert!(result.contains("direct_caller"));
        assert!(result.contains("indirect_caller"));
    }

    #[test]
    fn impact_radius_to_mermaid_empty() {
        let mock_gq = MockGraphQueryPort::new();
        let target = make_target_symbol("leaf_fn");
        let inspection_target = InspectionTarget::Symbol(target);
        let ctx = ViewContext {
            target: &inspection_target,
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: Some(&mock_gq),
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
        let ctx = ViewContext {
            target: &inspection_target,
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: Some(&mock_gq),
        };

        let result = impact_radius_to_mermaid(&ctx, "symbol:test:fn(path):1");

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("subgraph impact_radius"));
    }

    // ------------------------------------------------------------------------
    // decision_trace_to_mermaid — multimodal feature gate
    // ------------------------------------------------------------------------

    #[test]
    fn decision_trace_requires_multimodal_feature() {
        let ctx = ViewContext {
            target: &InspectionTarget::Symbol(make_target_symbol("test")),
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: None,
        };

        let result = decision_trace_to_mermaid(&ctx, "decision-uuid-123");

        #[cfg(feature = "multimodal")]
        {
            assert!(result.contains("flowchart LR"));
            assert!(result.contains("decision_trace"));
        }

        #[cfg(not(feature = "multimodal"))]
        {
            // The non-multimodal version returns a message indicating the feature is required
            assert!(result.contains("multimodal"));
            assert!(result.contains("requires"));
        }
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
        let ctx = ViewContext {
            target: &inspection_target,
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: Some(&mock_gq),
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
        let ctx = ViewContext {
            target: &inspection_target,
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: Some(&mock_gq),
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
        let ctx = ViewContext {
            target: &inspection_target,
            repo: &MockSymbolRepository,
            reader: &NoopSourceReader,
            quality: None,
            graph_query: Some(&mock_gq),
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
