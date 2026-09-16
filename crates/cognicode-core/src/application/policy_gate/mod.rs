//! e69 WU3 — `PolicyGate`.
//!
//! The gate is the **only authority** that turns an [`EvidenceBundle`]
//! into a structured [`PolicyDecision`]. It distinguishes hard
//! outcomes:
//!
//! - `Pass` — every required slot has `Evidence` of at least the
//!   declared grade, no failed/missing/unknown on any slot.
//! - `Warn` — required slots satisfied, but optional slots have
//!   failed/missing/unknown entries.
//! - `Block` — at least one required slot has `Failed`. Hard
//!   producer failures block the gate.
//! - `InsufficientEvidence` — at least one required slot has
//!   `Missing` or `Unknown`. The gate does NOT pass.
//!
//! ## Architecture invariants
//!
//! - Facts are canonical — the gate does not write to the Evidence
//!   Kernel.
//! - Evidence carries proof, never authority by itself — the gate
//!   decides **based on** evidence, not by minting it.
//! - Derived operational decisions are not Facts — `PolicyDecision`
//!   is derived.
//! - AI/plugins propose; they do not mint authority — the gate is the
//!   decision authority, not any producer.
//! - **Unknown/incomplete never becomes "safe"**: any MISSING/UNKNOWN
//!   on a required slot → `InsufficientEvidence`. Empty bundle →
//!   `InsufficientEvidence`. This is the load-bearing rule.
//!
//! ## Determinism
//!
//! The `reasons` vector is sorted by `(rule_name, slot_id)` so the
//! decision is byte-stable across runs. Two identical bundles under
//! the same spec produce the same decision and the same reasons.

use crate::application::evidence_bundle::{BundleEntry, EvidenceBundle, ProducerSlot};

/// A rule the gate checks against the bundle.
///
/// `required = true` slots participate in the gating decision; if they
/// are missing/unknown, the gate refuses to pass. `required = false`
/// slots are advisory — they may downgrade `Pass` to `Warn` but never
/// to `Pass-from-Missing`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateRule {
    /// Stable rule id (e.g. `"require:test-evidence"`). The reason
    /// log uses this for auditability.
    pub name: String,
    /// The slot this rule watches.
    pub slot: ProducerSlot,
    /// Minimum grade required for the Evidence entry to satisfy the
    /// rule. `None` means "any Evidence grade is acceptable".
    pub min_grade: Option<crate::domain::kernel_ids::EvidenceGrade>,
    /// Whether the slot is required.
    pub required: bool,
}

/// The full spec the gate evaluates against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicySpec {
    pub rules: Vec<GateRule>,
}

impl PolicySpec {
    pub fn new(rules: Vec<GateRule>) -> Self {
        Self { rules }
    }
}

/// The outcome of a gate evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyOutcome {
    /// Every required rule satisfied, no failed/missing/unknown on
    /// any slot (required or optional).
    Pass,
    /// Every required rule satisfied, but at least one optional
    /// slot has failed/missing/unknown. The gate has enough to make
    /// a decision; it just isn't a clean one.
    Warn,
    /// At least one required slot has `Failed`. Hard producer
    /// failure blocks the gate.
    Block,
    /// At least one required slot has `Missing` or `Unknown`, OR the
    /// bundle is empty. The gate refuses to pass on incomplete
    /// information.
    InsufficientEvidence,
}

impl std::fmt::Display for PolicyOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PolicyOutcome::Pass => "Pass",
            PolicyOutcome::Warn => "Warn",
            PolicyOutcome::Block => "Block",
            PolicyOutcome::InsufficientEvidence => "InsufficientEvidence",
        };
        f.write_str(s)
    }
}

/// A single reason a rule produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyReason {
    pub rule: String,
    pub slot: ProducerSlot,
    pub verdict: ReasonVerdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonVerdict {
    /// The required slot had Evidence with grade >= min_grade.
    Satisfied,
    /// The required slot had Evidence but its grade is below
    /// `min_grade`.
    BelowGrade,
    /// The required slot had a Failed entry.
    Failed,
    /// The required slot had a Missing entry.
    Missing,
    /// The required slot had an Unknown entry.
    Unknown,
    /// The slot was not in the bundle at all (treated as Missing
    /// for required, ignored for optional).
    Absent,
    /// The optional slot had a failed/missing/unknown entry (warn).
    OptionalDegraded,
}

