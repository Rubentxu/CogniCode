//! Corroboration scoring — domain service for computing multi-source
//! corroboration evidence in the multimodal graph.
//!
//! The scoring model allocates **one score per target node** by
//! bucket-maxing across provenances: for each distinct [`Provenance`]
//! that contributes edges to the same target, we take the **maximum**
//! edge score from that provenance. The final score is the **sum** of
//! the bucket maxima, clamped to `[0.0, 1.0]`.
//!
//! This design prevents a single source (e.g. one doc with many links)
//! from inflating the score while still rewarding **independent**
//! sources that agree on the same target.
//!
//! Usage:
//! ```rust,ignore
//! let scores = score_subgraph(&graph_nodes, &graph_edges);
//! ```

use std::collections::HashMap;

use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
use crate::domain::value_objects::provenance::Provenance;

/// Weight for a [`Provenance`] variant.
///
/// Manual entries get the highest weight (1.0), reflecting direct
/// human curation. Extracted (0.9) and Tested (0.85) are slightly
/// lower — they are trustworthy but not curated. Inferred (0.5)
/// and Ambiguous (0.3) represent heuristic / unresolved sources.
///
/// The match is **exhaustive** — adding a new `Provenance` variant
/// is a compile-error, forcing the author to assign a weight.
pub fn provenance_weight(p: &Provenance) -> f64 {
    match p {
        Provenance::Manual => 1.0,
        Provenance::Extracted => 0.9,
        Provenance::Tested => 0.85,
        Provenance::Inferred => 0.5,
        Provenance::Ambiguous => 0.3,
    }
}

/// Score for a single edge: `weight × confidence`, clamped to `[0.0, 1.0]`.
pub fn edge_score(edge: &GraphEdge) -> f64 {
    (provenance_weight(&edge.provenance) * edge.confidence).clamp(0.0, 1.0)
}

/// Score for a target node by bucket-maxing across provenances.
///
/// For each distinct `Provenance` contributing edges to `target`, the
/// **maximum** edge score within that bucket is retained. The per-bucket
/// maxima are **summed** and clamped to `[0.0, 1.0]`.
///
/// Returns `0.0` when the target has no incoming edges in the slice.
pub fn target_score(target: &NodeId, edges: &[GraphEdge]) -> f64 {
    let mut buckets: HashMap<Provenance, f64> = HashMap::new();
    for e in edges.iter().filter(|e| &e.target == target) {
        let s = edge_score(e);
        buckets
            .entry(e.provenance)
            .and_modify(|cur| {
                if s > *cur {
                    *cur = s;
                }
            })
            .or_insert(s);
    }
    buckets.values().sum::<f64>().clamp(0.0, 1.0)
}

