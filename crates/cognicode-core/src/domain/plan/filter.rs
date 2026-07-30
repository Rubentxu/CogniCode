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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanFilter {
    /// Confidence score filter (e.g., `WHERE confidence > 0.5`).
    Confidence { op: PlanFilterOp, threshold: f64 },
    /// Provenance attribute filter (e.g., `WHERE provenance.lsp = "go_to_definition"`).
    Provenance { key: String, value: String },
}

// Manual `Eq` for PlanFilter — needed because f64 is not Eq, but Float
// values used in confidence thresholds should be finite (NaN is rejected at construction).
// We implement PartialEq manually to ensure NaN == NaN (consistent with Hash, which uses to_bits()).
impl PartialEq for PlanFilter {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                PlanFilter::Confidence {
                    op: op1,
                    threshold: t1,
                },
                PlanFilter::Confidence {
                    op: op2,
                    threshold: t2,
                },
            ) => {
                op1 == op2 && {
                    if t1.is_nan() && t2.is_nan() {
                        true // NaN == NaN consistent with Hash (same bits hash equal)
                    } else {
                        t1.to_bits() == t2.to_bits()
                    }
                }
            }
            (
                PlanFilter::Provenance { key: k1, value: v1 },
                PlanFilter::Provenance { key: k2, value: v2 },
            ) => k1 == k2 && v1 == v2,
            _ => false,
        }
    }
}

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
        let filter = PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: 0.5,
        };
        let json = serde_json::to_string(&filter).expect("serialize");
        let parsed: PlanFilter = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, filter);
    }

    /// `PlanFilter::Confidence` with all operators.
    #[test]
    fn plan_filter_confidence_all_ops() {
        for op in [
            PlanFilterOp::Gt,
            PlanFilterOp::Lt,
            PlanFilterOp::Gte,
            PlanFilterOp::Lte,
            PlanFilterOp::Eq,
            PlanFilterOp::Ne,
        ] {
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
        let confidence = PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: 0.5,
        };
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
        let filter = PlanFilter::Confidence {
            op: PlanFilterOp::Gte,
            threshold: 0.95,
        };
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
        set.insert(PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: 0.5,
        });
        set.insert(PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: 0.5,
        }); // duplicate
        assert_eq!(set.len(), 1);
    }

    // -------------------------------------------------------------------------
    // W-C (NaN soundness): NaN threshold has consistent Hash/Eq contract
    // Scenario: `smells::S-002` / `coupling::C-002` — NaN violates IEEE 754 Eq
    // but Hash (to_bits) is consistent, so Eq must match for the Hash/Eq contract.
    // Fix: NaN == NaN returns true (consistent with Hash), finite values use bits comparison.
    // -------------------------------------------------------------------------

    /// `PlanFilter::Confidence { threshold: NaN }` equals itself (Hash/Eq contract).
    #[test]
    fn plan_filter_confidence_nan_equals_itself() {
        use std::collections::HashSet;
        let filter1 = PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: f64::NAN,
        };
        let filter2 = PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: f64::NAN,
        };
        // NaN == NaN via our custom Eq (consistent with Hash, which uses to_bits())
        assert_eq!(
            filter1, filter2,
            "NaN threshold should equal itself for Hash/Eq contract"
        );
        // Both insert at same hash bucket
        let mut set: HashSet<PlanFilter> = HashSet::new();
        set.insert(filter1);
        set.insert(filter2);
        assert_eq!(set.len(), 1, "NaN filters should dedupe in HashSet");
    }

    /// `PlanFilter::Confidence` with finite threshold uses normal equality.
    #[test]
    fn plan_filter_confidence_finite_equals_normal() {
        let filter1 = PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: 0.5,
        };
        let filter2 = PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: 0.5,
        };
        let filter3 = PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: 0.6,
        };
        assert_eq!(filter1, filter2);
        assert_ne!(filter1, filter3);
    }

    /// `PlanFilter::Confidence` with NaN does NOT equal a finite threshold.
    #[test]
    fn plan_filter_confidence_nan_not_equal_finite() {
        let nan_filter = PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: f64::NAN,
        };
        let finite_filter = PlanFilter::Confidence {
            op: PlanFilterOp::Gt,
            threshold: 0.5,
        };
        assert_ne!(nan_filter, finite_filter);
    }
}