/// The structured decision the gate returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    pub outcome: PolicyOutcome,
    pub reasons: Vec<PolicyReason>,
}

impl PolicyDecision {
    /// Shortcut: `outcome == Pass`.
    pub fn is_pass(&self) -> bool {
        self.outcome == PolicyOutcome::Pass
    }

    /// Shortcut: the gate refused to pass on incomplete information.
    pub fn is_insufficient(&self) -> bool {
        self.outcome == PolicyOutcome::InsufficientEvidence
    }
}

/// Evaluate the bundle against the spec and return a structured
/// decision.
///
/// The function is pure, deterministic, and total: any combination of
/// inputs produces exactly one [`PolicyDecision`].
pub fn evaluate(bundle: &EvidenceBundle, spec: &PolicySpec) -> PolicyDecision {
    let mut reasons: Vec<PolicyReason> = Vec::new();

    if bundle.is_empty() {
        return PolicyDecision {
            outcome: PolicyOutcome::InsufficientEvidence,
            reasons: vec![PolicyReason {
                rule: "<empty>".to_string(),
                slot: ProducerSlot {
                    source: crate::application::evidence_bundle::ProducerSource::Other,
                    slot_id: "<empty>".to_string(),
                },
                verdict: ReasonVerdict::Absent,
            }],
        };
    }

    for rule in &spec.rules {
        let entry = find_entry(bundle, &rule.slot);
        let verdict = match entry {
            Some(BundleEntry::Evidence { descriptor, .. }) => {
                if let Some(min) = rule.min_grade {
                    if grade_meets(descriptor.grade, min) {
                        ReasonVerdict::Satisfied
                    } else {
                        ReasonVerdict::BelowGrade
                    }
                } else {
                    ReasonVerdict::Satisfied
                }
            }
            Some(BundleEntry::ProducerFailed { .. }) => ReasonVerdict::Failed,
            Some(BundleEntry::ProducerMissing { .. }) => ReasonVerdict::Missing,
            Some(BundleEntry::ProducerUnknown { .. }) => ReasonVerdict::Unknown,
            None => ReasonVerdict::Absent,
        };

        reasons.push(PolicyReason {
            rule: rule.name.clone(),
            slot: rule.slot.clone(),
            verdict,
        });
    }

    // Outcome derivation.
    //
    // The order of checks encodes the load-bearing rule:
    //   Block > InsufficientEvidence > Warn > Pass.
    //
    // Any required-slot Failed → Block.
    // Any required-slot Missing/Unknown/Absent (or BelowGrade) →
    //   InsufficientEvidence. NEVER Pass.
    // Any optional-slot Failed/Missing/Unknown → Warn (if all
    //   required are Satisfied).
    // Otherwise → Pass.

    let mut any_required_failed = false;
    let mut any_required_insufficient = false;
    let mut any_optional_degraded = false;

    for (rule, reason) in spec.rules.iter().zip(reasons.iter()) {
        let required = rule.required;
        match reason.verdict {
            ReasonVerdict::Satisfied => {}
            ReasonVerdict::Failed if required => any_required_failed = true,
            ReasonVerdict::Missing
            | ReasonVerdict::Unknown
            | ReasonVerdict::Absent
            | ReasonVerdict::BelowGrade
                if required =>
            {
                any_required_insufficient = true;
            }
            ReasonVerdict::OptionalDegraded
            | ReasonVerdict::Failed
            | ReasonVerdict::Missing
            | ReasonVerdict::Unknown
            | ReasonVerdict::Absent
            | ReasonVerdict::BelowGrade
                if !required =>
            {
                any_optional_degraded = true;
            }
            _ => {}
        }
    }

    let outcome = if any_required_failed {
        PolicyOutcome::Block
    } else if any_required_insufficient {
        PolicyOutcome::InsufficientEvidence
    } else if any_optional_degraded {
        PolicyOutcome::Warn
    } else {
        PolicyOutcome::Pass
    };

    // Determinism: stable reasons order by (rule_name, slot_id).
    reasons.sort_by(|a, b| a.rule.cmp(&b.rule).then_with(|| a.slot.cmp(&b.slot)));

    PolicyDecision { outcome, reasons }
}

fn find_entry<'a>(bundle: &'a EvidenceBundle, slot: &ProducerSlot) -> Option<&'a BundleEntry> {
    bundle.entries.iter().find(|e| e.slot() == slot)
}

