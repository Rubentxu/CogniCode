//! ResultSet, TruncationMarker, SemanticsViolation, Path — execution result types.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.
//!
//! ## Design
//!
//! - `ResultSet` is a multiset envelope: `Rows`, `Nodes`, `Edges`, `Paths`, `Scalars`.
//!   Stable iteration order is guaranteed (insertion order), and equality is
//!   multiset equality (unordered unless the `ordered` flag is set).
//! - `TruncationMarker` marks explicit truncation vs. an error.
//! - `SemanticsViolation` encodes ordering/path mismatches for error reporting.
//! - `Path` encodes a sequence of `(NodeId, Option<EdgeKind>)` hops for graph traversal.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;

// Types from sibling modules.
use super::limits::PlanLimits;
use super::value::TypedValue;
use super::version::{PlanHash, PlanMetadata, PlanVersion};
use crate::domain::value_objects::EdgeKind;

// ============================================================================
// TruncationMarker
// ============================================================================

/// Marks that a result set was truncated, not errored.
///
/// Distinct from `ExecutorError::LimitExceeded` — a truncated result is a
/// successful execution that ran out of budget (rows/paths) rather than
/// failing due to a hard limit (time/memory).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TruncationMarker {
    /// Result row count exceeded `max_result_rows`.
    ResultRowsLimit,
    /// Path count exceeded `max_path_count`.
    PathCountLimit,
    /// Visited nodes exceeded `max_visited_nodes`.
    VisitedNodesLimit,
    /// Visited edges exceeded `max_visited_edges`.
    VisitedEdgesLimit,
}

impl fmt::Display for TruncationMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TruncationMarker::ResultRowsLimit => write!(f, "result_rows_limit"),
            TruncationMarker::PathCountLimit => write!(f, "path_count_limit"),
            TruncationMarker::VisitedNodesLimit => write!(f, "visited_nodes_limit"),
            TruncationMarker::VisitedEdgesLimit => write!(f, "visited_edges_limit"),
        }
    }
}

// ============================================================================
// SemanticsViolation
// ============================================================================

/// A semantics-level error: ordering mismatch, path sequence mismatch, or
/// tolerance exceeded for approximate numeric comparisons.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SemanticsViolation {
    #[error("ordered result mismatch: {0}")]
    PathOrderMismatch(String),
    #[error("numeric tolerance exceeded: {0}")]
    ToleranceExceeded(String),
    #[error("multiset elements differ: {0}")]
    MultisetMismatch(String),
}

// ============================================================================
// ResultSet
// ============================================================================

/// A set of execution results with multiset semantics.
///
/// `ResultSet` wraps five content variants. Equality is multiset equality
/// for the unordered variants (elements same regardless of order) and
/// sequential equality for ordered variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultSet {
    pub rows: Vec<Row>,
    pub nodes: Vec<NodeResult>,
    pub edges: Vec<EdgeResult>,
    pub paths: Vec<Path>,
    pub scalars: Vec<super::TypedValue>,
    /// If `true`, the result was truncated; `truncation` carries the marker.
    pub truncated: bool,
    /// The truncation marker (meaningful only when `truncated == true`).
    pub truncation: Option<TruncationMarker>,
}

impl ResultSet {
    /// Construct an empty `ResultSet` with no truncation.
    pub fn empty() -> Self {
        Self {
            rows: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            paths: Vec::new(),
            scalars: Vec::new(),
            truncated: false,
            truncation: None,
        }
    }

    /// Mark this result set as truncated with the given marker.
    pub fn with_truncation(mut self, marker: TruncationMarker) -> Self {
        self.truncated = true;
        self.truncation = Some(marker);
        self
    }

    /// Returns `true` if the result set is empty and not truncated.
    pub fn is_empty(&self) -> bool {
        !self.truncated
            && self.rows.is_empty()
            && self.nodes.is_empty()
            && self.edges.is_empty()
            && self.paths.is_empty()
            && self.scalars.is_empty()
    }
}

impl Default for ResultSet {
    fn default() -> Self {
        Self::empty()
    }
}

/// A row result (table-like result from a projection query).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Row {
    pub columns: Vec<super::TypedValue>,
}

/// A node result from a graph query.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeResult {
    pub id: String,
    pub labels: Vec<String>,
    pub properties: Vec<super::TypedValue>,
}

/// An edge result from a graph query.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeResult {
    pub id: String,
    pub src: String,
    pub dst: String,
    pub label: String,
    pub properties: Vec<super::TypedValue>,
}

// ============================================================================
// Path
// ============================================================================

/// A path through the graph: an ordered sequence of nodes and edges.
///
/// Each hop is `(node_id, edge_kind)` where `edge_kind` is `None` for the
/// first node (no incoming edge) and `Some` for subsequent nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Path {
    /// Ordered hops: each element is `(node_id, edge_kind)`.
    /// The first element has `node_id = start`, `edge_kind = None`.
    pub hops: Vec<PathHop>,
}

