//! Self-hosting controlled mutation corpus (e76 WU3).
//!
//! A self-hosting evaluation is only as good as the mutations it
//! can detect. This module provides:
//!
//! 1. **Mutation taxonomy** — a small, explicit set of mutation
//!    kinds that exercise the seams most likely to be mis-predicted
//!    by a self-evaluator:
//!    - [`MutationKind::GroundingMismatch`] — produces a fact that
//!      cannot be grounded to source.
//!    - [`MutationKind::SemanticCallChange`] — changes a function
//!      call in a way that is semantically observable.
//!    - [`MutationKind::ArchitectureBoundaryViolation`] — adds an
//!      import that crosses a documented architectural boundary
//!      (e.g. domain → infrastructure).
//!    - [`MutationKind::ReadSetRegression`] — changes a function
//!      such that a previously-known read set becomes stale.
//!
//! 2. **Expected outcome declaration** — every mutation declares
//!    its expected outcomes BEFORE execution. The expected-outcome
//!    set is the input to the WU2 sealed-prediction evaluator.
//!
//! 3. **Mutation outcome accounting** — the corpus declares, for
//!    each mutation, the canonical list of oracle ids that SHOULD
//!    fire if the mutation behaves as designed. False negatives
//!    (a mutation that should fire X but X was not observed) are
//!    surfaced as a dedicated counter — they MUST remain visible
//!    and CANNOT be hidden by aggregate precision.
//!
//! This module does NOT execute mutations. The caller is
//! responsible for applying the mutation to a copy of the source,
//! running the oracles, and feeding the observations back via the
//! WU2 prediction evaluator.
//!
//! Design constraints:
//! - The module MUST be platform-neutral and oracle-neutral.
//! - The module MUST keep false negatives visible (the directive
//!   explicitly forbids hiding them behind aggregate precision).
//! - Every mutation declares its expected outcomes BEFORE
//!   execution. There is no constructor that allows an after-the-
//!   fact update.

use std::collections::BTreeSet;

/// The taxonomy of controlled mutations. Each kind corresponds to
/// a specific way the self-hosting evaluator could be wrong, and
/// the corpus picks at least one concrete instance of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MutationKind {
    /// A fact whose payload cannot be grounded back to source
    /// (e.g. a synthesized fact with no source span).
    GroundingMismatch,
    /// A function call changed in a way that is semantically
    /// observable at the IR / type / test level.
    SemanticCallChange,
    /// An import or module reference that crosses a documented
    /// architectural boundary (e.g. domain depending on
    /// infrastructure).
    ArchitectureBoundaryViolation,
    /// A function changed such that a previously-known read set
    /// (the set of state observed by the function) becomes stale.
    ReadSetRegression,
}

impl MutationKind {
    /// Short human-readable label (used in logs).
    pub fn label(self) -> &'static str {
        match self {
            MutationKind::GroundingMismatch => "grounding_mismatch",
            MutationKind::SemanticCallChange => "semantic_call_change",
            MutationKind::ArchitectureBoundaryViolation => "architecture_boundary_violation",
            MutationKind::ReadSetRegression => "read_set_regression",
        }
    }
}

/// A single controlled mutation with its declared expected
/// outcomes. The expected outcomes are a `BTreeSet` so the
/// declaration is canonically ordered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlledMutation {
    /// Stable identifier (`kind.instance_id`).
    pub id: String,
    /// The kind of mutation.
    pub kind: MutationKind,
    /// One-line description of what the mutation does. Used in
    /// logs; not interpreted by the scorer.
    pub description: String,
    /// Oracle ids that MUST fire if the mutation behaves as
    /// designed. The set is the input to the WU2 prediction
    /// evaluator (mapped to `ExpectedObservation { present: true }`).
    pub expected_oracle_ids: BTreeSet<String>,
    /// Oracle ids that MUST NOT fire. Mapped to
    /// `ExpectedObservation { present: false }`.
    pub forbidden_oracle_ids: BTreeSet<String>,
}