fn grade_meets(
    actual: crate::domain::kernel_ids::EvidenceGrade,
    min: crate::domain::kernel_ids::EvidenceGrade,
) -> bool {
    use crate::domain::kernel_ids::EvidenceGrade::*;
    // Partial order: Supports ≥ Corroborates; Refutes does not
    // satisfy any non-Refutes minimum. Treat Supports and
    // Corroborates as positive (any positive beats any positive
    // threshold). Refutes never satisfies a positive threshold;
    // a Refutes minimum is only satisfied by Refutes.
    match (actual, min) {
        (Refutes, Refutes) => true,
        (Supports, Supports) | (Corroborates, Corroborates) => true,
        // Conservative: in v1 we treat Supports and Corroborates as
        // mutually substitutable for "positive" thresholds.
        (Supports, Corroborates) | (Corroborates, Supports) => true,
        // Refutes never satisfies a positive minimum.
        (Refutes, Supports) | (Refutes, Corroborates) => false,
        // Positive never satisfies a Refutes minimum.
        (Supports, Refutes) | (Corroborates, Refutes) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::change_tracking::planner::WorkId;
    use crate::application::evidence_bundle::{
        EvidenceBundle, EvidenceBundleId, ProducerSlot, ProducerSource,
    };
    use crate::domain::evidence_kernel::ids::{EvidenceId, FactId, SnapshotId};
    use crate::domain::findings::ports::{EvidenceDescriptor, FactDescriptor, FactSlot};
    use crate::domain::kernel_ids::{EvidenceGrade, ExecutionId};
    use crate::domain::naming::NamespacedName;

    fn ev_descriptor(id: u64, fact: u64, grade: EvidenceGrade) -> EvidenceDescriptor {
        EvidenceDescriptor {
            id: EvidenceId(id),
            grade,
            fact: FactSlot::Resolved(FactDescriptor {
                id: FactId(fact),
                subject: None,
                snapshot: SnapshotId(1),
            }),
        }
    }

    fn slot(source: ProducerSource, id: &str) -> ProducerSlot {
        ProducerSlot {
            source,
            slot_id: id.to_string(),
        }
    }

    fn work(s: &str) -> WorkId {
        WorkId::new(NamespacedName::new(s).unwrap())
    }

    fn bundle(entries: Vec<BundleEntry>) -> EvidenceBundle {
        EvidenceBundle::new(
            EvidenceBundleId(1),
            work("ci.x"),
            ExecutionId(1),
            SnapshotId(1),
            entries,
        )
    }

    fn rule(name: &str, slot: ProducerSlot, required: bool) -> GateRule {
        GateRule {
            name: name.to_string(),
            slot,
            min_grade: None,
            required,
        }
    }

    fn required(name: &str, slot: ProducerSlot) -> GateRule {
        rule(name, slot, true)
    }

    fn optional(name: &str, slot: ProducerSlot) -> GateRule {
        rule(name, slot, false)
    }

    // =====================================================================
    // WU3 adversarial matrix.
    // =====================================================================

    #[test]
    fn empty_bundle_is_insufficient_evidence() {
        let b = bundle(vec![]);
        let spec = PolicySpec::new(vec![]);
        let d = evaluate(&b, &spec);
        assert_eq!(d.outcome, PolicyOutcome::InsufficientEvidence);
    }

    #[test]
    fn required_slot_satisfied_with_evidence_passes() {
        let s = slot(ProducerSource::CargoTest, "ok");
        let b = bundle(vec![BundleEntry::Evidence {
            slot: s.clone(),
            descriptor: ev_descriptor(1, 10, EvidenceGrade::Supports),
        }]);
        let spec = PolicySpec::new(vec![required("require:test", s)]);
        let d = evaluate(&b, &spec);
        assert_eq!(d.outcome, PolicyOutcome::Pass);
        assert_eq!(d.reasons.len(), 1);
        assert_eq!(d.reasons[0].verdict, ReasonVerdict::Satisfied);
    }

    #[test]
    fn required_slot_missing_is_insufficient_evidence_not_pass() {
        let s = slot(ProducerSource::CargoTest, "missing");
        let b = bundle(vec![]); // no entry at all
        let spec = PolicySpec::new(vec![required("require:test", s)]);
        let d = evaluate(&b, &spec);
        assert_eq!(
            d.outcome,
            PolicyOutcome::InsufficientEvidence,
            "absence of required evidence MUST NOT become Pass"
        );
        assert_eq!(d.reasons[0].verdict, ReasonVerdict::Absent);
    }

    #[test]
    fn required_slot_producer_missing_is_insufficient_evidence_not_pass() {
        let s = slot(ProducerSource::JustRecipe, "missing");
        let b = bundle(vec![BundleEntry::ProducerMissing {
            slot: s.clone(),
            why_unreachable: "tool not found".into(),
        }]);
        let spec = PolicySpec::new(vec![required("require:just", s)]);
        let d = evaluate(&b, &spec);
        assert_eq!(d.outcome, PolicyOutcome::InsufficientEvidence);
    }

    #[test]
    fn required_slot_producer_unknown_is_insufficient_evidence_not_pass() {
        let s = slot(ProducerSource::Cogh, "garbled");
        let b = bundle(vec![BundleEntry::ProducerUnknown {
            slot: s.clone(),
            detail: "garbled".into(),
        }]);
        let spec = PolicySpec::new(vec![required("require:cogh", s)]);
        let d = evaluate(&b, &spec);
        assert_eq!(d.outcome, PolicyOutcome::InsufficientEvidence);
    }

    #[test]
    fn required_slot_producer_failed_is_block() {
        let s = slot(ProducerSource::CargoTest, "boom");
        let b = bundle(vec![BundleEntry::ProducerFailed {
            slot: s.clone(),
            reason: "exit 1".into(),
            raw: Some("...".into()),
        }]);
        let spec = PolicySpec::new(vec![required("require:cargo", s)]);
        let d = evaluate(&b, &spec);
        assert_eq!(d.outcome, PolicyOutcome::Block);
    }

    #[test]
    fn optional_slot_failed_does_not_block_only_warns() {
        let req = slot(ProducerSource::CargoTest, "req");
        let opt = slot(ProducerSource::CargoTest, "opt");
        let b = bundle(vec![
            BundleEntry::Evidence {
                slot: req.clone(),
                descriptor: ev_descriptor(1, 10, EvidenceGrade::Supports),
            },
            BundleEntry::ProducerFailed {
                slot: opt.clone(),
                reason: "exit 1".into(),
                raw: None,
            },
        ]);
        let spec = PolicySpec::new(vec![
            required("require:cargo", req),
            optional("optional:other", opt),
        ]);
        let d = evaluate(&b, &spec);
        assert_eq!(
            d.outcome,
            PolicyOutcome::Warn,
            "optional Failed → Warn, never Block and never Pass"
        );
    }

    #[test]
    fn required_slot_block_beats_optional_slot_warn() {
        let req = slot(ProducerSource::CargoTest, "req");
        let opt = slot(ProducerSource::JustRecipe, "opt");
        let b = bundle(vec![
            BundleEntry::ProducerFailed {
                slot: req.clone(),
                reason: "exit 1".into(),
                raw: None,
            },
            BundleEntry::ProducerUnknown {
                slot: opt.clone(),
                detail: "?".into(),
            },
        ]);
        let spec = PolicySpec::new(vec![
            required("require:cargo", req),
            optional("optional:just", opt),
        ]);
        let d = evaluate(&b, &spec);
        assert_eq!(d.outcome, PolicyOutcome::Block);
    }

    #[test]
    fn below_grade_required_slot_is_insufficient_evidence() {
        let s = slot(ProducerSource::CargoTest, "low");
        // Refutes never satisfies a Supports minimum — Refutes is a
        // hard negative grade and is NOT interchangeable with the
        // positive grades (see grade_meets).
        let b = bundle(vec![BundleEntry::Evidence {
            slot: s.clone(),
            descriptor: ev_descriptor(1, 10, EvidenceGrade::Refutes),
        }]);
        let mut r = required("require:test", s);
        r.min_grade = Some(EvidenceGrade::Supports);
        let spec = PolicySpec::new(vec![r]);
        let d = evaluate(&b, &spec);
        assert_eq!(d.outcome, PolicyOutcome::InsufficientEvidence);
        assert_eq!(d.reasons[0].verdict, ReasonVerdict::BelowGrade);
    }

    #[test]
    fn satisfies_when_min_grade_is_met() {
        // Supports satisfies a Supports minimum (same-grade match
        // is documented in grade_meets).
        let s = slot(ProducerSource::CargoTest, "ok");
        let b = bundle(vec![BundleEntry::Evidence {
            slot: s.clone(),
            descriptor: ev_descriptor(1, 10, EvidenceGrade::Supports),
        }]);
        let mut r = required("require:test", s);
        r.min_grade = Some(EvidenceGrade::Supports);
        let spec = PolicySpec::new(vec![r]);
        let d = evaluate(&b, &spec);
        assert_eq!(d.outcome, PolicyOutcome::Pass);
    }

    #[test]
    fn refutes_minimum_is_satisfied_only_by_refutes() {
        let s = slot(ProducerSource::CargoTest, "r");
        // Supports does NOT satisfy a Refutes minimum.
        let b_supports = bundle(vec![BundleEntry::Evidence {
            slot: s.clone(),
            descriptor: ev_descriptor(1, 1, EvidenceGrade::Supports),
        }]);
        let mut r_refutes = required("require:test", s.clone());
        r_refutes.min_grade = Some(EvidenceGrade::Refutes);
        let d = evaluate(&b_supports, &PolicySpec::new(vec![r_refutes.clone()]));
        assert_eq!(d.outcome, PolicyOutcome::InsufficientEvidence);
        assert_eq!(d.reasons[0].verdict, ReasonVerdict::BelowGrade);
    }

    // =====================================================================
    // Invariants.
    // =====================================================================

    #[test]
    fn absence_of_required_evidence_never_yields_pass() {
        // For every kind of "non-Evidence" outcome on a required
        // slot, the result MUST NOT be Pass.
        let s = slot(ProducerSource::CargoTest, "x");
        let required_rule = required("require:x", s.clone());

        let entries_kinds = vec![
            ("empty-bundle", vec![]),
            (
                "missing-entry",
                vec![BundleEntry::ProducerMissing {
                    slot: s.clone(),
                    why_unreachable: "?".into(),
                }],
            ),
            (
                "unknown-entry",
                vec![BundleEntry::ProducerUnknown {
                    slot: s.clone(),
                    detail: "?".into(),
                }],
            ),
            (
                "failed-entry",
                vec![BundleEntry::ProducerFailed {
                    slot: s.clone(),
                    reason: "?".into(),
                    raw: None,
                }],
            ),
            (
                "wrong-slot-evidence",
                vec![BundleEntry::Evidence {
                    slot: slot(ProducerSource::CargoTest, "different"),
                    descriptor: ev_descriptor(1, 1, EvidenceGrade::Supports),
                }],
            ),
        ];

        for (label, entries) in entries_kinds {
            let b = bundle(entries);
            let d = evaluate(&b, &PolicySpec::new(vec![required_rule.clone()]));
            assert_ne!(
                d.outcome,
                PolicyOutcome::Pass,
                "case {label} must NOT yield Pass"
            );
        }
    }

    #[test]
    fn decision_is_deterministic_under_spec_reordering() {
        let s1 = slot(ProducerSource::CargoTest, "a");
        let s2 = slot(ProducerSource::JustRecipe, "b");
        let b = bundle(vec![
            BundleEntry::Evidence {
                slot: s1.clone(),
                descriptor: ev_descriptor(1, 1, EvidenceGrade::Supports),
            },
            BundleEntry::Evidence {
                slot: s2.clone(),
                descriptor: ev_descriptor(2, 2, EvidenceGrade::Corroborates),
            },
        ]);
        let spec1 = PolicySpec::new(vec![
            required("require:a", s1.clone()),
            required("require:b", s2.clone()),
        ]);
        let spec2 = PolicySpec::new(vec![
            required("require:b", s2.clone()),
            required("require:a", s1.clone()),
        ]);
        let d1 = evaluate(&b, &spec1);
        let d2 = evaluate(&b, &spec2);
        assert_eq!(d1, d2);
        assert_eq!(d1.outcome, PolicyOutcome::Pass);
    }

    #[test]
    fn decision_carries_a_reason_for_every_rule() {
        let s1 = slot(ProducerSource::CargoTest, "a");
        let s2 = slot(ProducerSource::JustRecipe, "b");
        let b = bundle(vec![BundleEntry::Evidence {
            slot: s1.clone(),
            descriptor: ev_descriptor(1, 1, EvidenceGrade::Supports),
        }]);
        let spec = PolicySpec::new(vec![required("require:a", s1), required("require:b", s2)]);
        let d = evaluate(&b, &spec);
        assert_eq!(d.reasons.len(), 2);
        // One satisfied, one absent → InsufficientEvidence.
        assert_eq!(d.outcome, PolicyOutcome::InsufficientEvidence);
    }
}
