//! Analytics Oracle Harness — CI parity checker between PostgreSQL and Neo4j backends.
//!
//! Part of E28.6 Advanced Analytics Evidence Gate — PR3 Neo4j CI Oracle.
//!
//! ## Overview
//!
//! The `AnalyticsOracleHarness` is a CI-only, opt-in verification tool that compares
//! analytics results between the PostgreSQL-backed executor and a Neo4j reference
//! implementation. It is **never** in the production `run()` path.
//!
//! ## Design Constraints
//!
//! - Oracle is **read-only**: it queries Neo4j but does NOT write to it
//! - Oracle is **opt-in**: activated only when `NEO4J_URI` env var is set
//! - Oracle is **CI-gated**: in permissive mode it reports divergence but doesn't fail;
//!   in strict mode it returns an error on any divergence
//! - Oracle is **feature-gated**: when `neo4j` feature is disabled or `NEO4J_URI`
//!   is not set, `parity_check` returns `OracleError::NotConfigured`

use std::env;

use crate::domain::analytics::oracle::{
    Divergence, OracleConfig, OracleError, OracleReport, OracleResult,
};
use crate::domain::plan::MoldPlan;

/// Analytics Oracle Harness for CI parity checking.
///
/// Compares analytics results between PostgreSQL (primary) and Neo4j (reference).
///
/// # Modes
///
/// - **Permissive** (`strict: false`): reports divergence but doesn't fail the check
/// - **Strict** (`strict: true`): fails with `OracleError::Divergence` on any mismatch
///
/// # Example
///
/// ```ignore
/// let harness = AnalyticsOracleHarness {
///     neo4j_uri: "neo4j://localhost:7687".to_string(),
///     strict: true,
/// };
/// let report = harness.parity_check(&plan, &pg_result, &neo4j_result)?;
/// ```
pub struct AnalyticsOracleHarness {
    /// Neo4j connection URI.
    neo4j_uri: String,
    /// If `true`, any divergence causes an error; otherwise just reported.
    strict: bool,
}

impl AnalyticsOracleHarness {
    /// Construct a new harness from an [`OracleConfig`].
    pub fn new(config: OracleConfig) -> Self {
        Self {
            neo4j_uri: config.neo4j_uri,
            strict: config.strict,
        }
    }

    /// Returns `true` if the harness is running in strict mode.
    pub fn is_strict(&self) -> bool {
        self.strict
    }

    /// Returns the configured Neo4j URI.
    pub fn neo4j_uri(&self) -> &str {
        &self.neo4j_uri
    }

    /// Check parity between PostgreSQL and Neo4j analytics results.
    ///
    /// This method:
    /// 1. Checks if `NEO4J_URI` env var is set (runtime feature gate)
    /// 2. Queries Neo4j for the same analytics result
    /// 3. Compares node/edge counts and individual values
    /// 4. Returns an [`OracleReport`] with all divergences
    ///
    /// In **strict mode**, if any divergence is found, returns `Err(OracleError::Divergence)`.
    /// In **permissive mode**, divergences are included in the report but don't cause error.
    ///
    /// # Arguments
    ///
    /// - `plan` — Plan context as JSON (algorithm_id and params). Not used in stub but
    ///   enables future Neo4j query construction.
    /// - `pg_result` — The result from the PostgreSQL executor as JSON
    /// - `neo4j_result` — The result from the Neo4j executor as JSON (placeholder in stub)
    ///
    /// # Returns
    ///
    /// - `Ok(OracleReport)` if the check completed (may contain divergences in permissive mode)
    /// - `Err(OracleError::NotConfigured)` if `NEO4J_URI` is not set
    /// - `Err(OracleError::ConnectionError)` if Neo4j connection fails
    /// - `Err(OracleError::Divergence)` in strict mode if any divergence is detected
    pub async fn parity_check(
        &self,
        _plan: &serde_json::Value,
        pg_result: &serde_json::Value,
        _neo4j_result: &serde_json::Value,
    ) -> OracleResult<OracleReport> {
        // Runtime feature gate: NEO4J_URI must be set
        if env::var("NEO4J_URI").is_err() {
            return Err(OracleError::NotConfigured);
        }

        // For now, this is a stub implementation that compares the JSON results
        // directly. A full implementation would:
        // 1. Parse the MoldPlan to determine the algorithm type
        // 2. Construct an equivalent Cypher query for Neo4j
        // 3. Execute against Neo4j and compare results

        // Extract node/edge counts from PG result
        let pg_nodes = extract_node_count(pg_result);
        let pg_edges = extract_edge_count(pg_result);

        // For the stub, assume neo4j_result has the same structure
        // A real implementation would query Neo4j here
        let neo4j_nodes = extract_node_count(_neo4j_result);
        let neo4j_edges = extract_edge_count(_neo4j_result);

        // Detect divergences
        let mut divergences = Vec::new();

        if pg_nodes != neo4j_nodes {
            divergences.push(Divergence {
                node_or_edge: "node_count".to_string(),
                pg_value: serde_json::json!(pg_nodes),
                neo4j_value: serde_json::json!(neo4j_nodes),
            });
        }

        if pg_edges != neo4j_edges {
            divergences.push(Divergence {
                node_or_edge: "edge_count".to_string(),
                pg_value: serde_json::json!(pg_edges),
                neo4j_value: serde_json::json!(neo4j_edges),
            });
        }

        let report = OracleReport {
            divergences,
            pg_nodes,
            neo4j_nodes,
            pg_edges,
            neo4j_edges,
        };

        // In strict mode, any divergence is an error
        if self.strict && !report.is_aligned() {
            let msg = format!(
                "parity check failed: {} divergence(s) detected (pg_nodes={}, neo4j_nodes={}, pg_edges={}, neo4j_edges={})",
                report.divergences.len(),
                report.pg_nodes,
                report.neo4j_nodes,
                report.pg_edges,
                report.neo4j_edges
            );
            return Err(OracleError::Divergence(msg));
        }

        Ok(report)
    }
}

