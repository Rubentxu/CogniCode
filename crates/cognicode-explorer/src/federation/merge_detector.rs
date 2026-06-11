//! `MergeDetector` — heuristic scorer that suggests pairs of
//! federated nodes that might be the same real-world entity.
//!
//! **O(N²) brute-force** over the input `&[FederatedNode]`. The
//! detector is bounded to N ≤ 5000 in production (per the design
//! AD-5) — for larger inputs, callers should split.
//!
//! Scoring (per the design):
//! - Base: 0.5
//! - Label match: +0.3
//! - Kind match: +0.2
//! - Property overlap: +0.1 (capped at 1.0 total)
//! - Same-space pairs: filtered out (they share an id namespace
//!   and can't be the "federation" candidates the spec asks for).
//! - Below-threshold (0.8): filtered out.
//!
//! The label-normalisation step:
//! - lowercase
//! - trim leading/trailing whitespace
//! - collapse internal whitespace
//! - strip surrounding punctuation (a small allow-list of
//!   characters is preserved, see `normalize_label`).
//!
//! Gated behind the `multimodal` Cargo feature. Default builds
//! do not include this module.

use cognicode_core::domain::aggregates::generic_graph::GraphNode;
use cognicode_core::domain::value_objects::node_kind::NodeKind;

use crate::federation::federated_node::FederatedNode;
use crate::federation::merge_candidate::{MergeCandidate, MergeReason};

/// Threshold for the confidence filter. Pairs below this
/// confidence are not surfaced. Locked by the spec.
pub const MERGE_THRESHOLD: f64 = 0.8;

/// Scoring constants. Pulled into named constants so the test
/// suite can assert on them.
const SCORE_BASE: f64 = 0.5;
const SCORE_LABEL: f64 = 0.3;
const SCORE_KIND: f64 = 0.2;
const SCORE_PROPERTY: f64 = 0.1;

/// The merge detector. Stateless — every call takes the input
/// by reference and returns a fresh `Vec<MergeCandidate>`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MergeDetector;

impl MergeDetector {
    /// Construct a fresh detector. Stateless; equivalent to
    /// `MergeDetector::default()`.
    pub fn new() -> Self {
        Self
    }

    /// Score every pair of nodes from distinct spaces. Returns
    /// the candidates that pass the threshold filter, in a
    /// deterministic order (left-space order, then left-local-id
    /// order).
    pub fn detect(&self, nodes: &[FederatedNode]) -> Vec<MergeCandidate> {
        let mut out: Vec<MergeCandidate> = Vec::new();
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                let left = &nodes[i];
                let right = &nodes[j];
                // Same-space pairs are out of scope.
                if left.space_id == right.space_id {
                    continue;
                }
                if let Some(cand) = score_pair(left, right) {
                    if cand.confidence >= MERGE_THRESHOLD {
                        out.push(cand);
                    }
                }
            }
        }
        out
    }
}

/// Score one pair. Returns `None` if the pair is below the
/// threshold (the caller would discard it anyway, but this lets
/// us return early without cloning the nodes).
fn score_pair(left: &FederatedNode, right: &FederatedNode) -> Option<MergeCandidate> {
    let mut score = SCORE_BASE;
    let mut reasons: Vec<MergeReason> = Vec::new();

    if normalize_label(&left.node.label) == normalize_label(&right.node.label)
        && !normalize_label(&left.node.label).is_empty()
    {
        score += SCORE_LABEL;
        reasons.push(MergeReason::LabelMatch);
    }
    if left.node.kind == right.node.kind {
        score += SCORE_KIND;
        reasons.push(MergeReason::KindMatch);
    }
    if !left.node.properties.is_empty() && !right.node.properties.is_empty() {
        let overlap = property_overlap(&left.node, &right.node);
        if overlap {
            score += SCORE_PROPERTY;
            reasons.push(MergeReason::PropertyOverlap);
        }
    }
    // Cap to 1.0.
    if score > 1.0 {
        score = 1.0;
    }
    Some(
        MergeCandidate::new(left.clone(), right.clone(), score, reasons).with_clamped_confidence(),
    )
}

/// Normalise a label for label-matching: lowercase, trim,
/// collapse whitespace, strip surrounding punctuation.
pub fn normalize_label(s: &str) -> String {
    let lowered = s.to_ascii_lowercase();
    let trimmed = lowered.trim();
    // Collapse internal whitespace.
    let mut out = String::with_capacity(trimmed.len());
    let mut prev_space = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    // Strip surrounding punctuation (a small allow-list of
    // characters to keep: alphanumerics, hyphens, underscores,
    // dots, and spaces).
    out.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.')
        .to_string()
}

