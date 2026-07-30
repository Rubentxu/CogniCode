//! Analytics Oracle types for CI parity checking between PostgreSQL and Neo4j backends.
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
//! - Oracle is **feature-gated**: compiled out when `neo4j` feature is disabled

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configuration for the Analytics Oracle Harness.
///
/// Clone is cheap (String is cloned, but this is only constructed once at startup).
#[derive(Debug, Clone)]
pub struct OracleConfig {
    /// Neo4j connection URI (e.g., "neo4j://localhost:7687").
    pub neo4j_uri: String,
    /// If `true`, any divergence causes `parity_check` to return an error.
    /// If `false` (permissive), divergences are reported but don't cause failure.
    pub strict: bool,
}

/// Report produced by [`AnalyticsOracleHarness::parity_check`][super::AnalyticsOracleHarness::parity_check].
///
/// Summarizes the comparison between PostgreSQL and Neo4j analytics results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleReport {
    /// All detected divergences between PG and Neo4j results.
    pub divergences: Vec<Divergence>,
    /// Node count from the PostgreSQL executor.
    pub pg_nodes: usize,
    /// Node count from the Neo4j executor.
    pub neo4j_nodes: usize,
    /// Edge count from the PostgreSQL executor.
    pub pg_edges: usize,
    /// Edge count from the Neo4j executor.
    pub neo4j_edges: usize,
}

impl OracleReport {
    /// Returns `true` if there are no divergences.
    pub fn is_aligned(&self) -> bool {
        self.divergences.is_empty()
    }

    /// Returns `true` if node counts match between backends.
    pub fn nodes_match(&self) -> bool {
        self.pg_nodes == self.neo4j_nodes
    }

    /// Returns `true` if edge counts match between backends.
    pub fn edges_match(&self) -> bool {
        self.pg_edges == self.neo4j_edges
    }
}

/// A single divergence between PostgreSQL and Neo4j results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Divergence {
    /// Identifier of the node or edge where divergence was detected.
    pub node_or_edge: String,
    /// Value computed by the PostgreSQL executor.
    pub pg_value: Value,
    /// Value computed by the Neo4j executor.
    pub neo4j_value: Value,
}

/// Error returned when the Oracle fails to perform a parity check.
#[derive(Debug, thiserror::Error)]
pub enum OracleError {
    /// Neo4j URI was not configured (env var `NEO4J_URI` not set).
    #[error("NEO4J_URI is not configured — oracle harness not enabled")]
    NotConfigured,

    /// Failed to connect to Neo4j.
    #[error("neo4j connection error: {0}")]
    ConnectionError(String),

    /// Neo4j returned an unexpected result format.
    #[error("neo4j result parse error: {0}")]
    ResultParseError(String),

    /// Divergence detected in strict mode.
    #[error("parity check failed: {0}")]
    Divergence(String),
}

/// Result type alias for Oracle operations.
pub type OracleResult<T> = Result<T, OracleError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_report_is_aligned_empty_divergences() {
        let report = OracleReport {
            divergences: vec![],
            pg_nodes: 10,
            neo4j_nodes: 10,
            pg_edges: 5,
            neo4j_edges: 5,
        };
        assert!(report.is_aligned());
        assert!(report.nodes_match());
        assert!(report.edges_match());
    }

    #[test]
    fn oracle_report_has_divergence() {
        let report = OracleReport {
            divergences: vec![Divergence {
                node_or_edge: "node_1".to_string(),
                pg_value: serde_json::json!(1.0),
                neo4j_value: serde_json::json!(0.9),
            }],
            pg_nodes: 10,
            neo4j_nodes: 10,
            pg_edges: 5,
            neo4j_edges: 5,
        };
        assert!(!report.is_aligned());
        assert!(report.nodes_match());
        assert!(report.edges_match());
    }

    #[test]
    fn oracle_report_node_count_mismatch() {
        let report = OracleReport {
            divergences: vec![],
            pg_nodes: 10,
            neo4j_nodes: 9,
            pg_edges: 5,
            neo4j_edges: 5,
        };
        assert!(report.is_aligned());
        assert!(!report.nodes_match());
        assert!(report.edges_match());
    }
}
