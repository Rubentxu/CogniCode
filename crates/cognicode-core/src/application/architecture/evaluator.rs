//! Architecture evaluator (e77.1, M10 corrective slice).
//!
//! The evaluator's **primary output** is a list of
//! [`ArchitectureViolation`]s, not a list of [`Finding`]s. The
//! distinction is load-bearing:
//!
//! * A violation is a *detection*. It carries enough navigation
//!   information (constraint, file, line, dependency path) for a
//!   human or an automated tool to act on, plus an **optional**
//!   [`GroundingRef`] that bridges to canonical evidence.
//! * A finding is a *gated conclusion*. It carries
//!   `DetectorAuthority`, `EvidenceId`s, `DetectorDigests`, a
//!   causal chain grounded in canonical evidence, and a status.
//!   Building a finding requires the canonical evidence pipeline:
//!   `ProducedEvidence → CanonicalEvidenceWriter → FindingAssembler
//!   → FindingVerifier`. Architecture observations that have not
//!   been routed through that pipeline are not findings.
//!
//! The split mirrors the path that every other detector finding
//! already follows (see `application::findings::kernel_bridge`).
//!
//! ```text
//! ArchitectureConstraint (admitted)
//!        │
//!        ▼
//! ArchitectureEvaluator::evaluate
//!        │
//!        ▼
//! EvaluationReport {
//!     violations: Vec<ArchitectureViolation>,
//! }
//! ```
//!
//! Assembly into `Finding`s is the caller's responsibility. The
//! `application::architecture::grounding` module (WU2) provides the
//! helper that wires violations through canonical evidence.
//!
//! ## Authority model
//!
//! The evaluator does **not** mint authority. A violation carries
//! no `DetectorAuthority` field at all. If a downstream consumer
//! wants a `Finding`, it must derive the authority from the
//! canonical path — which today cannot mint `Gated` without an
//! actual detector execution. This is the structural reason an
//! admitted constraint cannot, by itself, confer gate authority.

use std::fmt;

use crate::domain::architecture::{
    ArchitectureConstraint, ArchitectureConstraintKind, ArchitectureViolation, ForbiddenDependencyRule,
    LayerDependencyRule, NamespaceBoundaryRule, ViolationId,
};
use crate::domain::architecture::{LayerId, UseStatement};
use crate::domain::findings::FindingKind;

// ============================================================================
// Source
// ============================================================================

/// A single source file under evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub file_path: String,
    pub module_path: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArchitectureSource {
    pub files: Vec<SourceFile>,
}

// ============================================================================
// Report
// ============================================================================

/// Report of an evaluation pass. Carries violations only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationReport {
    /// The architecture observations the evaluator emitted.
    pub violations: Vec<ArchitectureViolation>,
    /// How many `use` statements were considered.
    pub statements_examined: u32,
    /// How many of them matched the constraint's forbidden surface.
    /// Equals `violations.len()` for the three rule families of e77.
    pub matches: u32,
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchitectureEvaluatorError {
    /// A `use` statement could not be parsed.
    ParseFailed(String),
    /// A constraint had a malformed id.
    BadConstraintId,
}

impl fmt::Display for ArchitectureEvaluatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseFailed(s) => write!(f, "use-line parse failed: {s}"),
            Self::BadConstraintId => f.write_str("bad constraint id"),
        }
    }
}

impl std::error::Error for ArchitectureEvaluatorError {}

// ============================================================================
// Evaluator
// ============================================================================

/// Stateless evaluator. Construct once, call many times.
#[derive(Debug, Default, Clone)]
pub struct ArchitectureEvaluator;

