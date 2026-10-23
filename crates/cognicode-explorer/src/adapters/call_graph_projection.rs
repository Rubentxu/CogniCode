//! Adapter that builds the read-side projection the graph services read from.
//!
//! The explorer is a composition layer: it decides which implementation of
//! the projection port it uses, and hands the result to the services that
//! know how to analyse it. One call site per query, one place that names
//! the adapter.

use std::sync::Arc;

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_core::domain::ports::call_graph_projection::CallGraphProjectionPort;

/// Build the read-side projection over `graph`.
pub fn project(graph: &CallGraph) -> Arc<dyn CallGraphProjectionPort> {
    cognicode_core::infrastructure::graph::project_call_graph(graph)
}
