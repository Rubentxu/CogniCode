//! Admission service for architecture constraints (WU2).
//!
//! The fundamental rule (load-bearing property for e77):
//!
//! > An ADR by itself has ZERO execution/gating authority.
//! > Only an admitted `ArchitectureConstraint` may produce drift findings.
//!
//! Concretely, this means:
//!
//! * A bare [`ConstraintCandidate`] (i.e. a proposed rule that has not
//!   gone through [`ArchitectureAdmissionService::admit`]) **never**
//!   produces findings. The evaluator only consults admitted
//!   constraints.
//! * Admission requires a promoted admitter
//!   ([`Admitter::may_admit`] is `true`).
//! * Admission is idempotent on id: a second attempt with the same id
//!   is rejected with [`AdmissionError::AlreadyAdmitted`], **not**
//!   silently re-admitted.
//! * Every admission produces a [`ConstraintAdmission`] that records
//!   the disposition. The service does not return one without the
//!   other.
//!
//! Pure orchestration: no I/O, no `sqlx`/`tokio`. The clock is a port
//! ([`ArchitectureClock`]) so tests can fix the timestamp.

use std::fmt;

use crate::domain::architecture::{
    Admitter, ArchitectureConstraint, ConstraintAdmission, ConstraintAdmissionDisposition,
    ConstraintCandidate, ConstraintError,
};

/// Errors raised by the admission service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    /// The candidate id was empty or malformed.
    Malformed(ConstraintError),
    /// The candidate was rejected because the admitter is not
    /// promoted.
    AdmitterNotPromoted,
    /// A constraint with the same id was already admitted.
    AlreadyAdmitted,
    /// The rule body failed validation.
    InvalidRuleBody,
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Malformed(e) => return write!(f, "malformed candidate: {e}"),
            Self::AdmitterNotPromoted => "admitter is not promoted",
            Self::AlreadyAdmitted => "constraint already admitted",
            Self::InvalidRuleBody => "invalid rule body",
        })
    }
}

impl std::error::Error for AdmissionError {}

impl From<ConstraintError> for AdmissionError {
    fn from(e: ConstraintError) -> Self {
        Self::Malformed(e)
    }
}

/// Outcome of an admission attempt — paired with the rejected/empty
/// constraint for callers that want to log the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionOutcome {
    pub result: Result<ConstraintAdmission, AdmissionError>,
}

/// Clock port for the admission timestamp. Production code uses
/// [`SystemArchitectureClock`]; tests can fix the value.
pub trait ArchitectureClock {
    fn now(&self) -> String;
}

/// Real-world clock: returns an RFC3339-like string. The exact format
/// is opaque to callers — they should treat it as a token.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemArchitectureClock;

impl ArchitectureClock for SystemArchitectureClock {
    fn now(&self) -> String {
        // We deliberately do not pull in `chrono` / `time`; the
        // admission service does not need a calendar-aware format,
        // only a stable, monotonic-enough token. The string is opaque
        // to all downstream code; it shows up only in the audit log.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("admitted@{nanos}")
    }
}

/// The admission service. Holds the set of currently admitted
/// constraints and enforces the invariants above.
///
/// This is a **port-like façade**: it lives in the application layer
/// because it composes domain types and enforces orchestration rules,
/// but it has no I/O of its own. Adapters (e.g. a Postgres-backed
/// registry) can wrap it.
#[derive(Debug)]
pub struct ArchitectureAdmissionService {
    /// Idempotency set: ids of admitted constraints.
    admitted_ids: std::collections::BTreeSet<String>,
    /// The admitted constraints, in insertion order.
    admitted: Vec<ArchitectureConstraint>,
}

impl Default for ArchitectureAdmissionService {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchitectureAdmissionService {
    /// Construct an empty service.
    pub fn new() -> Self {
        Self {
            admitted_ids: std::collections::BTreeSet::new(),
            admitted: Vec::new(),
        }
    }

    /// Construct from a pre-existing list of admitted constraints.
    /// Used by the persistence adapter to hydrate from disk.
    pub fn from_admitted(constraints: Vec<ArchitectureConstraint>) -> Self {
        let mut admitted_ids = std::collections::BTreeSet::new();
        for c in &constraints {
            admitted_ids.insert(c.id.as_str().to_string());
        }
        Self {
            admitted_ids,
            admitted: constraints,
        }
    }

    /// Borrow the currently admitted constraints.
    pub fn admitted(&self) -> &[ArchitectureConstraint] {
        &self.admitted
    }

