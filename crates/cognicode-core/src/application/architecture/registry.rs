//! Registry that ties together an admission service and the evaluator.
//!
//! The registry is the **only** entry point a caller should reach for.
//! It enforces the rule that only admitted constraints may produce
//! findings: candidates passed to [`ArchitectureRegistry::evaluate`]
//! are rejected before any work is done.
//!
//! ```text
//! ArchitectureRegistry
//!     ├── admission:  ArchitectureAdmissionService
//!     └── evaluator:  ArchitectureEvaluator
//! ```
//!
//! Pure orchestration (no I/O). Persistence adapters wrap this type.

use crate::application::architecture::admission::ArchitectureAdmissionService;
use crate::application::architecture::evaluator::{
    ArchitectureEvaluator, ArchitectureEvaluatorError, ArchitectureSource, EvaluationReport,
};
use crate::domain::architecture::{
    ArchitectureConstraint, ArchitectureViolation, ConstraintCandidate, TemporaryException,
};

/// Errors raised by the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchitectureRegistryError {
    /// The candidate is not admitted; the registry refuses to
    /// evaluate it.
    CandidateNotAdmitted,
    /// The evaluator failed.
    Evaluator(ArchitectureEvaluatorError),
}

impl std::fmt::Display for ArchitectureRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CandidateNotAdmitted => f.write_str(
                "candidate is not admitted; only admitted ArchitectureConstraint may produce drift findings",
            ),
            Self::Evaluator(e) => write!(f, "evaluator error: {e}"),
        }
    }
}

impl std::error::Error for ArchitectureRegistryError {}

impl From<ArchitectureEvaluatorError> for ArchitectureRegistryError {
    fn from(e: ArchitectureEvaluatorError) -> Self {
        Self::Evaluator(e)
    }
}

/// Composed registry.
#[derive(Debug)]
pub struct ArchitectureRegistry {
    pub admission: ArchitectureAdmissionService,
    pub evaluator: ArchitectureEvaluator,
    /// Time-bounded exceptions that suppress specific drift findings.
    /// See [`TemporaryException`] for the contract.
    temporary_exceptions: Vec<TemporaryException>,
}

impl Default for ArchitectureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchitectureRegistry {
    pub fn new() -> Self {
        Self {
            admission: ArchitectureAdmissionService::new(),
            evaluator: ArchitectureEvaluator::new(),
            temporary_exceptions: Vec::new(),
        }
    }

    /// Builder: install a pre-populated admission service. Used by
    /// tests that admit candidate fixtures out-of-band and want to
    /// re-use the populated set rather than re-admitting.
    pub fn with_admission(mut self, admission: ArchitectureAdmissionService) -> Self {
        self.admission = admission;
        self
    }

    /// Builder: install a complete set of temporary exceptions,
    /// replacing any that were previously installed.
    pub fn with_temporary_exceptions(mut self, exceptions: Vec<TemporaryException>) -> Self {
        self.temporary_exceptions = exceptions;
        self
    }

    /// Borrow the registered temporary exceptions.
    pub fn temporary_exceptions(&self) -> &[TemporaryException] {
        &self.temporary_exceptions
    }

    /// Evaluate a candidate. **Always rejected** unless the candidate
    /// id is present in the admission set.
    ///
    /// This is the load-bearing property: an ADR text or a bare
    /// candidate cannot reach the evaluator.
    pub fn evaluate_candidate(
        &self,
        candidate: &ConstraintCandidate,
        source: &ArchitectureSource,
    ) -> Result<EvaluationReport, ArchitectureRegistryError> {
        if !self.is_admitted(candidate) {
            return Err(ArchitectureRegistryError::CandidateNotAdmitted);
        }
        // Re-derive the constraint view the evaluator needs.
        let constraint = ArchitectureConstraint {
            id: candidate.id.clone(),
            kind: candidate.kind.clone(),
            adr_ref: candidate.adr_ref.clone(),
            // The timestamp and admitter are not used by the
            // evaluator; we pass placeholders to satisfy the type.
            admitted_at: "0".into(),
            admitted_by: crate::domain::architecture::Admitter {
                id: "registry@system".into(),
                role: crate::domain::architecture::AdmitterRole::CiPromoter,
            },
            admission_evidence: None,
        };
        self.evaluator
            .evaluate(&constraint, source)
            .map(|r| self.filter_exceptions(r))
            .map_err(Into::into)
    }

    /// Evaluate an already-admitted constraint.
    pub fn evaluate(
        &self,
        constraint: &ArchitectureConstraint,
        source: &ArchitectureSource,
    ) -> Result<EvaluationReport, ArchitectureEvaluatorError> {
        self.evaluator
            .evaluate(constraint, source)
            .map(|r| self.filter_exceptions(r))
    }