impl ControlledMutation {
    /// Construct a controlled mutation. No method exists for
    /// mutating `expected_oracle_ids` or `forbidden_oracle_ids`
    /// after construction — once declared, the expectations are
    /// sealed.
    pub fn new(id: impl Into<String>, kind: MutationKind, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            description: description.into(),
            expected_oracle_ids: BTreeSet::new(),
            forbidden_oracle_ids: BTreeSet::new(),
        }
    }

    /// Builder-style: declare an oracle id that MUST fire.
    pub fn must_observe(mut self, oracle_id: impl Into<String>) -> Self {
        self.expected_oracle_ids.insert(oracle_id.into());
        self
    }

    /// Builder-style: declare an oracle id that MUST NOT fire.
    pub fn must_not_observe(mut self, oracle_id: impl Into<String>) -> Self {
        self.forbidden_oracle_ids.insert(oracle_id.into());
        self
    }
}

/// The corpus of controlled mutations. At minimum the corpus
/// covers one instance per [`MutationKind`] (the four declared
/// in the e76 directive: grounding mismatch, semantic call change,
/// architecture boundary violation, read-set regression).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MutationCorpus {
    pub mutations: Vec<ControlledMutation>,
}

impl MutationCorpus {
    /// Build the canonical corpus with one instance per kind. This
    /// is the minimum coverage; concrete self-hosting runs may
    /// extend it.
    pub fn canonical() -> Self {
        Self {
            mutations: vec![
                ControlledMutation::new(
                    "m1.grounding_mismatch",
                    MutationKind::GroundingMismatch,
                    "synthesised fact with no source span",
                )
                .must_observe("grounding.fact.unsourced")
                .must_not_observe("grounding.fact.well_formed"),
                ControlledMutation::new(
                    "m2.semantic_call_change",
                    MutationKind::SemanticCallChange,
                    "swap a function call for a semantically different one",
                )
                .must_observe("semantic.call.changed")
                .must_not_observe("semantic.call.unchanged"),
                ControlledMutation::new(
                    "m3.architecture_boundary_violation",
                    MutationKind::ArchitectureBoundaryViolation,
                    "domain depends on infrastructure directly",
                )
                .must_observe("architecture.boundary.violated")
                .must_not_observe("architecture.boundary.respected"),
                ControlledMutation::new(
                    "m4.read_set_regression",
                    MutationKind::ReadSetRegression,
                    "function observes state outside its declared read set",
                )
                .must_observe("readset.regression.detected")
                .must_not_observe("readset.regression.absent"),
            ],
        }
    }

    /// Build the [`crate::application::self_hosting::prediction::SealedPrediction`]
    /// that this corpus produces. Each `must_observe` becomes a
    /// present expectation; each `must_not_observe` becomes an
    /// absent expectation.
    pub fn to_sealed_prediction(
        &self,
        label: impl Into<String>,
    ) -> crate::application::self_hosting::prediction::SealedPrediction {
        use crate::application::self_hosting::prediction::{ExpectedObservation, SealedPrediction};
        let mut expected: Vec<ExpectedObservation> = Vec::new();
        for m in &self.mutations {
            for id in &m.expected_oracle_ids {
                expected.push(ExpectedObservation {
                    id: id.clone(),
                    expected_present: true,
                });
            }
            for id in &m.forbidden_oracle_ids {
                expected.push(ExpectedObservation {
                    id: id.clone(),
                    expected_present: false,
                });
            }
        }
        SealedPrediction::seal(label, expected)
    }
}

/// Account for false negatives at the corpus level.
///
/// A mutation has a false negative if it declared `must_observe`
/// oracle X but X did not fire. This MUST remain visible in the
/// report; the directive explicitly forbids hiding false negatives
/// behind aggregate precision.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FalseNegativeReport {
    /// Number of (mutation, oracle) pairs that declared
    /// `must_observe` but did not fire.
    pub count: usize,
    /// Mutation ids that contributed false negatives (so they
    /// can be inspected individually).
    pub offending_mutations: Vec<String>,
}

impl FalseNegativeReport {
    /// Build the report from the corpus and the WU2 score matrix
    /// (or, more directly, from the set of observed oracle ids).
    pub fn from_observations(
        corpus: &MutationCorpus,
        observed_oracle_ids: &BTreeSet<String>,
    ) -> Self {
        let mut report = Self::default();
        for m in &corpus.mutations {
            let missed: Vec<&String> = m
                .expected_oracle_ids
                .iter()
                .filter(|id| !observed_oracle_ids.contains(*id))
                .collect();
            if !missed.is_empty() {
                report.count += missed.len();
                report.offending_mutations.push(m.id.clone());
            }
        }
        report
    }
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_corpus_covers_all_four_kinds() {
        let corpus = MutationCorpus::canonical();
        let kinds: std::collections::HashSet<_> = corpus.mutations.iter().map(|m| m.kind).collect();
        assert!(kinds.contains(&MutationKind::GroundingMismatch));
        assert!(kinds.contains(&MutationKind::SemanticCallChange));
        assert!(kinds.contains(&MutationKind::ArchitectureBoundaryViolation));
        assert!(kinds.contains(&MutationKind::ReadSetRegression));
    }

