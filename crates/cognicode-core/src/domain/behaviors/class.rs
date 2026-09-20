//! Behavior classes and authority (M7.3, cycle e64, ADR-044).
//!
//! ## The class is not the actor
//!
//! It is tempting to derive authority from *who* is running:
//!
//! ```text
//! ActorKind::Agent  ⇒ AgentBehavior        ← wrong
//! ActorKind::Behavior ⇒ ReactiveAnalysis   ← wrong
//! ```
//!
//! They are different dimensions. A human can trigger an `AgentBehavior`, and
//! an AI agent can *request* a trusted `ReactiveAnalysis`; neither transforms
//! the other. The class belongs to the behavior's **definition**, is fixed by
//! **admission**, and travels with the permit.
//!
//! ## Effects, not deliverables
//!
//! The table does not say "may fabricate a Finding". A behavior never fabricates
//! a finding: it may only ask to *execute an already-admitted analysis*, and that
//! request still goes through `DetectorExecutor`, `FindingAssembler` and
//! `FindingVerifier`. Modelling effects this way keeps the M6 seam intact
//! instead of opening a second path into findings.
//!
//! ## The table
//!
//! ```text
//! effect                      PureDerivation  ReactiveAnalysis  AgentBehavior
//! CommitCanonicalFact              yes              no              no
//! RecordEvidence                   yes             yes             yes
//! RecordHypothesis                  no             yes             yes
//! ProposeChange                     no              no             yes
//! ExecuteAdmittedAnalysis           no             yes             yes
//! ```
//!
//! `PureDerivation` commits facts because that is what a deterministic pass
//! is *for*; it may not hypothesize or propose, because it does not speculate.
//! `AgentBehavior` may propose but never commit canonical truth, because
//! canonical truth is established by mechanisms that can be checked, not by
//! assertion. `ExecuteAdmittedAnalysis` on an `AgentBehavior` grants no new
//! authority: it can only ask for a permit that admission already minted.
//!
//! Pure domain: no I/O.

use serde::{Deserialize, Serialize};

/// What a behavior is allowed to be, decided by admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorClass {
    /// Derived deterministically from canonical inputs. May commit facts.
    PureDerivation,
    /// Reacts to a change by re-running admitted analysis.
    ReactiveAnalysis,
    /// An agent acting on its own judgement.
    AgentBehavior,
}

impl BehaviorClass {
    /// Stable name for diagnostics and event payloads.
    pub fn name(self) -> &'static str {
        match self {
            Self::PureDerivation => "pure_derivation",
            Self::ReactiveAnalysis => "reactive_analysis",
            Self::AgentBehavior => "agent_behavior",
        }
    }

    /// Whether this class may commit canonical facts.
    pub fn may_commit_canonical_truth(self) -> bool {
        matches!(self, Self::PureDerivation)
    }
}

impl std::fmt::Display for BehaviorClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Something a behavior may ask the platform to do.
///
/// These are *requests*, not capabilities the behavior holds: an authorized
/// effect is handed to an adapter by the runtime, and the adapter is the only
/// thing that touches the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorEffectKind {
    /// Write a fact into canonical truth.
    CommitCanonicalFact,
    /// Record evidence, at the grade the evidence actually carries.
    RecordEvidence,
    /// Record a hypothesis: explicitly not canonical truth.
    RecordHypothesis,
    /// Propose a change for a human or a later gate to accept.
    ProposeChange,
    /// Ask the platform to run an analysis that permission already admitted.
    ///
    /// This never grants new authority: a behavior cannot mint an
    /// `ExecutionPermit`, it can only request that an admitted one be run.
    ExecuteAdmittedAnalysis,
}

impl BehaviorEffectKind {
    /// Stable name for diagnostics and event payloads.
    pub fn name(self) -> &'static str {
        match self {
            Self::CommitCanonicalFact => "commit_canonical_fact",
            Self::RecordEvidence => "record_evidence",
            Self::RecordHypothesis => "record_hypothesis",
            Self::ProposeChange => "propose_change",
            Self::ExecuteAdmittedAnalysis => "execute_admitted_analysis",
        }
    }

    /// Every effect, in a stable order.
    pub fn all() -> Vec<Self> {
        vec![
            Self::CommitCanonicalFact,
            Self::RecordEvidence,
            Self::RecordHypothesis,
            Self::ProposeChange,
            Self::ExecuteAdmittedAnalysis,
        ]
    }
}

impl std::fmt::Display for BehaviorEffectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The authority table: what each class may ask for.
#[derive(Debug, Clone, Copy, Default)]
pub struct BehaviorAuthorityPolicy;

