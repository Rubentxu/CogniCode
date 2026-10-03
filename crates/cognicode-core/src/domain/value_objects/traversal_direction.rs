//! Which way to walk the call graph.
//!
//! This enum lived in `infrastructure::graph::on_demand_graph`, next to the
//! type that happens to consume it most, and `application` imported it from
//! there to pick a direction out of a query string. Which direction a caller
//! asked for is not an implementation detail of how the graph is stored — it
//! is the question itself, and it sits beside `NodeKind`, `EdgeKind` and
//! `SymbolKind` for the same reason.
//!
//! `infrastructure::graph` re-exports it, so the on-demand builder that
//! defined it kept the name it already used. One definition, no second enum.

/// Direction for call hierarchy traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    /// Traverse callees (outgoing edges - what does this symbol call)
    Callees,
    /// Traverse callers (incoming edges - what calls this symbol)
    Callers,
    /// Traverse both directions
    Both,
}