    #[test]
    fn canonical_corpus_declares_expected_outcomes() {
        let corpus = MutationCorpus::canonical();
        // Every mutation in the canonical corpus MUST declare at
        // least one `must_observe` outcome — otherwise there is
        // nothing to score against.
        for m in &corpus.mutations {
            assert!(
                !m.expected_oracle_ids.is_empty(),
                "mutation {} declares no must-observe oracle ids",
                m.id
            );
        }
    }

    #[test]
    fn sealed_prediction_carries_both_must_and_must_not() {
        let corpus = MutationCorpus::canonical();
        let pred = corpus.to_sealed_prediction("corpus-v1");
        let present: Vec<_> = pred
            .expected
            .iter()
            .filter(|e| e.expected_present)
            .map(|e| e.id.as_str())
            .collect();
        let absent: Vec<_> = pred
            .expected
            .iter()
            .filter(|e| !e.expected_present)
            .map(|e| e.id.as_str())
            .collect();
        assert!(present.contains(&"grounding.fact.unsourced"));
        assert!(present.contains(&"semantic.call.changed"));
        assert!(present.contains(&"architecture.boundary.violated"));
        assert!(present.contains(&"readset.regression.detected"));
        assert!(absent.contains(&"grounding.fact.well_formed"));
        assert!(absent.contains(&"readset.regression.absent"));
    }

    #[test]
    fn false_negative_report_is_zero_when_all_observations_fire() {
        let corpus = MutationCorpus::canonical();
        let observed: BTreeSet<String> = corpus
            .mutations
            .iter()
            .flat_map(|m| m.expected_oracle_ids.iter().cloned())
            .collect();
        let report = FalseNegativeReport::from_observations(&corpus, &observed);
        assert_eq!(report.count, 0);
        assert!(report.offending_mutations.is_empty());
    }

    #[test]
    fn false_negative_report_surfaces_missed_observations() {
        let corpus = MutationCorpus::canonical();
        // Observe everything except one of the four must-observe
        // signals.
        let mut observed: BTreeSet<String> = corpus
            .mutations
            .iter()
            .flat_map(|m| m.expected_oracle_ids.iter().cloned())
            .collect();
        observed.remove("grounding.fact.unsourced");
        let report = FalseNegativeReport::from_observations(&corpus, &observed);
        assert_eq!(report.count, 1);
        assert_eq!(
            report.offending_mutations,
            vec!["m1.grounding_mismatch".to_string()]
        );
    }

    #[test]
    fn mutation_kind_label_is_stable() {
        // The labels are part of the audit-log contract; a test
        // pins them so accidental renames are caught.
        assert_eq!(
            MutationKind::GroundingMismatch.label(),
            "grounding_mismatch"
        );
        assert_eq!(
            MutationKind::SemanticCallChange.label(),
            "semantic_call_change"
        );
        assert_eq!(
            MutationKind::ArchitectureBoundaryViolation.label(),
            "architecture_boundary_violation"
        );
        assert_eq!(
            MutationKind::ReadSetRegression.label(),
            "read_set_regression"
        );
    }

    #[test]
    fn mutation_corpus_does_not_leak_platform_specific_identifiers() {
        // The module is platform-neutral and oracle-neutral.
        // It must not reference specific oracles or runtimes
        // in code.
        let full = include_str!("mutation_corpus.rs");
        let stripped = crate::application::portable_execution::strip_doc_comments_and_tests(full);
        let lower = stripped.to_lowercase();
        for forbidden in [
            "podman", "systemd", "quadlet", "wsl", "hyper-v", "docker", "rustc", "cargo ",
            "cargo.", "clippy",
        ] {
            assert!(
                !lower.contains(forbidden),
                "self_hosting::mutation_corpus code (non-test, non-doc) must not leak identifier: {forbidden}"
            );
        }
    }
}