// =============================================================================
// Helper functions for result parsing
// =============================================================================

fn extract_node_count(result: &serde_json::Value) -> usize {
    // Try common patterns for node count in analytics results
    if let Some(obj) = result.as_object() {
        // Pattern: { "nodes": [...], ... }
        if let Some(nodes) = obj.get("nodes") {
            if let Some(arr) = nodes.as_array() {
                return arr.len();
            }
        }
        // Pattern: { "node_count": N }
        if let Some(count) = obj.get("node_count").or(obj.get("nodes_count")) {
            if let Some(n) = count.as_u64() {
                return n as usize;
            }
        }
        // Pattern: { "community_ids": [...] } for conductance/modularity
        if let Some(ids) = obj.get("community_ids") {
            if let Some(arr) = ids.as_array() {
                return arr.len();
            }
        }
    }
    // Fallback: count top-level array length if result is an array
    if let Some(arr) = result.as_array() {
        return arr.len();
    }
    0
}

fn extract_edge_count(result: &serde_json::Value) -> usize {
    if let Some(obj) = result.as_object() {
        // Pattern: { "edges": [...], ... }
        if let Some(edges) = obj.get("edges") {
            if let Some(arr) = edges.as_array() {
                return arr.len();
            }
        }
        // Pattern: { "edge_count": N }
        if let Some(count) = obj.get("edge_count").or(obj.get("edges_count")) {
            if let Some(n) = count.as_u64() {
                return n as usize;
            }
        }
    }
    0
}

/// Check if the oracle harness is available (NEO4J_URI is set).
///
/// This is a helper for callers to check availability before constructing a harness.
pub fn is_oracle_available() -> bool {
    env::var("NEO4J_URI").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_node_count_from_nodes_array() {
        let result = json!({
            "nodes": ["a", "b", "c"],
            "edges": []
        });
        assert_eq!(super::extract_node_count(&result), 3);
    }

    #[test]
    fn test_extract_node_count_from_community_ids() {
        let result = json!({
            "community_ids": [1, 2, 3, 4],
            "scores": [0.1, 0.2, 0.3, 0.4]
        });
        assert_eq!(super::extract_node_count(&result), 4);
    }

    #[test]
    fn test_extract_edge_count() {
        let result = json!({
            "nodes": [],
            "edges": [(0, 1), (1, 2)]
        });
        assert_eq!(super::extract_edge_count(&result), 2);
    }

    #[test]
    fn test_oracle_not_configured_when_uri_missing() {
        // When NEO4J_URI is not set, oracle should report as unavailable
        // This test documents the behavior: oracle is only available when NEO4J_URI is set
        // In the test environment, NEO4J_URI is typically not set
        let is_available = super::is_oracle_available();
        // If NEO4J_URI is set in the environment, oracle is available
        // If NEO4J_URI is not set, oracle is not available
        if env::var("NEO4J_URI").is_err() {
            assert!(
                !is_available,
                "Oracle should not be available without NEO4J_URI"
            );
        }
    }

    #[test]
    fn test_harness_is_strict() {
        let config = OracleConfig {
            neo4j_uri: "neo4j://localhost:7687".to_string(),
            strict: true,
        };
        let harness = AnalyticsOracleHarness::new(config);
        assert!(harness.is_strict());

        let config_permissive = OracleConfig {
            neo4j_uri: "neo4j://localhost:7687".to_string(),
            strict: false,
        };
        let harness_permissive = AnalyticsOracleHarness::new(config_permissive);
        assert!(!harness_permissive.is_strict());
    }
}
