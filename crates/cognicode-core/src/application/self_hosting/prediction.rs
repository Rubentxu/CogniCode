//! Self-hosting prediction vs observation (e76 WU2).
//!
//! When CogniCode evaluates CogniCode, the evaluation must satisfy
//! two structural properties to remain honest:
//!
//! 1. **Sealed predictions**: the predictions are recorded BEFORE
//!    the candidate change is observed. They cannot be retro-fitted
//!    to the observations. This is the analogue of "pre-registration"
//!    in empirical science.
//!
//! 2. **Independent observations**: the observations come from
//!    independent oracles (rustc, cargo test, cargo clippy, the
//!    sandbox/equivalence harnesses). CogniCode does NOT grade
//!    itself. The observations are inputs; the evaluator only
//!    compares them to the sealed predictions.
//!
//! This module provides:
//!
//! - [`SealedPrediction`] — an immutable prediction record (a set
//!   of expected observations, signed by a digest at sealing time).
//! - [`Observation`] — a single observed outcome (oracle id +
//!   optional payload digest).
//! - [`ScoreMatrix`] — TP/FP/FN counts derived from a comparison.
//! - [`score`] — the function that compares a sealed prediction to a
//!   set of observations and produces the score matrix.
//!
//! What this module does NOT do:
//!
//! - It does NOT execute oracles. The caller runs rustc/cargo/etc
//!   and feeds the observations in.
//! - It does NOT mint authority (no Pass/Fail). A score is a number;
//!   the gate is the authority.
//! - It does NOT persist anything. Persistence is the caller's job.
//!
//! Design constraint: this module MUST be platform-neutral and
//! MUST NOT mention any oracle-specific runtime identifier in code
//! (rustc, cargo, clippy, podman, docker, etc.). The forbidden list
//! is checked by a static test.

// =========================================================================
// Prediction (sealed)
// =========================================================================

/// A single expected observation. Predictions are sets of these;
/// the set is sealed (frozen) at the moment the prediction is made,
/// so post-hoc observation cannot influence what was predicted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExpectedObservation {
    /// Stable identifier of the expected observation. Conventionally
    /// a namespaced string (`oracle.scope.subject`, e.g.
    /// `cargo.test.pass`). The oracle is NOT evaluated here; the
    /// caller chooses the namespace.
    pub id: String,
    /// Whether the caller expected the observation to be present
    /// (`true`) or absent (`false`). Absent expectations are useful
    /// for predicting regressions: "this oracle MUST NOT fire".
    pub expected_present: bool,
}

/// A sealed (immutable) prediction record.
///
/// `seal_digest` is computed over the canonical encoding of
/// `expected` at sealing time. The digest is what gets logged so
/// later audits can confirm the predictions were not modified after
/// the observations were collected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedPrediction {
    /// Human-readable label of the prediction (e.g.
    /// "self-host-baseline-v1").
    pub label: String,
    /// The frozen set of expected observations.
    pub expected: Vec<ExpectedObservation>,
    /// Digest computed at sealing time. Conventionally a SHA-256
    /// hex string over the canonical JSON encoding of `expected`.
    /// The seal binds the prediction to its content at sealing
    /// time; any later mutation changes the seal.
    pub seal_digest: String,
}

impl SealedPrediction {
    /// Build a sealed prediction. The seal is computed by sorting
    /// `expected` by `id` (canonical order) and hashing the joined
    /// `id|expected_present` strings via SHA-256.
    ///
    /// Sorting is important: the seal must be invariant to the
    /// caller's insertion order, otherwise two semantically equal
    /// predictions would seal differently.
    pub fn seal(label: impl Into<String>, expected: Vec<ExpectedObservation>) -> Self {
        let mut sorted = expected;
        sorted.sort_by(|a, b| a.id.cmp(&b.id));
        let canonical = sorted
            .iter()
            .map(|e| format!("{}|{}", e.id, e.expected_present))
            .collect::<Vec<_>>()
            .join("\n");
        let seal_digest = crate::application::portable_execution::content_digest(canonical.as_bytes())
            .as_str()
            .to_string();
        Self {
            label: label.into(),
            expected: sorted,
            seal_digest,
        }
    }
}