impl BehaviorAuthorityPolicy {
    /// Whether `class` may request `effect`.
    ///
    /// The runtime asks this **before** the adapter is reached, so a refusal is
    /// structural rather than a promise.
    pub fn allows(class: BehaviorClass, effect: BehaviorEffectKind) -> bool {
        use BehaviorEffectKind::*;
        match class {
            BehaviorClass::PureDerivation => matches!(effect, CommitCanonicalFact | RecordEvidence),
            BehaviorClass::ReactiveAnalysis => {
                matches!(
                    effect,
                    RecordEvidence | RecordHypothesis | ExecuteAdmittedAnalysis
                )
            }
            BehaviorClass::AgentBehavior => matches!(
                effect,
                RecordEvidence | RecordHypothesis | ProposeChange | ExecuteAdmittedAnalysis
            ),
        }
    }

    /// The effects `class` may request, in a stable order.
    pub fn allowed(class: BehaviorClass) -> Vec<BehaviorEffectKind> {
        BehaviorEffectKind::all()
            .into_iter()
            .filter(|e| Self::allows(class, *e))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_pure_derivation_may_commit_canonical_truth() {
        let class = BehaviorClass::PureDerivation;
        assert!(BehaviorAuthorityPolicy::allows(
            class,
            BehaviorEffectKind::CommitCanonicalFact
        ));
        for class in [
            BehaviorClass::ReactiveAnalysis,
            BehaviorClass::AgentBehavior,
        ] {
            assert!(
                !BehaviorAuthorityPolicy::allows(class, BehaviorEffectKind::CommitCanonicalFact),
                "{class} must never commit canonical truth"
            );
        }
    }

    #[test]
    fn only_an_agent_may_propose_a_change() {
        assert!(BehaviorAuthorityPolicy::allows(
            BehaviorClass::AgentBehavior,
            BehaviorEffectKind::ProposeChange
        ));
        for class in [
            BehaviorClass::PureDerivation,
            BehaviorClass::ReactiveAnalysis,
        ] {
            assert!(!BehaviorAuthorityPolicy::allows(
                class,
                BehaviorEffectKind::ProposeChange
            ));
        }
    }

    #[test]
    fn a_pure_derivation_does_not_speculate() {
        for effect in [
            BehaviorEffectKind::RecordHypothesis,
            BehaviorEffectKind::ProposeChange,
            BehaviorEffectKind::ExecuteAdmittedAnalysis,
        ] {
            assert!(
                !BehaviorAuthorityPolicy::allows(BehaviorClass::PureDerivation, effect),
                "a deterministic pass must not {effect}"
            );
        }
    }

    #[test]
    fn every_class_may_record_evidence() {
        for class in [
            BehaviorClass::PureDerivation,
            BehaviorClass::ReactiveAnalysis,
            BehaviorClass::AgentBehavior,
        ] {
            assert!(BehaviorAuthorityPolicy::allows(
                class,
                BehaviorEffectKind::RecordEvidence
            ));
        }
    }

    /// The published table, asserted row by row so a change to it is a visible
    /// decision rather than a silent drift.
    #[test]
    fn the_table_is_exactly_what_the_design_says() {
        use BehaviorClass::*;
        use BehaviorEffectKind::*;
        let expected = [
            (PureDerivation, CommitCanonicalFact, true),
            (PureDerivation, RecordEvidence, true),
            (PureDerivation, RecordHypothesis, false),
            (PureDerivation, ProposeChange, false),
            (PureDerivation, ExecuteAdmittedAnalysis, false),
            (ReactiveAnalysis, CommitCanonicalFact, false),
            (ReactiveAnalysis, RecordEvidence, true),
            (ReactiveAnalysis, RecordHypothesis, true),
            (ReactiveAnalysis, ProposeChange, false),
            (ReactiveAnalysis, ExecuteAdmittedAnalysis, true),
            (AgentBehavior, CommitCanonicalFact, false),
            (AgentBehavior, RecordEvidence, true),
            (AgentBehavior, RecordHypothesis, true),
            (AgentBehavior, ProposeChange, true),
            (AgentBehavior, ExecuteAdmittedAnalysis, true),
        ];
        for (class, effect, allowed) in expected {
            assert_eq!(
                BehaviorAuthorityPolicy::allows(class, effect),
                allowed,
                "{class} / {effect}"
            );
        }
        assert_eq!(expected.len(), 15, "the table is exhaustive");
    }

    #[test]
    fn allowed_lists_are_stable() {
        assert_eq!(
            BehaviorAuthorityPolicy::allowed(BehaviorClass::PureDerivation),
            vec![
                BehaviorEffectKind::CommitCanonicalFact,
                BehaviorEffectKind::RecordEvidence
            ]
        );
        assert_eq!(
            BehaviorAuthorityPolicy::allowed(BehaviorClass::AgentBehavior).len(),
            4
        );
    }
}