    /// Whether the candidate id is in the admission set.
    pub fn is_admitted(&self, candidate: &ConstraintCandidate) -> bool {
        self.admission
            .admitted()
            .iter()
            .any(|c| c.id.as_str() == candidate.id.as_str())
    }

    /// Filter out violations that match a non-expired temporary
    /// exception. The match delegates to
    /// [`TemporaryException::matches`], which prefixes
    /// `dependency_path` so that one allowlist entry can cover all
    /// leaf imports under an import group (see the type's doc).
    /// Expired exceptions are silently ignored — a stale allowlist is
    /// louder than a passing build: the same violation that was
    /// suppressed yesterday will be emitted today if the exception's
    /// expiry has passed.
    fn filter_exceptions(&self, mut report: EvaluationReport) -> EvaluationReport {
        if self.temporary_exceptions.is_empty() {
            return report;
        }
        let today = today_iso_date();
        let active: Vec<&TemporaryException> = self
            .temporary_exceptions
            .iter()
            .filter(|ex| !ex.is_expired(&today))
            .collect();
        report
            .violations
            .retain(|v: &ArchitectureViolation| !active.iter().any(|ex| ex.matches(v)));
        report
    }
}

/// Today's date in ISO-8601 `YYYY-MM-DD` form, computed from the
/// system clock (`std::time::SystemTime`). Returns the empty string
/// when the system clock is before the UNIX epoch (treated as "1970"
/// for expiry comparison purposes — i.e. effectively every exception
/// is expired, which is the safest default).
fn today_iso_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Civil-from-days algorithm (Howard Hinnant). Days since 1970-01-01 → (Y, M, D).
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::architecture::admission::SystemArchitectureClock;
    use crate::domain::architecture::{
        Admitter, AdmitterRole, ArchitectureConstraintId, ArchitectureConstraintKind,
        ForbiddenDependencyRule, LayerDependencyRule, LayerId,
    };

    fn make_promoted_admitter() -> Admitter {
        Admitter {
            id: "human:test".into(),
            role: AdmitterRole::HumanPromoter,
        }
    }

    fn make_candidate_layer(id: &str) -> ConstraintCandidate {
        ConstraintCandidate {
            id: ArchitectureConstraintId::new(id).unwrap(),
            kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Infrastructure],
                rationale: "test".into(),
            }),
            adr_ref: Some("ADR-046".into()),
            proposed_by: "human:test".into(),
        }
    }

    #[test]
    fn evaluate_candidate_rejects_when_not_admitted() {
        let registry = ArchitectureRegistry::new();
        let candidate = make_candidate_layer("architecture.test.rule");
        let report = registry.evaluate_candidate(&candidate, &ArchitectureSource::default());
        assert!(matches!(
            report,
            Err(ArchitectureRegistryError::CandidateNotAdmitted)
        ));
    }

    #[test]
    fn evaluate_admitted_constraint_returns_zero_findings_for_clean_source() {
        let mut registry = ArchitectureRegistry::new();
        let candidate = make_candidate_layer("architecture.test.layer");
        let out = registry.admission.admit(
            candidate.clone(),
            &make_promoted_admitter(),
            &SystemArchitectureClock,
        );
        assert!(out.result.is_ok());
        let constraint = registry
            .admission
            .admitted()
            .first()
            .expect("one admitted constraint")
            .clone();
        // The clean source has zero `use` statements → zero violations.
        let report = registry
            .evaluate(&constraint, &ArchitectureSource::default())
            .unwrap();
        assert!(report.violations.is_empty());
    }

    #[test]
    fn evaluate_admitted_constraint_rejects_source_with_violation() {
        let mut registry = ArchitectureRegistry::new();
        let candidate = ConstraintCandidate {
            id: ArchitectureConstraintId::new("architecture.no_infra_in_domain").unwrap(),
            kind: ArchitectureConstraintKind::ForbiddenDependency(ForbiddenDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_paths: vec!["infrastructure".into()],
                rationale: "domain has no I/O".into(),
            }),
            adr_ref: Some("ADR-046".into()),
            proposed_by: "human:test".into(),
        };
        let out = registry.admission.admit(
            candidate.clone(),
            &make_promoted_admitter(),
            &SystemArchitectureClock,
        );
        assert!(out.result.is_ok());
        let constraint = registry
            .admission
            .admitted()
            .first()
            .expect("one admitted constraint")
            .clone();
        let source = ArchitectureSource {
            files: vec![crate::application::architecture::evaluator::SourceFile {
                file_path: "src/domain/foo.rs".into(),
                module_path: Some("domain::foo".into()),
                source: "use crate::infrastructure::db;\n".into(),
            }],
        };
        let report = registry.evaluate(&constraint, &source).unwrap();
        assert_eq!(report.violations.len(), 1);
        assert_eq!(
            report.violations[0].finding_kind.as_str(),
            "architecture.forbidden_dependency"
        );
    }
}