// =========================================================================
// Observation (recorded after the fact)
// =========================================================================

/// A single observed outcome, produced by an independent oracle.
///
/// Observations are APPENDED to the audit log; they do not modify
/// the sealed prediction. This asymmetry is the structural
/// protection against circular self-judgement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Observation {
    /// Identifier of the observation. Matches the prediction's
    /// `ExpectedObservation::id` namespace.
    pub id: String,
    /// Whether the observation fired (was present). `false` means
    /// the oracle did NOT produce this signal — which is itself
    /// information (an "absent observation").
    pub present: bool,
}

// =========================================================================
// Scoring
// =========================================================================

/// Counts of true positives, false positives, false negatives, true
/// negatives, and unknowns.
///
/// Definitions:
/// - TP: expected present + observed present.
/// - FP: expected absent + observed present (unexpected signal).
/// - FN: expected present + observed absent (missed prediction).
/// - TN: expected absent + observed absent.
/// - Unknown: observation id is in neither prediction nor observed
///   set (this is a labelling gap; it must remain visible).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScoreMatrix {
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
    pub true_negative: usize,
    pub unknown: usize,
}

impl ScoreMatrix {
    /// Total number of expected observations (TP + FN).
    pub fn expected_count(&self) -> usize {
        self.true_positive + self.false_negative
    }
    /// Total number of observed observations (TP + FP).
    pub fn observed_count(&self) -> usize {
        self.true_positive + self.false_positive
    }
    /// Precision = TP / (TP + FP). Returns None if no observed
    /// observations (degenerate).
    pub fn precision(&self) -> Option<f64> {
        let denom = self.observed_count();
        if denom == 0 {
            None
        } else {
            Some(self.true_positive as f64 / denom as f64)
        }
    }
    /// Recall = TP / (TP + FN). Returns None if no expected
    /// observations (degenerate).
    pub fn recall(&self) -> Option<f64> {
        let denom = self.expected_count();
        if denom == 0 {
            None
        } else {
            Some(self.true_positive as f64 / denom as f64)
        }
    }
    /// Unknown/fallback rate = unknown / total comparison entries.
    /// Must remain visible; the directive explicitly forbids hiding
    /// unknowns behind aggregate precision.
    pub fn unknown_rate(&self, total: usize) -> f64 {
        if total == 0 {
            0.0
        } else {
            self.unknown as f64 / total as f64
        }
    }
}

