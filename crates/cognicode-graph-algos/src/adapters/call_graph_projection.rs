//! petgraph adapter: implement `GraphBuilder` for `CallGraphProjection`.
//!
//! This module is **only compiled** when the `petgraph-adapter` feature
//! is enabled (default: off). Native consumers (`cognicode-core`) opt in;
//! WASM consumers never see this code.
//!
//! Full implementation lands in PR #2b (algorithm extraction). For PR #2a
//! we only establish the module and a smoke test.

#[cfg(test)]
mod tests {
    // Full impl in PR #2b. The smoke test lives in cognicode-core's
    // graph_analytics.rs until the delegation is wired.
    #[test]
    fn module_compiles() {
        // This test exists only to confirm `petgraph-adapter` feature
        // gates the module correctly. Remove in PR #2b once delegation
        // is wired and the test lives at the call site.
    }
}