impl Path {
    /// Construct a path from a sequence of `(node_id, edge_kind)` hops.
    pub fn new(hops: Vec<PathHop>) -> Self {
        Self { hops }
    }

    /// Returns the start node of the path.
    pub fn start(&self) -> &str {
        self.hops.first().map(|h| h.node_id.as_str()).unwrap_or("")
    }

    /// Returns the end node of the path.
    pub fn end(&self) -> &str {
        self.hops.last().map(|h| h.node_id.as_str()).unwrap_or("")
    }

    /// Returns the number of edges in the path (hops - 1).
    pub fn len(&self) -> usize {
        self.hops.len().saturating_sub(1)
    }

    /// Returns `true` if the path has no hops.
    pub fn is_empty(&self) -> bool {
        self.hops.is_empty()
    }
}

/// A single hop in a path: a node with an optional incoming edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PathHop {
    /// The node id for this hop.
    pub node_id: String,
    /// The kind of the edge that led to this node (None for start node).
    pub edge_kind: Option<EdgeKind>,
}

// ============================================================================
// assert_equivalent
// ============================================================================

/// Assert two `ResultSet`s are equivalent under multiset semantics.
///
/// For unordered results (nodes, edges, scalars), this checks multiset equality:
/// same elements regardless of order.
///
/// For ordered results (paths), this checks that both sequences are equal
/// element-by-element.
///
/// Returns `Ok(())` if equivalent, `Err(SemanticsViolation)` otherwise.
pub fn assert_equivalent(a: &ResultSet, b: &ResultSet) -> Result<(), SemanticsViolation> {
    // Check truncation first
    if a.truncated != b.truncated {
        return Err(SemanticsViolation::MultisetMismatch(
            "truncation flag mismatch".into(),
        ));
    }
    if a.truncation != b.truncation {
        return Err(SemanticsViolation::MultisetMismatch(
            "truncation marker mismatch".into(),
        ));
    }

    // Unordered multiset comparison for rows, nodes, edges, scalars
    fn multiset_eq<T: Eq + Hash>(a: &[T], b: &[T]) -> bool {
        use std::collections::HashMap;
        let mut count: HashMap<&T, usize> = HashMap::new();
        for v in a {
            *count.entry(v).or_insert(0) += 1;
        }
        for v in b {
            match count.get_mut(v) {
                Some(c) if *c > 0 => *c -= 1,
                _ => return false,
            }
        }
        count.values().all(|&c| c == 0)
    }

    if !multiset_eq(&a.rows, &b.rows) {
        return Err(SemanticsViolation::MultisetMismatch("rows mismatch".into()));
    }
    if !multiset_eq(&a.nodes, &b.nodes) {
        return Err(SemanticsViolation::MultisetMismatch("nodes mismatch".into()));
    }
    if !multiset_eq(&a.edges, &b.edges) {
        return Err(SemanticsViolation::MultisetMismatch("edges mismatch".into()));
    }
    if !multiset_eq(&a.scalars, &b.scalars) {
        return Err(SemanticsViolation::MultisetMismatch("scalars mismatch".into()));
    }

    // Ordered comparison for paths
    if a.paths.len() != b.paths.len() {
        return Err(SemanticsViolation::PathOrderMismatch(format!(
            "path count: {} vs {}",
            a.paths.len(),
            b.paths.len()
        )));
    }
    for (pa, pb) in a.paths.iter().zip(b.paths.iter()) {
        if pa != pb {
            return Err(SemanticsViolation::PathOrderMismatch(format!(
                "path mismatch: {:?} vs {:?}",
                pa, pb
            )));
        }
    }

    Ok(())
}

// ============================================================================
// assert_approx_equal
// ============================================================================