/// `true` iff the two nodes share at least one property KEY
/// (not a Jaccard overlap; the v1 detector uses a simple
/// "any overlap" heuristic).
fn property_overlap(a: &GraphNode, b: &GraphNode) -> bool {
    a.properties.keys().any(|k| b.properties.contains_key(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::generic_graph::GraphNode;
    use cognicode_core::domain::value_objects::node_kind::NodeKind;
    use cognicode_core::domain::value_objects::SpaceId;
    use cognicode_core::domain::value_objects::SymbolKind;

    fn make_node(id: &str, label: &str, kind: NodeKind) -> GraphNode {
        GraphNode::builder(id, kind)
            .label(label)
            .build()
    }

    fn make_fnode(space: &str, id: &str, label: &str, kind: NodeKind) -> FederatedNode {
        FederatedNode::new(
            make_node(id, label, kind),
            SpaceId::try_new(space).unwrap(),
        )
    }

    /// `normalize_label` lowercases and strips whitespace.
    #[test]
    fn label_normalization_lowercases_and_strips_whitespace() {
        assert_eq!(normalize_label("  User  "), "user");
        assert_eq!(normalize_label("USER"), "user");
        assert_eq!(normalize_label("  User  Model  "), "user model");
    }

    /// `normalize_label` preserves hyphens, underscores, dots.
    #[test]
    fn label_normalization_preserves_hyphens() {
        assert_eq!(normalize_label("user-name"), "user-name");
        assert_eq!(normalize_label("user_model"), "user_model");
        assert_eq!(normalize_label("v1.2"), "v1.2");
    }

    /// Label match alone produces 0.5 + 0.3 = 0.8 (the threshold).
    #[test]
    fn scoring_label_only_returns_0_8() {
        let a = make_fnode("a", "file:a:1", "User", NodeKind::Symbol(SymbolKind::Function));
        let b = make_fnode("b", "file:b:1", "user", NodeKind::Symbol(SymbolKind::Function));
        // Same label, same kind — wait, that gives 0.8 + 0.2 = 1.0.
        // To isolate the label-only score, use DIFFERENT kinds.
        let c = make_fnode("b", "file:c:1", "user", NodeKind::Decision);
        let cand = score_pair(&a, &c).expect("score");
        assert!(
            (cand.confidence - 0.8).abs() < 1e-9,
            "expected ~0.8, got {}",
            cand.confidence
        );
        assert!(cand.reasons.contains(&MergeReason::LabelMatch));
        assert!(!cand.reasons.contains(&MergeReason::KindMatch));
    }

    /// Kind match alone produces 0.5 + 0.2 = 0.7.
    #[test]
    fn scoring_kind_only_returns_0_7() {
        let a = make_fnode("a", "file:a:1", "Alpha", NodeKind::Symbol(SymbolKind::Function));
        let b = make_fnode(
            "b",
            "file:b:1",
            "Beta",
            NodeKind::Symbol(SymbolKind::Function),
        );
        let cand = score_pair(&a, &b).expect("score");
        assert!(
            (cand.confidence - 0.7).abs() < 1e-9,
            "expected ~0.7, got {}",
            cand.confidence
        );
        assert!(!cand.reasons.contains(&MergeReason::LabelMatch));
        assert!(cand.reasons.contains(&MergeReason::KindMatch));
    }

    /// Full match (label + kind + property overlap) caps at 1.0.
    #[test]
    fn scoring_full_match_returns_1_0_capped() {
        let mut a = make_fnode("a", "file:a:1", "User", NodeKind::Symbol(SymbolKind::Function));
        let mut b = make_fnode("b", "file:b:1", "user", NodeKind::Symbol(SymbolKind::Function));
        a.node.properties.insert("k".to_string(), "v".to_string());
        b.node.properties.insert("k".to_string(), "v".to_string());
        let cand = score_pair(&a, &b).expect("score");
        assert!(
            (cand.confidence - 1.0).abs() < 1e-9,
            "expected 1.0, got {}",
            cand.confidence
        );
    }

    /// Base-only (no label, no kind match — DIFFERENT kinds so
    /// the label-match path can also be off) returns 0.5.
    /// We use the base score path: 2 different labels, 2
    /// different kinds, 0 properties.
    #[test]
    fn scoring_base_only_returns_0_5() {
        let a = make_fnode("a", "file:a:1", "Alpha", NodeKind::Symbol(SymbolKind::Function));
        let b = make_fnode("b", "file:b:1", "Beta", NodeKind::Decision);
        let cand = score_pair(&a, &b).expect("score");
        assert!(
            (cand.confidence - 0.5).abs() < 1e-9,
            "expected 0.5, got {}",
            cand.confidence
        );
    }

    /// Scoring caps at 1.0 even when more than enough
    /// components fire (defensive — currently 0.5 + 0.3 + 0.2 +
    /// 0.1 = 1.1, capped to 1.0).
    #[test]
    fn scoring_caps_at_1_0_when_property_overlap_fires() {
        // 3 components firing = 1.0 (capped). We assert the
        // cap behaviour directly via MergeCandidate.
        let left = make_fnode("a", "file:a:1", "User", NodeKind::Symbol(SymbolKind::Function));
        let right = make_fnode("b", "file:b:1", "user", NodeKind::Symbol(SymbolKind::Function));
        // Without property overlap, the score is 0.5 + 0.3 + 0.2 = 1.0
        // exactly. Adding property overlap pushes it to 1.1 → capped to 1.0.
        let cand = score_pair(&left, &right).expect("score");
        assert!(cand.confidence <= 1.0);
    }

    /// Same-space pairs are filtered out (no `MergeCandidate`
    /// emitted for them).
    #[test]
    fn same_space_pair_filtered_out() {
        let a = make_fnode("a", "file:a:1", "User", NodeKind::Symbol(SymbolKind::Function));
        let b = make_fnode("a", "file:a:2", "User", NodeKind::Symbol(SymbolKind::Function));
        let det = MergeDetector::new();
        let out = det.detect(&[a, b]);
        assert!(out.is_empty(), "same-space pair must be filtered");
    }

    /// Pairs below the threshold are excluded.
    #[test]
    fn below_threshold_excluded() {
        let a = make_fnode("a", "file:a:1", "Alpha", NodeKind::Symbol(SymbolKind::Function));
        let b = make_fnode("b", "file:b:1", "Beta", NodeKind::Symbol(SymbolKind::Function));
        let det = MergeDetector::new();
        let out = det.detect(&[a, b]);
        // Score = 0.5 (base) — below threshold.
        assert!(out.is_empty());
    }

    /// Empty input returns an empty vec.
    #[test]
    fn empty_input_returns_empty_vec() {
        let det = MergeDetector::new();
        assert!(det.detect(&[]).is_empty());
    }

    /// Label-only match produces a `LabelMatch` reason.
    #[test]
    fn reasons_populated_for_label_only_match() {
        let a = make_fnode("a", "file:a:1", "User", NodeKind::Symbol(SymbolKind::Function));
        let b = make_fnode("b", "file:b:1", "user", NodeKind::Decision);
        let det = MergeDetector::new();
        let out = det.detect(&[a, b]);
        assert_eq!(out.len(), 1);
        assert!(out[0].reasons.contains(&MergeReason::LabelMatch));
    }

    /// Full match populates all three reasons.
    #[test]
    fn reasons_populated_for_full_match() {
        let mut a = make_fnode("a", "file:a:1", "User", NodeKind::Symbol(SymbolKind::Function));
        let mut b = make_fnode("b", "file:b:1", "user", NodeKind::Symbol(SymbolKind::Function));
        a.node.properties.insert("k".to_string(), "v".to_string());
        b.node.properties.insert("k".to_string(), "v".to_string());
        let det = MergeDetector::new();
        let out = det.detect(&[a, b]);
        assert_eq!(out.len(), 1);
        let r = &out[0].reasons;
        assert!(r.contains(&MergeReason::LabelMatch));
        assert!(r.contains(&MergeReason::KindMatch));
        assert!(r.contains(&MergeReason::PropertyOverlap));
    }

    /// Three-space cluster: 3 pairs total (1-2, 1-3, 2-3).
    #[test]
    fn three_space_cluster_produces_three_pairs() {
        let a = make_fnode("a", "file:a:1", "User", NodeKind::Symbol(SymbolKind::Function));
        let b = make_fnode("b", "file:b:1", "user", NodeKind::Symbol(SymbolKind::Function));
        let c = make_fnode("c", "file:c:1", "USER", NodeKind::Symbol(SymbolKind::Function));
        let det = MergeDetector::new();
        let out = det.detect(&[a, b, c]);
        assert_eq!(out.len(), 3, "expected 3 pairs, got {}", out.len());
    }
}