    /// Admit a candidate. Returns the resulting
    /// [`ConstraintAdmission`] on success, or the rejection reason on
    /// failure.
    ///
    /// The admitter **must** have `may_admit() == true`. Anything else
    /// is rejected with [`AdmissionError::AdmitterNotPromoted`].
    pub fn admit<C: ArchitectureClock>(
        &mut self,
        candidate: ConstraintCandidate,
        admitter: &Admitter,
        clock: &C,
    ) -> AdmissionOutcome {
        let id_str = candidate.id.as_str().to_string();
        if self.admitted_ids.contains(&id_str) {
            return AdmissionOutcome {
                result: Err(AdmissionError::AlreadyAdmitted),
            };
        }
        if !admitter.may_admit() {
            return AdmissionOutcome {
                result: Err(AdmissionError::AdmitterNotPromoted),
            };
        }
        if !rule_body_valid(&candidate) {
            return AdmissionOutcome {
                result: Err(AdmissionError::InvalidRuleBody),
            };
        }
        let constraint = ArchitectureConstraint {
            id: candidate.id,
            kind: candidate.kind,
            adr_ref: candidate.adr_ref,
            admitted_at: clock.now(),
            admitted_by: admitter.clone(),
            admission_evidence: None,
        };
        self.admitted_ids.insert(id_str);
        self.admitted.push(constraint.clone());
        AdmissionOutcome {
            result: Ok(ConstraintAdmission {
                constraint,
                disposition: ConstraintAdmissionDisposition::Admitted,
            }),
        }
    }
}

/// Validate the rule body for the three rule families.
fn rule_body_valid(candidate: &ConstraintCandidate) -> bool {
    use crate::domain::architecture::ArchitectureConstraintKind;
    match &candidate.kind {
        ArchitectureConstraintKind::LayerDependency(rule) => !rule.forbidden_targets.is_empty(),
        ArchitectureConstraintKind::ForbiddenDependency(rule) => !rule.forbidden_paths.is_empty(),
        ArchitectureConstraintKind::NamespaceBoundary(rule) => {
            !rule.caller_namespace.is_empty() && !rule.forbidden_targets.is_empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::architecture::{
        AdmitterRole, ArchitectureConstraintId, ArchitectureConstraintKind,
        ForbiddenDependencyRule, LayerDependencyRule, LayerId,
    };

    fn candidate_layer(id: &str) -> ConstraintCandidate {
        ConstraintCandidate {
            id: ArchitectureConstraintId::new(id).unwrap(),
            kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Infrastructure],
                rationale: "domain must not depend on infrastructure".into(),
            }),
            adr_ref: Some("ADR-046".into()),
            proposed_by: "human:test".into(),
        }
    }

    fn promoted_admitter() -> Admitter {
        Admitter {
            id: "human:test".into(),
            role: AdmitterRole::HumanPromoter,
        }
    }

    fn non_promoted_admitter() -> Admitter {
        Admitter {
            id: "ai:claude".into(),
            role: AdmitterRole::Other,
        }
    }

    struct FixedClock(&'static str);
    impl ArchitectureClock for FixedClock {
        fn now(&self) -> String {
            self.0.into()
        }
    }

    #[test]
    fn non_promoted_admitter_is_rejected() {
        let mut svc = ArchitectureAdmissionService::new();
        let out = svc.admit(
            candidate_layer("architecture.test.rule"),
            &non_promoted_admitter(),
            &FixedClock("t0"),
        );
        assert_eq!(out.result, Err(AdmissionError::AdmitterNotPromoted));
        assert!(svc.admitted().is_empty());
    }

    #[test]
    fn promoted_admitter_admits_once() {
        let mut svc = ArchitectureAdmissionService::new();
        let out = svc.admit(
            candidate_layer("architecture.test.rule"),
            &promoted_admitter(),
            &FixedClock("t0"),
        );
        assert!(out.result.is_ok());
        assert_eq!(svc.admitted().len(), 1);
        assert_eq!(svc.admitted()[0].admitted_at, "t0");

        // Idempotency: re-admitting the same id fails.
        let out2 = svc.admit(
            candidate_layer("architecture.test.rule"),
            &promoted_admitter(),
            &FixedClock("t1"),
        );
        assert_eq!(out2.result, Err(AdmissionError::AlreadyAdmitted));
        assert_eq!(svc.admitted().len(), 1);
    }

    #[test]
    fn invalid_rule_body_is_rejected() {
        let mut svc = ArchitectureAdmissionService::new();
        let candidate = ConstraintCandidate {
            id: ArchitectureConstraintId::new("architecture.empty.rule").unwrap(),
            kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![], // empty -> invalid
                rationale: "test".into(),
            }),
            adr_ref: None,
            proposed_by: "human:test".into(),
        };
        let out = svc.admit(candidate, &promoted_admitter(), &FixedClock("t0"));
        assert_eq!(out.result, Err(AdmissionError::InvalidRuleBody));
    }

    #[test]
    fn forbidden_dependency_admits() {
        let mut svc = ArchitectureAdmissionService::new();
        let candidate = ConstraintCandidate {
            id: ArchitectureConstraintId::new("architecture.no_sqlx_in_domain").unwrap(),
            kind: ArchitectureConstraintKind::ForbiddenDependency(ForbiddenDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_paths: vec!["sqlx".into()],
                rationale: "domain has no I/O".into(),
            }),
            adr_ref: Some("ADR-046".into()),
            proposed_by: "human:test".into(),
        };
        let out = svc.admit(candidate, &promoted_admitter(), &FixedClock("t0"));
        assert!(out.result.is_ok());
        assert_eq!(svc.admitted().len(), 1);
    }
}