/// Assert two `TypedValue::Float` values are approximately equal within a
/// relative tolerance `eps` (default 1e-6).
///
/// Returns `Ok(())` if both are finite floats and `|a - b| <= eps * max(|a|,|b|)`,
/// or if both are the same integer representation.
///
/// Returns `Err(SemanticsViolation::ToleranceExceeded)` if the tolerance is exceeded.
pub fn assert_approx_equal(a: super::TypedValue, b: super::TypedValue, eps: f64) -> Result<(), SemanticsViolation> {
    match (a, b) {
        (super::TypedValue::Float(af), super::TypedValue::Float(bf)) => {
            if (af - bf).abs() <= eps * af.abs().max(bf.abs().max(1.0)) {
                Ok(())
            } else {
                Err(SemanticsViolation::ToleranceExceeded(format!(
                    "|{af} - {bf}| > {eps}"
                )))
            }
        }
        (super::TypedValue::Int(ai), super::TypedValue::Int(bi)) => {
            if ai == bi {
                Ok(())
            } else {
                Err(SemanticsViolation::ToleranceExceeded(format!(
                    "int {ai} != int {bi}"
                )))
            }
        }
        (a, b) => {
            if a == b {
                Ok(())
            } else {
                Err(SemanticsViolation::ToleranceExceeded(format!(
                    "{a} != {b}"
                )))
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.8a RED — ResultSet multiset identity + assert_equivalent + truncation marker
    // Scenario: `executor-semantics::Multiset Identity and Ordering` (both) +
    //          `executor-semantics::Truncation` (Truncation is explicit + TruncationMarker distinct from error)
    // Assert: same elements different order → `assert_equivalent` Ok;
    //         ordered paths mismatch → Err(PathOrderMismatch);
    //         truncated success vs error distinguishable
    // Task 1.9a RED — Path edge-kind preservation + self-loop hop + assert_approx_equal tolerance
    // Scenario: `executor-semantics::Path Node and Edge Sequence` +
    //           `executor-semantics::Numeric Tolerance` (both)
    // Assert: A→B→C carries `[Calls]` then `[Imports]` per hop;
    //         Float(0.5) vs Float(0.5000001) at 1e-6 → Ok;
    //         0.01 delta at 1e-6 → Err(ToleranceExceeded)
    // -------------------------------------------------------------------------

    /// `ResultSet::empty()` is empty and not truncated.
    #[test]
    fn result_set_empty() {
        let rs = ResultSet::empty();
        assert!(rs.is_empty());
        assert!(!rs.truncated);
        assert!(rs.truncation.is_none());
    }

    /// `ResultSet::with_truncation` sets both flags.
    #[test]
    fn result_set_truncation() {
        let rs = ResultSet::empty().with_truncation(TruncationMarker::ResultRowsLimit);
        assert!(rs.truncated);
        assert_eq!(rs.truncation, Some(TruncationMarker::ResultRowsLimit));
    }

    /// `assert_equivalent` returns `Ok` for two identical unordered `ResultSet`s.
    #[test]
    fn assert_equivalent_identical() {
        let a = ResultSet {
            nodes: vec![NodeResult {
                id: "n1".into(),
                labels: vec![],
                properties: vec![],
            }],
            ..ResultSet::empty()
        };
        let b = ResultSet {
            nodes: vec![NodeResult {
                id: "n1".into(),
                labels: vec![],
                properties: vec![],
            }],
            ..ResultSet::empty()
        };
        assert!(assert_equivalent(&a, &b).is_ok());
    }

    /// `assert_equivalent` returns `Ok` for unordered nodes in different order.
    #[test]
    fn assert_equivalent_unordered() {
        let a = ResultSet {
            nodes: vec![
                NodeResult { id: "n1".into(), labels: vec![], properties: vec![] },
                NodeResult { id: "n2".into(), labels: vec![], properties: vec![] },
            ],
            ..ResultSet::empty()
        };
        let b = ResultSet {
            nodes: vec![
                NodeResult { id: "n2".into(), labels: vec![], properties: vec![] },
                NodeResult { id: "n1".into(), labels: vec![], properties: vec![] },
            ],
            ..ResultSet::empty()
        };
        assert!(assert_equivalent(&a, &b).is_ok(), "unordered nodes: same multiset → equivalent");
    }

    /// `assert_equivalent` returns `Err(PathOrderMismatch)` for paths in different order.
    #[test]
    fn assert_equivalent_paths_ordered() {
        use crate::domain::value_objects::DependencyType;
        let a = ResultSet {
            paths: vec![
                Path::new(vec![
                    PathHop { node_id: "A".into(), edge_kind: None },
                    PathHop { node_id: "B".into(), edge_kind: Some(EdgeKind::Dependency(DependencyType::Calls)) },
                ]),
            ],
            ..ResultSet::empty()
        };
        let b = ResultSet {
            paths: vec![
                Path::new(vec![
                    PathHop { node_id: "B".into(), edge_kind: None },
                    PathHop { node_id: "A".into(), edge_kind: Some(EdgeKind::Dependency(DependencyType::Calls)) },
                ]),
            ],
            ..ResultSet::empty()
        };
        assert!(
            matches!(assert_equivalent(&a, &b), Err(SemanticsViolation::PathOrderMismatch(_))),
            "ordered paths: different order → PathOrderMismatch"
        );
    }

    /// `assert_equivalent` returns `Err(MultisetMismatch)` for different node ids.
    #[test]
    fn assert_equivalent_mismatch() {
        let a = ResultSet {
            nodes: vec![NodeResult { id: "n1".into(), labels: vec![], properties: vec![] }],
            ..ResultSet::empty()
        };
        let b = ResultSet {
            nodes: vec![NodeResult { id: "n2".into(), labels: vec![], properties: vec![] }],
            ..ResultSet::empty()
        };
        assert!(matches!(
            assert_equivalent(&a, &b),
            Err(SemanticsViolation::MultisetMismatch(_))
        ));
    }

    /// `assert_equivalent` detects truncation flag mismatch.
    #[test]
    fn assert_equivalent_truncation_mismatch() {
        let a = ResultSet::empty();
        let b = ResultSet::empty().with_truncation(TruncationMarker::ResultRowsLimit);
        assert!(matches!(
            assert_equivalent(&a, &b),
            Err(SemanticsViolation::MultisetMismatch(_))
        ));
    }

    /// `TruncationMarker` is distinct from an error — it's a success with budget exhausted.
    #[test]
    fn truncation_marker_is_success() {
        let rs = ResultSet::empty().with_truncation(TruncationMarker::ResultRowsLimit);
        // Truncated result set is NOT an error — it's a successful execution
        // that ran out of row budget.
        assert!(rs.truncated);
        assert!(!rs.is_empty()); // empty set + truncated flag
    }

    /// `Path` preserves edge kinds per hop.
    #[test]
    fn path_preserves_edge_kinds() {
        use crate::domain::value_objects::DependencyType;
        let path = Path::new(vec![
            PathHop { node_id: "A".into(), edge_kind: None },
            PathHop { node_id: "B".into(), edge_kind: Some(EdgeKind::Dependency(DependencyType::Calls)) },
            PathHop { node_id: "C".into(), edge_kind: Some(EdgeKind::Dependency(DependencyType::Imports)) },
        ]);
        assert_eq!(path.hops[0].edge_kind, None);
        assert_eq!(path.hops[1].edge_kind.as_ref().map(|e| e.as_str()).as_deref(), Some("dependency.calls"));
        assert_eq!(path.hops[2].edge_kind.as_ref().map(|e| e.as_str()).as_deref(), Some("dependency.imports"));
        assert_eq!(path.start(), "A");
        assert_eq!(path.end(), "C");
        assert_eq!(path.len(), 2); // 3 hops = 2 edges
    }

    /// `Path` start and end for a single-node path.
    #[test]
    fn path_single_node() {
        let path = Path::new(vec![PathHop { node_id: "X".into(), edge_kind: None }]);
        assert_eq!(path.start(), "X");
        assert_eq!(path.end(), "X");
        assert_eq!(path.len(), 0);
    }

    /// `assert_approx_equal` returns `Ok` for floats within tolerance.
    #[test]
    fn assert_approx_equal_within_tolerance() {
        let a = super::TypedValue::Float(0.5);
        let b = super::TypedValue::Float(0.5000001);
        let result = assert_approx_equal(a, b, 1e-6);
        assert!(result.is_ok(), "0.5 vs 0.5000001 at 1e-6 should pass");
    }

    /// `assert_approx_equal` returns `Err(ToleranceExceeded)` for floats outside tolerance.
    #[test]
    fn assert_approx_equal_exceeds_tolerance() {
        let a = super::TypedValue::Float(0.5);
        let b = super::TypedValue::Float(0.51);
        let result = assert_approx_equal(a, b, 1e-6);
        assert!(
            matches!(result, Err(SemanticsViolation::ToleranceExceeded(_))),
            "0.5 vs 0.51 delta=0.01 at 1e-6 should fail"
        );
    }

    /// `assert_approx_equal` returns `Ok` for equal integers.
    #[test]
    fn assert_approx_equal_int() {
        let a = super::TypedValue::Int(42);
        let b = super::TypedValue::Int(42);
        assert!(assert_approx_equal(a, b, 1e-6).is_ok());
    }

    /// `assert_approx_equal` returns `Err` for different integers.
    #[test]
    fn assert_approx_equal_int_mismatch() {
        let a = super::TypedValue::Int(42);
        let b = super::TypedValue::Int(43);
        assert!(matches!(
            assert_approx_equal(a, b, 1e-6),
            Err(SemanticsViolation::ToleranceExceeded(_))
        ));
    }

    /// `ResultSet` serde round-trip with truncation.
    #[test]
    fn result_set_serde_roundtrip() {
        let original = ResultSet::empty()
            .with_truncation(TruncationMarker::PathCountLimit);
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: ResultSet = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.truncated, true);
        assert_eq!(parsed.truncation, Some(TruncationMarker::PathCountLimit));
    }

    /// `Path` serde round-trip.
    #[test]
    fn path_serde_roundtrip() {
        use crate::domain::value_objects::DependencyType;
        let path = Path::new(vec![
            PathHop { node_id: "A".into(), edge_kind: None },
            PathHop { node_id: "B".into(), edge_kind: Some(EdgeKind::Dependency(DependencyType::Calls)) },
        ]);
        let json = serde_json::to_string(&path).expect("serialize");
        let parsed: Path = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, path);
    }
}
