//! `MergeCandidate` — a heuristic suggestion that two federated
//! nodes might refer to the same real-world entity.
//!
//! The detector is **suggest, never merge** (per design AD-4): it
//! returns a `confidence` score in `[0.0, 1.0]` and a list of
//! contributing `MergeReason`s. Downstream tools and humans
//! confirm.
//!
//! Gated behind the `multimodal` Cargo feature. Default builds do
//! not include this module.

use serde::{Deserialize, Serialize};

use crate::domain::federation::federated_node::FederatedNode;

/// A heuristic suggestion that two nodes might be the same
/// real-world entity. Constructed by the [`MergeDetector`].
///
/// The pair is unordered in the abstract, but the detector
/// produces candidates in a deterministic order (by the
/// registration order of the spaces, then by the local id
/// within each space).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergeCandidate {
    /// The "left" node of the pair. Detector-internal choice; the
    /// field is just `left` for the caller's convenience.
    pub left: FederatedNode,
    /// The "right" node of the pair.
    pub right: FederatedNode,
    /// The aggregate confidence in `[0.0, 1.0]`. Higher is
    /// stronger. The detector filters out pairs below 0.8.
    pub confidence: f64,
    /// The reasons that contributed to the confidence score.
    pub reasons: Vec<MergeReason>,
}

/// Why the detector thinks two nodes might be the same.
///
/// `#[non_exhaustive]` so future scorers (e.g. property overlap
/// with a Jaccard index) can be added without breaking callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MergeReason {
    /// The normalised labels are identical (case-insensitive,
    /// whitespace-collapsed, punctuation-stripped).
    LabelMatch,
    /// The `NodeKind`s are equal.
    KindMatch,
    /// The two nodes share at least one property key (currently
    /// the detector emits this when both nodes have ANY
    /// property — a simple heuristic, not a Jaccard overlap).
    PropertyOverlap,
}

impl MergeCandidate {
    /// Construct a candidate. The detector is the only caller;
    /// public for the public type but not a documented entry
    /// point.
    pub fn new(
        left: FederatedNode,
        right: FederatedNode,
        confidence: f64,
        reasons: Vec<MergeReason>,
    ) -> Self {
        Self {
            left,
            right,
            confidence,
            reasons,
        }
    }

    /// Clamp the confidence to `[0.0, 1.0]`. Used by the
    /// detector after summing scoring components.
    pub fn with_clamped_confidence(mut self) -> Self {
        if self.confidence > 1.0 {
            self.confidence = 1.0;
        } else if self.confidence < 0.0 {
            self.confidence = 0.0;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::generic_graph::GraphNode;
    use crate::domain::value_objects::SpaceId;
    use crate::domain::value_objects::node_kind::NodeKind;

    fn make_node(id: &str, label: &str) -> FederatedNode {
        FederatedNode::new(
            GraphNode::builder(
                id,
                NodeKind::Symbol(crate::domain::value_objects::SymbolKind::Function),
            )
            .label(label)
            .build(),
            SpaceId::try_new("a").unwrap(),
        )
    }

    /// `MergeCandidate::new` stores the fields.
    #[test]
    fn merge_candidate_constructs_with_left_right_confidence_reasons() {
        let left = make_node("file:a.rs:User:1", "User");
        let right = make_node("file:b.rs:User:1", "User");
        let cand = MergeCandidate::new(
            left.clone(),
            right.clone(),
            0.9,
            vec![MergeReason::LabelMatch, MergeReason::KindMatch],
        );
        assert_eq!(cand.left, left);
        assert_eq!(cand.right, right);
        assert_eq!(cand.confidence, 0.9);
        assert_eq!(cand.reasons.len(), 2);
    }

    /// `MergeReason` has 3 documented variants.
    #[test]
    fn merge_reason_label_match_kind_match_property_overlap_variants_construct() {
        let _ = MergeReason::LabelMatch;
        let _ = MergeReason::KindMatch;
        let _ = MergeReason::PropertyOverlap;
    }

    /// `with_clamped_confidence` clamps values above 1.0 to 1.0
    /// and below 0.0 to 0.0.
    #[test]
    fn merge_candidate_confidence_clamps_to_1_when_components_exceed() {
        let left = make_node("file:a.rs:U:1", "U");
        let right = make_node("file:b.rs:U:1", "U");
        let cand = MergeCandidate::new(left, right, 1.4, vec![MergeReason::LabelMatch])
            .with_clamped_confidence();
        assert_eq!(cand.confidence, 1.0);
    }
}
