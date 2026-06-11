//! `ConfidenceRules` — sole sanctioned path for assigning `(Provenance, f64)`
//! to a `CallGraph` edge.
//!
//! The rule table is fixed and **must be enforced from one place**. Callers
//! express their *intent* via [`ExtractionContext`], not raw values, so the
//! domain owns the mapping:
//!
//! | Context                          | Output `(Provenance, f64)`        |
//! |----------------------------------|-----------------------------------|
//! | `DirectExtraction`               | `(Extracted, 1.0)`                |
//! | `Heuristic { score: s }`         | `(Inferred, clamp(s, 0.5, 0.9))`  |
//! | `Unresolved`                     | `(Ambiguous, 0.3)`                |
//! | `Manual`                         | `(Manual, 1.0)`                   |
//! | `Tested`                         | `(Tested, 1.0)`                   |
//!
//! ## Rejection rules
//!
//! [`ConfidenceRules::assign`] returns a [`ConfidenceError`] when:
//! * a `Heuristic` score is `NaN`, `±inf`, or outside `[0.0, 1.0]`,
//!   because clamping silently would hide caller bugs.
//!
//! The assigned `confidence` always satisfies
//! `0.0 <= c <= 1.0 && !c.is_nan() && !c.is_infinite()`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::value_objects::Provenance;

/// How a call-graph edge was obtained.
///
/// This enum is the **only** accepted input to
/// [`ConfidenceRules::assign`]. Callers must describe the *extraction
/// context*, not the desired `(Provenance, f64)` pair.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ExtractionContext {
    /// Edge was observed directly in the source (AST extractor call node).
    DirectExtraction,
    /// Edge was produced by a heuristic resolver. The `score` is a
    /// confidence value in `[0.0, 1.0]`. It is **clamped** into the
    /// `[0.5, 0.9]` band by the rules service, but the input score
    /// itself must be a finite, in-range number.
    Heuristic {
        /// Confidence value in `[0.0, 1.0]`. Out-of-range, `NaN`, and
        /// infinite values are rejected.
        score: f64,
    },
    /// Edge could not be resolved to a single concrete target.
    Unresolved,
    /// Edge was curated by a human (added manually to the graph).
    /// Preserved as a distinct provenance across the
    /// store / load round-trip — used by the postgres
    /// repository to faithfully restore `Provenance::Manual`
    /// rows. Maps to `(Provenance::Manual, 1.0)`.
    Manual,
    /// Edge is backed by a passing test. Preserved as a distinct
    /// provenance across the store / load round-trip. Maps to
    /// `(Provenance::Tested, 1.0)`.
    Tested,
}

/// Failure cases for [`ConfidenceRules::assign`].
///
/// `PartialEq` is derived (not `Eq`) because `OutOfRange` carries an
/// `f64` payload, and `f64` does not implement `Eq`.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum ConfidenceError {
    /// Score was not a real number (i.e. `f64::NAN`).
    #[error("confidence score is NaN")]
    NotANumber,
    /// Score was `+inf` or `-inf`.
    #[error("confidence score is infinite")]
    Infinite,
    /// Score was outside the closed interval `[0.0, 1.0]`.
    #[error("confidence score {0} is outside [0.0, 1.0]")]
    OutOfRange(f64),
}

/// Domain service that maps an [`ExtractionContext`] to the canonical
/// `(Provenance, confidence)` pair for a `CallGraph` edge.
///
/// The mapping is fixed and documented in the module-level table. The
/// service is stateless — instantiate it once (or use the
/// [`Default`] impl) and call [`Self::assign`] per edge.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConfidenceRules;

impl ConfidenceRules {
    /// Construct a new rules service. Equivalent to `Default::default()`.
    pub fn new() -> Self {
        Self
    }