/// Compute corroboration scores for the subgraph.
///
/// Returns a `HashMap` that contains BOTH per-edge and per-target
/// scores so a single API call can serve both consumers:
///
/// * **Per-edge entries** are keyed by `"source->target"` (the
///   composite key the front-end adapter reconstructs from DTO
///   `GraphEdge.source` and `GraphEdge.target`) with each edge's
///   `edge_score` value. These are kept for backward compatibility
///   with existing front-end code that visualises edges.
///
/// * **Per-target entries** are keyed by `"any->target"` and
///   contain the bucket-maxed [`target_score`] — i.e. the
///   corroboration score of a node, computed by summing the
///   per-provenance maxima of all incoming edges (clamped to
///   `[0.0, 1.0]`). This is the metric the corroboration spec
///   actually wants: "how well-attested is this target node?".
///
/// The result is deterministic for identical inputs: edges are
/// traversed in order and the score formula is pure.
pub fn score_subgraph(
    _nodes: &[GraphNode],
    edges: &[GraphEdge],
) -> HashMap<String, f64> {
    let mut out: HashMap<String, f64> = HashMap::new();
    // 1) Per-edge scores (backward-compat keys).
    for e in edges {
        out.insert(
            format!("{}->{}", e.source.0, e.target.0),
            edge_score(e),
        );
    }
    // 2) Per-target scores (new): bucket-max by target.
    // Collect distinct targets first so the map is deterministic
    // regardless of HashMap iteration order.
    let mut distinct_targets: Vec<&NodeId> =
        edges.iter().map(|e| &e.target).collect();
    distinct_targets.sort_by(|a, b| a.0.cmp(&b.0));
    distinct_targets.dedup();
    for target in distinct_targets {
        out.insert(format!("any->{}", target.0), target_score(target, edges));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::generic_graph::GraphEdge;
    use crate::domain::value_objects::edge_kind::EdgeKind;
    use crate::domain::value_objects::dependency_type::DependencyType;
    use std::collections::HashMap;

    // ---- helpers ----

    fn nid(s: &str) -> NodeId {
        NodeId::new(s)
    }

    fn make_edge(
        _id: &str,
        source: &str,
        target: &str,
        prov: Provenance,
        confidence: f64,
    ) -> GraphEdge {
        GraphEdge {
            source: NodeId::new(source),
            target: NodeId::new(target),
            kind: EdgeKind::Dependency(DependencyType::Calls),
            provenance: prov,
            confidence,
            metadata: HashMap::new(),
        }
    }

    // ========================================================================
    // 1.1 — provenance_weight exhaustive match
    // ========================================================================

    /// Manual gets weight 1.0 (highest — human-curated).
    #[test]
    fn provenance_weight_manual() {
        assert!((provenance_weight(&Provenance::Manual) - 1.0).abs() < f64::EPSILON);
    }

    /// Extracted gets weight 0.9.
    #[test]
    fn provenance_weight_extracted() {
        assert!((provenance_weight(&Provenance::Extracted) - 0.9).abs() < f64::EPSILON);
    }

    /// Tested gets weight 0.85.
    #[test]
    fn provenance_weight_tested() {
        assert!((provenance_weight(&Provenance::Tested) - 0.85).abs() < f64::EPSILON);
    }

    /// Inferred gets weight 0.5.
    #[test]
    fn provenance_weight_inferred() {
        assert!((provenance_weight(&Provenance::Inferred) - 0.5).abs() < f64::EPSILON);
    }

    /// Ambiguous gets weight 0.3.
    #[test]
    fn provenance_weight_ambiguous() {
        assert!((provenance_weight(&Provenance::Ambiguous) - 0.3).abs() < f64::EPSILON);
    }

    // ========================================================================
    // 1.3 — edge_score
    // ========================================================================

    /// Manual 0.7 → 1.0 * 0.7 = 0.7
    #[test]
    fn edge_score_manual_07() {
        let e = make_edge("e1", "A", "B", Provenance::Manual, 0.7);
        assert!((edge_score(&e) - 0.7).abs() < f64::EPSILON);
    }

    /// Manual 0.5, confidence 0.4 (should still be 0.4)
    #[test]
    fn edge_score_manual_confidence_04() {
        let e = make_edge("e2", "A", "B", Provenance::Manual, 0.4);
        assert!((edge_score(&e) - 0.4).abs() < f64::EPSILON);
    }

    /// confidence 0.0 → 0.0 regardless of weight
    #[test]
    fn edge_score_zero_confidence() {
        let e = make_edge("e3", "A", "B", Provenance::Manual, 0.0);
        assert!((edge_score(&e)).abs() < f64::EPSILON);
    }

    /// Inferred 1.0 → 0.5 * 1.0 = 0.5
    #[test]
    fn edge_score_inferred_10() {
        let e = make_edge("e4", "A", "B", Provenance::Inferred, 1.0);
        assert!((edge_score(&e) - 0.5).abs() < f64::EPSILON);
    }

    // ========================================================================
    // 1.5 — target_score
    // ========================================================================

    /// 2 Manual edges (0.8, 0.9) to same target → bucket-max = 0.9
    /// (same provenance bucket, only max counts)
    #[test]
    fn target_score_two_manual_same_target() {
        let edges = vec![
            make_edge("e1", "A", "B", Provenance::Manual, 0.8),
            make_edge("e2", "A", "B", Provenance::Manual, 0.9),
        ];
        let score = target_score(&nid("B"), &edges);
        assert!((score - 0.9).abs() < f64::EPSILON);
    }

    /// 2 Extracted edges (0.5, 0.95) → bucket-max edge score 0.855
    #[test]
    fn target_score_two_extracted_same_target() {
        let edges = vec![
            make_edge("e1", "A", "B", Provenance::Extracted, 0.5),
            make_edge("e2", "A", "B", Provenance::Extracted, 0.95),
        ];
        let score = target_score(&nid("B"), &edges);
        assert!((score - 0.855).abs() < f64::EPSILON);
    }

    /// Mixed provenances: Manual 0.5, Extracted 0.6, Inferred 0.7
    #[test]
    fn target_score_mixed_provenances() {
        let edges = vec![
            make_edge("e1", "A", "B", Provenance::Manual, 0.5),
            make_edge("e2", "A", "B", Provenance::Extracted, 0.6),
            make_edge("e3", "A", "B", Provenance::Inferred, 0.7),
        ];
        let score = target_score(&nid("B"), &edges);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    /// Empty edges → 0.0
    #[test]
    fn target_score_empty_edges() {
        let score = target_score(&nid("B"), &[]);
        assert!((score).abs() < f64::EPSILON);
    }

    /// Target not in any edge → 0.0
    #[test]
    fn target_score_missing_target() {
        let edges = vec![make_edge("e1", "A", "C", Provenance::Manual, 0.9)];
        let score = target_score(&nid("B"), &edges);
        assert!((score).abs() < f64::EPSILON);
    }

    // ========================================================================
    // 1.7 — score_subgraph
    // ========================================================================

    /// 4 edges with known scores → map has 4 per-edge + 3 per-target
    /// = 7 entries, with correct values for both kinds.
    #[test]
    fn score_subgraph_four_edges() {
        let edges = vec![
            make_edge("e1", "A", "B", Provenance::Manual, 1.0),
            make_edge("e2", "A", "C", Provenance::Extracted, 0.8),
            make_edge("e3", "B", "D", Provenance::Tested, 0.9),
            make_edge("e4", "C", "D", Provenance::Inferred, 1.0),
        ];
        let nodes = vec![];
        let scores = score_subgraph(&nodes, &edges);
        // 4 per-edge + 3 per-target (B, C, D).
        assert_eq!(scores.len(), 7);
        // --- per-edge scores (backward-compat) ---
        // A->B: Manual 1.0 → 1.0 * 1.0 = 1.0
        assert!((scores["A->B"] - 1.0).abs() < f64::EPSILON);
        // A->C: Extracted 0.8 → 0.9 * 0.8 = 0.72
        assert!((scores["A->C"] - 0.72).abs() < f64::EPSILON);
        // B->D: Tested 0.9 → 0.85 * 0.9 = 0.765
        assert!((scores["B->D"] - 0.765).abs() < f64::EPSILON);
        // C->D: Inferred 1.0 → 0.5 * 1.0 = 0.5
        assert!((scores["C->D"] - 0.5).abs() < f64::EPSILON);
        // --- per-target scores (new) ---
        // any->B: only one edge (Manual 1.0) → 1.0
        assert!((scores["any->B"] - 1.0).abs() < f64::EPSILON);
        // any->C: only one edge (Extracted 0.8) → 0.72
        assert!((scores["any->C"] - 0.72).abs() < f64::EPSILON);
        // any->D: two edges (Tested 0.9 → 0.765, Inferred 1.0 → 0.5).
        // Bucket-max per provenance, then sum: 0.765 + 0.5 = 1.265
        // clamped to 1.0.
        assert!((scores["any->D"] - 1.0).abs() < f64::EPSILON);
    }

    /// Empty edges → empty map (no per-edge, no per-target).
    #[test]
    fn score_subgraph_empty_edges() {
        let scores = score_subgraph(&[], &[]);
        assert!(scores.is_empty());
    }

    /// Deterministic — calling twice on same input yields equal maps
    #[test]
    fn score_subgraph_deterministic() {
        let edges = vec![
            make_edge("e1", "A", "B", Provenance::Manual, 1.0),
            make_edge("e2", "A", "C", Provenance::Extracted, 0.8),
        ];
        let nodes = vec![];
        let a = score_subgraph(&nodes, &edges);
        let b = score_subgraph(&nodes, &edges);
        assert_eq!(a, b);
    }

    /// Per-target bucket-max: when two edges share a target AND
    /// the same provenance, only the max counts toward the
    /// provenance bucket. (The per-edge key collides for parallel
    /// edges — the LAST edge's score wins, mirroring the previous
    /// `HashMap` behaviour. The per-target key is the new
    /// bucket-maxed value.)
    #[test]
    fn score_subgraph_per_target_bucket_max() {
        let edges = vec![
            make_edge("e1", "A", "B", Provenance::Manual, 0.4),
            make_edge("e2", "A", "B", Provenance::Manual, 0.9),
            make_edge("e3", "A", "B", Provenance::Inferred, 0.6),
        ];
        let scores = score_subgraph(&[], &edges);
        // 1 per-edge key (all three edges share "A->B"; last
        // wins) + 1 per-target key (B) = 2 entries.
        assert_eq!(scores.len(), 2);
        // Per-edge "A->B": last edge (Inferred 0.6) wins =
        // 0.5 * 0.6 = 0.3.
        assert!((scores["A->B"] - 0.3).abs() < f64::EPSILON);
        // Per-target any->B: bucket max Manual = 0.9, bucket max
        // Inferred = 0.3 → 0.9 + 0.3 = 1.2 → clamped to 1.0.
        assert!((scores["any->B"] - 1.0).abs() < f64::EPSILON);
    }
}