impl ArchitectureEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Evaluate an admitted constraint against the source.
    ///
    /// The evaluator only consults admitted constraints (the
    /// registry guards it). Non-admitted constraints are rejected
    /// upstream. The output is a list of [`ArchitectureViolation`]s
    /// — never [`Finding`](crate::domain::findings::Finding)s.
    pub fn evaluate(
        &self,
        constraint: &ArchitectureConstraint,
        source: &ArchitectureSource,
    ) -> Result<EvaluationReport, ArchitectureEvaluatorError> {
        let mut statements: Vec<UseStatement> = Vec::new();
        for file in &source.files {
            let mut parsed = crate::domain::architecture::parse_use_lines(&file.source, &file.file_path)
                .map_err(|e| ArchitectureEvaluatorError::ParseFailed(format!("{e:?}")))?;
            for stmt in &mut parsed {
                stmt.module_path = file.module_path.clone();
            }
            statements.extend(parsed);
        }
        let statements_examined = statements.len() as u32;
        let matches = match &constraint.kind {
            ArchitectureConstraintKind::LayerDependency(rule) => {
                self.find_layer_violations(rule, &statements)
            }
            ArchitectureConstraintKind::ForbiddenDependency(rule) => {
                self.find_forbidden_violations(rule, &statements)
            }
            ArchitectureConstraintKind::NamespaceBoundary(rule) => {
                self.find_boundary_violations(rule, &statements)
            }
        };
        let mut violations = Vec::with_capacity(matches.len());
        for stmt in &matches {
            let violation = self.build_violation(constraint, stmt)?;
            violations.push(violation);
        }
        Ok(EvaluationReport {
            violations,
            statements_examined,
            matches: matches.len() as u32,
        })
    }

    fn find_layer_violations(
        &self,
        rule: &LayerDependencyRule,
        statements: &[UseStatement],
    ) -> Vec<UseStatement> {
        statements
            .iter()
            .filter(|stmt| {
                let from_layer = layer_of(stmt);
                if from_layer != rule.from_layer {
                    return false;
                }
                let target_layer = target_layer_of(&stmt.path);
                rule.forbidden_targets.contains(&target_layer)
            })
            .cloned()
            .collect()
    }

    fn find_forbidden_violations(
        &self,
        rule: &ForbiddenDependencyRule,
        statements: &[UseStatement],
    ) -> Vec<UseStatement> {
        statements
            .iter()
            .filter(|stmt| {
                let from_layer = layer_of(stmt);
                if from_layer != rule.from_layer {
                    return false;
                }
                rule.forbidden_paths.iter().any(|p| stmt.path.starts_with(p))
            })
            .cloned()
            .collect()
    }

    fn find_boundary_violations(
        &self,
        rule: &NamespaceBoundaryRule,
        statements: &[UseStatement],
    ) -> Vec<UseStatement> {
        statements
            .iter()
            .filter(|stmt| {
                let module = stmt
                    .module_path
                    .clone()
                    .unwrap_or_else(|| stmt.file_path.clone());
                if !module.starts_with(&rule.caller_namespace) {
                    return false;
                }
                rule.forbidden_targets
                    .iter()
                    .any(|p| stmt.path.starts_with(p))
            })
            .cloned()
            .collect()
    }

    /// Construct an [`ArchitectureViolation`] from a matched
    /// statement. The violation carries an *optional*
    /// [`GroundingRef`] — when the canonical mapping is not yet
    /// available, it is `None` and the violation is explanatory
    /// only.
    ///
    /// The grounding is **never** filled in here. Filling in the
    /// grounding requires the canonical mapping (WU2), which is
    /// outside the evaluator's responsibility.
    fn build_violation(
        &self,
        constraint: &ArchitectureConstraint,
        stmt: &UseStatement,
    ) -> Result<ArchitectureViolation, ArchitectureEvaluatorError> {
        let finding_kind = FindingKind::new(constraint.kind.finding_kind())
            .map_err(|_| ArchitectureEvaluatorError::BadConstraintId)?;
        let from_layer = layer_of(stmt);
        let id = ViolationId::compute(
            &constraint.id,
            &stmt.file_path,
            stmt.line,
            &stmt.path,
        );
        Ok(ArchitectureViolation {
            id,
            constraint_id: constraint.id.clone(),
            finding_kind,
            file_path: stmt.file_path.clone(),
            module_path: stmt.module_path.clone(),
            line: stmt.line,
            dependency_path: stmt.path.clone(),
            from_layer,
            grounding: None,
            rationale: constraint.kind.rationale().to_string(),
        })
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn layer_of(stmt: &UseStatement) -> LayerId {
    if let Some(module) = &stmt.module_path {
        LayerId::from_module_path(module)
    } else {
        LayerId::from_module_path(&stmt.file_path)
    }
}

fn target_layer_of(path: &str) -> LayerId {
    let first = path.split("::").next().unwrap_or("");
    LayerId::from_module_path(first)
}

impl ArchitectureConstraintKind {
    /// Human-readable rationale for the rule. Used by the violation
    /// DTO so downstream consumers do not need to re-derive it.
    fn rationale(&self) -> &str {
        match self {
            Self::LayerDependency(rule) => &rule.rationale,
            Self::ForbiddenDependency(rule) => &rule.rationale,
            Self::NamespaceBoundary(rule) => &rule.rationale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::architecture::{Admitter, AdmitterRole, ArchitectureConstraintId, LayerId};

    fn admitted_constraint(rule: ArchitectureConstraintKind) -> ArchitectureConstraint {
        ArchitectureConstraint {
            id: ArchitectureConstraintId::new("architecture.test.rule").unwrap(),
            kind: rule,
            adr_ref: Some("ADR-046".into()),
            admitted_at: "t0".into(),
            admitted_by: Admitter {
                id: "human:test".into(),
                role: AdmitterRole::HumanPromoter,
            },
            admission_evidence: None,
        }
    }

    #[test]
    fn clean_source_yields_no_violations() {
        let constraint = admitted_constraint(ArchitectureConstraintKind::LayerDependency(
            LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Infrastructure],
                rationale: "test".into(),
            },
        ));
        let report = ArchitectureEvaluator::new()
            .evaluate(&constraint, &ArchitectureSource::default())
            .unwrap();
        assert!(report.violations.is_empty());
        assert_eq!(report.statements_examined, 0);
        assert_eq!(report.matches, 0);
    }

    #[test]
    fn domain_reaching_infrastructure_emits_violation() {
        let constraint = admitted_constraint(ArchitectureConstraintKind::LayerDependency(
            LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Infrastructure],
                rationale: "domain must not depend on infrastructure".into(),
            },
        ));
        let source = ArchitectureSource {
            files: vec![SourceFile {
                file_path: "src/domain/foo.rs".into(),
                module_path: Some("domain::foo".into()),
                source: "use crate::infrastructure::db;\n".into(),
            }],
        };
        let report = ArchitectureEvaluator::new()
            .evaluate(&constraint, &source)
            .unwrap();
        assert_eq!(report.violations.len(), 1);
        let v = &report.violations[0];
        assert_eq!(v.finding_kind.as_str(), "architecture.layer_dependency");
        assert_eq!(v.dependency_path, "infrastructure::db");
        assert_eq!(v.line, 1);
        assert_eq!(v.from_layer, LayerId::Domain);
        // No canonical grounding yet: the violation is explanatory only.
        assert!(v.grounding.is_none());
    }

    #[test]
    fn violation_id_is_deterministic() {
        let constraint = admitted_constraint(ArchitectureConstraintKind::LayerDependency(
            LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Infrastructure],
                rationale: "test".into(),
            },
        ));
        let source = ArchitectureSource {
            files: vec![SourceFile {
                file_path: "src/domain/foo.rs".into(),
                module_path: Some("domain::foo".into()),
                source: "use crate::infrastructure::db;\n".into(),
            }],
        };
        let a = ArchitectureEvaluator::new().evaluate(&constraint, &source).unwrap();
        let b = ArchitectureEvaluator::new().evaluate(&constraint, &source).unwrap();
        assert_eq!(a.violations.len(), b.violations.len());
        assert_eq!(a.violations[0].id, b.violations[0].id);
    }

    #[test]
    fn forbidden_dependency_emits_violation() {
        let constraint = admitted_constraint(ArchitectureConstraintKind::ForbiddenDependency(
            ForbiddenDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_paths: vec!["sqlx".into()],
                rationale: "domain has no I/O".into(),
            },
        ));
        let source = ArchitectureSource {
            files: vec![SourceFile {
                file_path: "src/domain/foo.rs".into(),
                module_path: Some("domain::foo".into()),
                source: "use sqlx::Pool;\n".into(),
            }],
        };
        let report = ArchitectureEvaluator::new()
            .evaluate(&constraint, &source)
            .unwrap();
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].finding_kind.as_str(), "architecture.forbidden_dependency");
    }

    #[test]
    fn namespace_boundary_emits_violation() {
        let constraint = admitted_constraint(ArchitectureConstraintKind::NamespaceBoundary(
            NamespaceBoundaryRule {
                caller_namespace: "domain::evidence_kernel".into(),
                forbidden_targets: vec!["presentation".into()],
                rationale: "evidence_kernel must not drive UI".into(),
            },
        ));
        let source = ArchitectureSource {
            files: vec![SourceFile {
                file_path: "src/domain/evidence_kernel/store.rs".into(),
                module_path: Some("domain::evidence_kernel::store".into()),
                source: "use crate::presentation::render;\n".into(),
            }],
        };
        let report = ArchitectureEvaluator::new()
            .evaluate(&constraint, &source)
            .unwrap();
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].finding_kind.as_str(), "architecture.namespace_boundary");
    }

    /// The e77 first slice exposed a `Finding` with `Gated` authority
    /// from the evaluator. The post-e77.1 evaluator must NOT expose
    /// `Gated` anywhere — type-level guarantee.
    #[test]
    fn report_does_not_mention_gated_authority() {
        let constraint = admitted_constraint(ArchitectureConstraintKind::LayerDependency(
            LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Infrastructure],
                rationale: "test".into(),
            },
        ));
        let source = ArchitectureSource {
            files: vec![SourceFile {
                file_path: "src/domain/foo.rs".into(),
                module_path: Some("domain::foo".into()),
                source: "use crate::infrastructure::db;\n".into(),
            }],
        };
        let report = ArchitectureEvaluator::new()
            .evaluate(&constraint, &source)
            .unwrap();
        let dbg = format!("{report:?}");
        assert!(!dbg.contains("Gated"), "report must not contain Gated: {dbg}");
        // The primary output type is `EvaluationReport`, not a finding
        // shape. We assert the absence of the `findings` field by
        // checking the field name explicitly.
        assert!(
            !dbg.contains("findings:"),
            "report must not have a `findings` field: {dbg}"
        );
        assert!(
            dbg.contains("violations:"),
            "report must have a `violations` field: {dbg}"
        );
    }
}