    /// Assign `(Provenance, confidence)` to an edge produced in the given
    /// extraction context.
    ///
    /// # Errors
    ///
    /// Returns [`ConfidenceError::NotANumber`] / [`ConfidenceError::Infinite`]
    /// / [`ConfidenceError::OutOfRange`] when the `Heuristic` score is
    /// invalid. `DirectExtraction`, `Unresolved`, `Manual`, and
    /// `Tested` never fail.
    pub fn assign(&self, ctx: ExtractionContext) -> Result<(Provenance, f64), ConfidenceError> {
        match ctx {
            ExtractionContext::DirectExtraction => Ok((Provenance::Extracted, 1.0)),
            ExtractionContext::Heuristic { score } => {
                if score.is_nan() {
                    return Err(ConfidenceError::NotANumber);
                }
                if !score.is_finite() {
                    return Err(ConfidenceError::Infinite);
                }
                if !(0.0..=1.0).contains(&score) {
                    return Err(ConfidenceError::OutOfRange(score));
                }
                let clamped = score.clamp(0.5, 0.9);
                Ok((Provenance::Inferred, clamped))
            }
            ExtractionContext::Unresolved => Ok((Provenance::Ambiguous, 0.3)),
            // Manual and Tested round-trip bit-exactly. The
            // stored confidence is 1.0 (matching the previous
            // lossy DirectExtraction round-trip) so older rows
            // that already carry `(Manual, 1.0)` /
            // `(Tested, 1.0)` survive the migration unchanged.
            ExtractionContext::Manual => Ok((Provenance::Manual, 1.0)),
            ExtractionContext::Tested => Ok((Provenance::Tested, 1.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Golden fixtures --------------------------------------------------
    // These pin the contract bit-exactly. If you change a value, you must
    // also update the spec's golden table — they are the same source of
    // truth, just expressed in two places (Rust test + spec doc).

    #[test]
    fn golden_direct_extraction() {
        let rules = ConfidenceRules::new();
        let (p, c) = rules
            .assign(ExtractionContext::DirectExtraction)
            .expect("DirectExtraction never errors");
        assert_eq!(p, Provenance::Extracted);
        assert_eq!(c, 1.0_f64);
    }

    #[test]
    fn golden_manual() {
        let rules = ConfidenceRules::new();
        let (p, c) = rules
            .assign(ExtractionContext::Manual)
            .expect("Manual never errors");
        assert_eq!(p, Provenance::Manual);
        assert_eq!(c, 1.0_f64);
    }

    #[test]
    fn golden_tested() {
        let rules = ConfidenceRules::new();
        let (p, c) = rules
            .assign(ExtractionContext::Tested)
            .expect("Tested never errors");
        assert_eq!(p, Provenance::Tested);
        assert_eq!(c, 1.0_f64);
    }

    #[test]
    fn golden_unresolved() {
        let rules = ConfidenceRules::new();
        let (p, c) = rules
            .assign(ExtractionContext::Unresolved)
            .expect("Unresolved never errors");
        assert_eq!(p, Provenance::Ambiguous);
        assert_eq!(c, 0.3_f64);
    }

    #[test]
    fn golden_heuristic_above_band_is_clamped() {
        let rules = ConfidenceRules::new();
        let (p, c) = rules
            .assign(ExtractionContext::Heuristic { score: 1.0 })
            .expect("score in range");
        assert_eq!(p, Provenance::Inferred);
        assert_eq!(c, 0.9_f64); // clamped to top of band
    }

    #[test]
    fn golden_heuristic_below_band_is_clamped() {
        let rules = ConfidenceRules::new();
        let (p, c) = rules
            .assign(ExtractionContext::Heuristic { score: 0.1 })
            .expect("score in range");
        assert_eq!(p, Provenance::Inferred);
        assert_eq!(c, 0.5_f64); // clamped to bottom of band
    }

    #[test]
    fn golden_heuristic_inside_band_passes_through() {
        let rules = ConfidenceRules::new();
        for score in [0.5_f64, 0.7_f64, 0.9_f64] {
            let (p, c) = rules
                .assign(ExtractionContext::Heuristic { score })
                .expect("score in range");
            assert_eq!(p, Provenance::Inferred);
            assert_eq!(c, score, "score {score} should pass through unchanged");
        }
    }

    // --- Rejection paths ---------------------------------------------------

    #[test]
    fn rejects_nan() {
        let rules = ConfidenceRules::new();
        let result = rules.assign(ExtractionContext::Heuristic { score: f64::NAN });
        assert!(matches!(result, Err(ConfidenceError::NotANumber)));
    }

    #[test]
    fn rejects_positive_infinity() {
        let rules = ConfidenceRules::new();
        let result = rules.assign(ExtractionContext::Heuristic {
            score: f64::INFINITY,
        });
        assert!(matches!(result, Err(ConfidenceError::Infinite)));
    }

    #[test]
    fn rejects_negative_infinity() {
        let rules = ConfidenceRules::new();
        let result = rules.assign(ExtractionContext::Heuristic {
            score: f64::NEG_INFINITY,
        });
        assert!(matches!(result, Err(ConfidenceError::Infinite)));
    }

    #[test]
    fn rejects_above_range() {
        let rules = ConfidenceRules::new();
        let result = rules.assign(ExtractionContext::Heuristic { score: 1.2 });
        assert!(matches!(result, Err(ConfidenceError::OutOfRange(1.2))));
    }

    #[test]
    fn rejects_below_range() {
        let rules = ConfidenceRules::new();
        let result = rules.assign(ExtractionContext::Heuristic { score: -0.1 });
        assert!(matches!(result, Err(ConfidenceError::OutOfRange(-0.1))));
    }

    // --- Invariant: every accepted result is in [0.0, 1.0] and finite ---

    #[test]
    fn invariant_every_accepted_result_is_in_range_and_finite() {
        let rules = ConfidenceRules::new();
        let contexts = [
            ExtractionContext::DirectExtraction,
            ExtractionContext::Unresolved,
            ExtractionContext::Heuristic { score: 0.0 },
            ExtractionContext::Heuristic { score: 0.3 },
            ExtractionContext::Heuristic { score: 0.5 },
            ExtractionContext::Heuristic { score: 0.7 },
            ExtractionContext::Heuristic { score: 0.9 },
            ExtractionContext::Heuristic { score: 1.0 },
        ];
        for ctx in contexts {
            let (p, c) = rules.assign(ctx).expect("in-range input never fails");
            assert!(
                (0.0..=1.0).contains(&c),
                "{p:?} produced out-of-range confidence {c} for {ctx:?}"
            );
            assert!(!c.is_nan(), "{p:?} produced NaN for {ctx:?}");
            assert!(
                c.is_finite(),
                "{p:?} produced non-finite confidence for {ctx:?}"
            );
        }
    }
}