/// Compare a sealed prediction to a set of observations and
/// produce a [`ScoreMatrix`].
///
/// Algorithm (the documented truth table, per expectation):
///
/// ```text
/// expected_present  observed   result
/// true              true       TP
/// true              false      FN
/// true              missing    FN   (a missing observation is absent)
/// false             true       FP   (an unexpected signal)
/// false             false      TN
/// false             missing    TN
/// ```
///
/// Then every observation absent from the expected set is counted:
/// present → FP, absent → TN.
///
/// The `(false, true)` cell was previously counted as TP because the
/// implementation matched on the observation alone. That contradicted the
/// documented contract and hid real violations of "this MUST NOT fire"
/// expectations; it is corrected here (e82.2).
///
/// `unknown` remains 0 from this function. A case with no recorded observation
/// at all is represented by e81's `ReplayCaseOutcome::Incomplete`, not by an
/// invented unknown count.
pub fn score(prediction: &SealedPrediction, observations: &[Observation]) -> ScoreMatrix {
    use std::collections::HashMap;
    let obs_index: HashMap<&str, bool> =
        observations.iter().map(|o| (o.id.as_str(), o.present)).collect();
    let mut matrix = ScoreMatrix::default();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for exp in &prediction.expected {
        seen.insert(exp.id.as_str());
        let observed = obs_index.get(exp.id.as_str()).copied();
        match (exp.expected_present, observed) {
            (true, Some(true)) => matrix.true_positive += 1,
            (true, Some(false)) | (true, None) => matrix.false_negative += 1,
            (false, Some(true)) => matrix.false_positive += 1,
            (false, Some(false)) | (false, None) => matrix.true_negative += 1,
        }
    }
    for obs in observations {
        if seen.contains(obs.id.as_str()) {
            continue;
        }
        // Observation has no matching expectation.
        if obs.present {
            matrix.false_positive += 1;
        } else {
            matrix.true_negative += 1;
        }
    }
    // Unknown = ids that are neither in prediction nor observations
    // (this is a degenerate case here; surfaced for completeness).
    matrix.unknown = 0;
    matrix
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn exp(id: &str, present: bool) -> ExpectedObservation {
        ExpectedObservation {
            id: id.into(),
            expected_present: present,
        }
    }

    fn obs(id: &str, present: bool) -> Observation {
        Observation {
            id: id.into(),
            present,
        }
    }

    #[test]
    fn seal_is_invariant_to_input_order() {
        let a = SealedPrediction::seal(
            "t",
            vec![exp("x", true), exp("y", false), exp("z", true)],
        );
        let b = SealedPrediction::seal(
            "t",
            vec![exp("z", true), exp("x", true), exp("y", false)],
        );
        assert_eq!(a.seal_digest, b.seal_digest);
        assert_eq!(a.expected, b.expected);
    }

    #[test]
    fn seal_changes_when_content_changes() {
        let a = SealedPrediction::seal("t", vec![exp("x", true)]);
        let b = SealedPrediction::seal("t", vec![exp("x", false)]);
        assert_ne!(a.seal_digest, b.seal_digest);
    }

    #[test]
    fn score_perfect_match_yields_only_tp_and_tn() {
        let p = SealedPrediction::seal(
            "t",
            vec![exp("a", true), exp("b", false), exp("c", true)],
        );
        let observations = vec![obs("a", true), obs("b", false), obs("c", true)];
        let m = score(&p, &observations);
        assert_eq!(m.true_positive, 2);
        assert_eq!(m.false_positive, 0);
        assert_eq!(m.false_negative, 0);
        assert_eq!(m.true_negative, 1);
        assert_eq!(m.precision(), Some(1.0));
        assert_eq!(m.recall(), Some(1.0));
    }

    #[test]
    fn score_false_negative_when_expected_present_but_observed_absent() {
        let p = SealedPrediction::seal("t", vec![exp("a", true), exp("b", true)]);
        let observations = vec![obs("a", true), obs("b", false)];
        let m = score(&p, &observations);
        assert_eq!(m.true_positive, 1);
        assert_eq!(m.false_negative, 1);
        assert_eq!(m.recall(), Some(0.5));
    }

    #[test]
    fn score_false_positive_when_unexpected_observation_present() {
        let p = SealedPrediction::seal("t", vec![exp("a", true)]);
        let observations = vec![obs("a", true), obs("surprise", true)];
        let m = score(&p, &observations);
        assert_eq!(m.true_positive, 1);
        assert_eq!(m.false_positive, 1);
        assert_eq!(m.precision(), Some(0.5));
    }

    #[test]
    fn score_treats_missing_observation_as_absent() {
        // The expected set mentions `b`; the observation set
        // doesn't. The evaluator MUST treat this as FN, not as
        // "unknown" — otherwise missing observations could be
        // hidden by the unknown bucket.
        let p = SealedPrediction::seal("t", vec![exp("a", true), exp("b", true)]);
        let observations = vec![obs("a", true)];
        let m = score(&p, &observations);
        assert_eq!(m.true_positive, 1);
        assert_eq!(m.false_negative, 1);
        assert_eq!(m.unknown, 0);
    }

    #[test]
    fn precision_and_recall_are_none_for_empty_inputs() {
        let m = ScoreMatrix::default();
        assert_eq!(m.precision(), None);
        assert_eq!(m.recall(), None);
        assert_eq!(m.unknown_rate(0), 0.0);
    }

    #[test]
    fn unknown_rate_remains_visible() {
        // The unknown rate is a structural invariant. Even when
        // the score is perfect, the function must report the rate
        // given the total comparison size.
        let m = ScoreMatrix {
            true_positive: 3,
            false_positive: 0,
            false_negative: 0,
            true_negative: 1,
            unknown: 1,
        };
        let total = 5;
        assert!((m.unknown_rate(total) - 0.2).abs() < 1e-9);
    }

    #[test]
    fn prediction_module_does_not_leak_platform_specific_identifiers() {
        // The module is platform-neutral and oracle-neutral.
        // It must not reference specific oracles or runtimes
        // in code (we strip both doc comments and the test
        // module, since the test necessarily enumerates the
        // forbidden names as data for the check).
        let full = include_str!("prediction.rs");
        let stripped = crate::application::portable_execution::strip_doc_comments_and_tests(full);
        let lower = stripped.to_lowercase();
        for forbidden in [
            "podman", "systemd", "quadlet", "wsl", "hyper-v", "docker",
            "rustc", "cargo ", "cargo.", "clippy",
        ] {
            assert!(
                !lower.contains(forbidden),
                "self_hosting::prediction code (non-test, non-doc) must not leak identifier: {forbidden}"
            );
        }
    }

    // ── e82.2: scorer truth table ────────────────────────────────────────

    /// Score a single (expectation, observation) cell.
    fn cell(expected_present: bool, observed: Option<bool>) -> ScoreMatrix {
        let prediction = SealedPrediction::seal("truth-table", vec![exp("x", expected_present)]);
        let observations: Vec<Observation> = match observed {
            Some(present) => vec![obs("x", present)],
            None => Vec::new(),
        };
        score(&prediction, &observations)
    }

    fn counts(matrix: &ScoreMatrix) -> (usize, usize, usize, usize) {
        (
            matrix.true_positive,
            matrix.false_positive,
            matrix.false_negative,
            matrix.true_negative,
        )
    }

    #[test]
    fn e82_2_score_truth_table_is_exactly_the_documented_contract() {
        // expected=true, observed=true -> TP
        assert_eq!(counts(&cell(true, Some(true))), (1, 0, 0, 0));
        // expected=true, observed=false -> FN
        assert_eq!(counts(&cell(true, Some(false))), (0, 0, 1, 0));
        // expected=false, observed=true -> FP (the corrected cell)
        assert_eq!(counts(&cell(false, Some(true))), (0, 1, 0, 0));
        // expected=false, observed=false -> TN
        assert_eq!(counts(&cell(false, Some(false))), (0, 0, 0, 1));
    }

    #[test]
    fn e82_2_missing_observation_is_treated_as_absent() {
        // expected=true, observation missing -> FN
        assert_eq!(counts(&cell(true, None)), (0, 0, 1, 0));
        // expected=false, observation missing -> TN
        assert_eq!(counts(&cell(false, None)), (0, 0, 0, 1));
    }

    #[test]
    fn e82_2_unexpected_present_observation_is_a_false_positive() {
        let prediction = SealedPrediction::seal("t", vec![exp("known", true)]);
        let observations = vec![obs("known", true), obs("surprise", true)];
        assert_eq!(counts(&score(&prediction, &observations)), (1, 1, 0, 0));
    }

    #[test]
    fn e82_2_truth_table_is_order_invariant() {
        let a = SealedPrediction::seal("t", vec![exp("x", false), exp("y", true)]);
        let b = SealedPrediction::seal("t", vec![exp("y", true), exp("x", false)]);
        let observations = vec![obs("x", true), obs("y", true)];
        assert_eq!(score(&a, &observations), score(&b, &observations));
        let reversed = vec![obs("y", true), obs("x", true)];
        assert_eq!(score(&a, &observations), score(&a, &reversed));
    }

    #[test]
    fn e82_2_duplicate_observation_ids_have_last_wins_semantics() {
        // CHARACTERIZED, not changed: `score` indexes observations into a map
        // keyed by id, so the LAST duplicate wins. That is deterministic (not
        // order-random), but it is a silent collapse. Rejecting duplicates
        // would change the signature and widen e82.2's scope, so it is pinned
        // here and recorded as a limitation instead.
        let prediction = SealedPrediction::seal("t", vec![exp("x", true)]);
        let present_then_absent = vec![obs("x", true), obs("x", false)];
        let absent_then_present = vec![obs("x", false), obs("x", true)];
        assert_eq!(
            counts(&score(&prediction, &present_then_absent)),
            (0, 0, 1, 0)
        );
        assert_eq!(
            counts(&score(&prediction, &absent_then_present)),
            (1, 0, 0, 0)
        );
    }
}
