//! petgraph-adapter feature stub.
//!
//! The actual `impl GraphBuilder for CallGraphProjection` lives in
//! `cognicode-core` (where `CallGraphProjection` is defined) — see
//! `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs`.
//!
//! This module exists only to validate that the `petgraph-adapter`
//! feature flag compiles when enabled. Removing it would break the
//! feature gate contract (cargo errors when a feature points at a
//! non-existent module). Future PETGRAPH-specific adapters (e.g.
//! a standalone `JsonGraph`-style adapter that wraps petgraph for
//! use without the cognicode-core domain layer) can live here.

#[cfg(feature = "petgraph-adapter")]
#[cfg(test)]
mod tests {
    /// Compile-time check: the petgraph-adapter feature flag gates
    /// this crate correctly. The `impl GraphBuilder for
    /// CallGraphProjection` lives in cognicode-core (the owner of
    /// the type), so this module only validates the feature wiring.
    #[test]
    fn petgraph_adapter_feature_compiles() {
        assert!(true);
    }
}
