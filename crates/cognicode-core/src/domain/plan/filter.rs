//! PlanFilter — typed filter predicates for MoldQL plan-level filtering.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR2 Plan Algebra.
//!
//! ## Design
//!
//! `PlanFilter` encodes filter predicates that apply at the plan level
//! (e.g., `WHERE confidence > 0.5`). These are distinct from graph traversal
//! predicates (`PathPredicate`) which apply during edge/node traversal.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::Hash;

// Sealed trait — implemented by all plan types to certify backend-neutrality.
use super::neutrality::Sealed;

// ============================================================================
// PlanFilterOp
// ============================================================================

/// Comparison operator for filter predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanFilterOp {
    Gt,
    Lt,
    Gte,
    Lte,
    Eq,
    Ne,
}

impl fmt::Display for PlanFilterOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanFilterOp::Gt => write!(f, ">"),
            PlanFilterOp::Lt => write!(f, "<"),
            PlanFilterOp::Gte => write!(f, ">="),
            PlanFilterOp::Lte => write!(f, "<="),
            PlanFilterOp::Eq => write!(f, "="),
            PlanFilterOp::Ne => write!(f, "!="),
        }
    }
}

impl Sealed for PlanFilterOp {}

// ============================================================================
// PlanFilter
// ============================================================================

/// A filter predicate applied at the plan level.
///
/// `PlanFilter` encodes structured filter conditions that are translated
/// to backend-specific predicates during plan lowering.
///
/// Note: `Eq` and `Hash` are NOT derived because `f64` does not implement `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlanFilter {
    /// Confidence score filter (e.g., `WHERE confidence > 0.5`).
    Confidence { op: PlanFilterOp, threshold: f64 },
    /// Provenance attribute filter (e.g., `WHERE provenance.lsp = "go_to_definition"`).
    Provenance { key: String, value: String },
}

// Manual `Eq` for PlanFilter — needed because f64 is not Eq, but Float
// values used in confidence thresholds are always finite (NaN is rejected at construction).
impl Eq for PlanFilter {}

// Manual `Hash` for PlanFilter — f64 implements Hash but not Eq.
impl Hash for PlanFilter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            PlanFilter::Confidence { op, threshold } => {
                op.hash(state);
                threshold.to_bits().hash(state);
            }
            PlanFilter::Provenance { key, value } => {
                key.hash(state);
                value.hash(state);
            }
        }
    }
}

impl fmt::Display for PlanFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanFilter::Confidence { op, threshold } => {
                write!(f, "confidence {op} {threshold}")
            }
            PlanFilter::Provenance { key, value } => {
                write!(f, "provenance.{key} = \"{value}\"")
            }
        }
    }
}

impl Sealed for PlanFilter {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 2.4a RED — PlanFilter enum (Confidence + Provenance)
    // Scenario: `explorerql-compilation::Filter Encoding on the Plan`
    // Assert: WHERE confidence > 0.5 → PlanFilter::Confidence { Gt, 0.5 };
    //         WHERE provenance.lsp = "go_to_definition" → PlanFilter::Provenance { "lsp", "go_to_definition" }
    // -------------------------------------------------------------------------

    /// `PlanFilter::Confidence` serde round-trip.
    #[test]
    fn plan_filter_confidence_roundtrip() {
        let filter = PlanFilter::Confidence { op: PlanFilterOp::Gt, threshold: 0.5 };
        let json = serde_json::to_string(&filter).expect("serialize");
        let parsed: PlanFilter = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, filter);
    }

    /// `PlanFilter::Confidence` with all operators.
    #[test]
    fn plan_filter_confidence_all_ops() {
        for op in [PlanFilterOp::Gt, PlanFilterOp::Lt, PlanFilterOp::Gte,
                   PlanFilterOp::Lte, PlanFilterOp::Eq, PlanFilterOp::Ne] {
            let filter = PlanFilter::Confidence { op, threshold: 0.7 };
            let json = serde_json::to_string(&filter).expect("serialize");
            let parsed: PlanFilter = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, filter);
        }
    }

    /// `PlanFilter::Provenance` serde round-trip.
    #[test]
    fn plan_filter_provenance_roundtrip() {
        let filter = PlanFilter::Provenance {
            key: "lsp".into(),
            value: "go_to_definition".into(),
        };
        let json = serde_json::to_string(&filter).expect("serialize");
        let parsed: PlanFilter = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, filter);
    }

    /// `PlanFilter::Display` shows readable filter representation.
    #[test]
    fn plan_filter_display() {
        let confidence = PlanFilter::Confidence { op: PlanFilterOp::Gt, threshold: 0.5 };
        assert!(confidence.to_string().contains("0.5"));

        let provenance = PlanFilter::Provenance {
            key: "lsp".into(),
            value: "go_to_definition".into(),
        };
        let display = provenance.to_string();
        assert!(display.contains("provenance"));
        assert!(display.contains("lsp"));
    }

    /// `PlanFilter::Confidence` threshold survives JSON round-trip with precision.
    #[test]
    fn plan_filter_confidence_precision() {
        let filter = PlanFilter::Confidence { op: PlanFilterOp::Gte, threshold: 0.95 };
        let json = serde_json::to_string(&filter).expect("serialize");
        let parsed: PlanFilter = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, filter);
        if let PlanFilter::Confidence { threshold, .. } = parsed {
            assert!((threshold - 0.95).abs() < 1e-10);
        } else {
            panic!("expected Confidence variant");
        }
    }

    /// `PlanFilterOp` Display returns operator symbol.
    #[test]
    fn plan_filter_op_display() {
        assert_eq!(PlanFilterOp::Gt.to_string(), ">");
        assert_eq!(PlanFilterOp::Lt.to_string(), "<");
        assert_eq!(PlanFilterOp::Eq.to_string(), "=");
        assert_eq!(PlanFilterOp::Ne.to_string(), "!=");
    }

    /// `PlanFilterOp` is exhaustive (all 6 operators).
    #[test]
    fn plan_filter_op_exhaustive() {
        let ops = [
            PlanFilterOp::Gt,
            PlanFilterOp::Lt,
            PlanFilterOp::Gte,
            PlanFilterOp::Lte,
            PlanFilterOp::Eq,
            PlanFilterOp::Ne,
        ];
        assert_eq!(ops.len(), 6);
    }

    /// `PlanFilter` is `Send + Sync + 'static`.
    #[test]
    fn plan_filter_send_sync_static() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_static<T: 'static>() {}
        assert_send::<PlanFilter>();
        assert_sync::<PlanFilter>();
        assert_static::<PlanFilter>();
    }

    /// `PlanFilter::Confidence` can be used in a `HashSet`.
    #[test]
    fn plan_filter_in_hashset() {
        use std::collections::HashSet;
        let mut set: HashSet<PlanFilter> = HashSet::new();
        set.insert(PlanFilter::Confidence { op: PlanFilterOp::Gt, threshold: 0.5 });
        set.insert(PlanFilter::Confidence { op: PlanFilterOp::Gt, threshold: 0.5 }); // duplicate
        assert_eq!(set.len(), 1);
    }
}
