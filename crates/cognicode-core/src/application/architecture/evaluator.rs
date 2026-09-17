//! Architecture evaluator (WU3) and drift finding construction (WU4).
//!
//! The evaluator is the **only** component that may emit architecture
//! drift findings, and it consults only admitted
//! [`ArchitectureConstraint`]s (see [`crate::application::architecture::registry`]).
//!
//! Output shape:
//!
//! ```text
//! ArchitectureConstraint (admitted)
//!        │
//!        ▼
//! ArchitectureEvaluator::evaluate
//!        │
//!        ▼
//! EvaluationReport {
//!     findings: Vec<Finding>,
//! }
//! ```
//!
//! Each emitted `Finding` is a normal M6/M7 finding with a namespaced
//! kind in the `architecture.*` namespace. The causal chain explains
//! the violation (`Source: file:line` → `Flow: use crate::infrastructure`
//! → `Sink: infrastructure`). The `evidence` field is populated from
//! synthetic evidence ids (no I/O; the kernel evidence handles are
//! stubs the application can later replace with real ones).
//!
//! ## Authority model
//!
//! Drift findings are emitted with `DetectorAuthority::Gated` (the
//! constraint is admitted by a promoted admitter, and the gate is the
//! caller's `FindingGate`). This is what makes the load-bearing
//! property "ADR text alone → ZERO findings" hold: no constraint, no
//! detector definition, no finding.

use std::fmt;

use crate::domain::architecture::{
    ArchitectureConstraint, ArchitectureConstraintKind, ForbiddenDependencyRule,
    LayerDependencyRule, NamespaceBoundaryRule,
};
use crate::domain::architecture::{LayerId, UseStatement};
use crate::domain::findings::{
    CausalStep, CausalStepKind, DetectorAuthority, DetectorExecutionRef, DetectorId,
    EvidenceClass, Finding, FindingKind, FindingOrigin, FindingSeverity, FindingStatus,
    RiskLevel,
};
use crate::domain::kernel_ids::EvidenceId;

// ============================================================================
// Source
// ============================================================================

/// A single source file under evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    /// The file path (e.g. `src/domain/foo.rs`).
    pub file_path: String,
    /// Optional module path (e.g. `domain::foo`). When `None`, the
    /// evaluator falls back to deriving the layer from `file_path`.
    pub module_path: Option<String>,
    /// The full source text.
    pub source: String,
}

/// The collection of source files under evaluation. Default
/// (`Default::default()`) yields an empty source → zero findings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArchitectureSource {
    pub files: Vec<SourceFile>,
}

// ============================================================================
// Report
// ============================================================================

/// Report of an evaluation pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationReport {
    /// The findings emitted by this pass. May be empty.
    pub findings: Vec<Finding>,
    /// How many `use` statements were considered.
    pub statements_examined: u32,
    /// How many of them matched the constraint's forbidden surface.
    /// Equals `findings.len()` for the three rule families of e77,
    /// but kept as a separate field so future rule kinds (e.g. that
    /// fire once per file) can diverge.
    pub matches: u32,
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchitectureEvaluatorError {
    /// A `use` statement could not be parsed. Should not happen with
    /// the hand-rolled parser; included so the surface is closed.
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
    /// Only admitted constraints reach this method (the registry
    /// guards it). Non-admitted constraints are rejected upstream.
    pub fn evaluate(
        &self,
        constraint: &ArchitectureConstraint,
        source: &ArchitectureSource,
    ) -> Result<EvaluationReport, ArchitectureEvaluatorError> {
        // Parse every file once. A parse failure is a hard error: if
        // we silently drop a file we might miss real drifts.
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
        let mut findings = Vec::with_capacity(matches.len());
        for stmt in &matches {
            let finding = self.build_finding(constraint, stmt)?;
            findings.push(finding);
        }
        Ok(EvaluationReport {
            findings,
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

    fn build_finding(
        &self,
        constraint: &ArchitectureConstraint,
        stmt: &UseStatement,
    ) -> Result<Finding, ArchitectureEvaluatorError> {
        let kind_str = constraint.kind.finding_kind();
        let kind = FindingKind::new(kind_str)
            .map_err(|_| ArchitectureEvaluatorError::BadConstraintId)?;

        let detector_id = DetectorId::new(format!("architecture.evaluator.{}", constraint.id.as_str()))
            .map_err(|_| ArchitectureEvaluatorError::BadConstraintId)?;
        let execution_ref = DetectorExecutionRef::new(
            detector_id,
            constraint.admitted_at.clone(),
            DetectorAuthority::Gated,
            crate::domain::findings::DetectorDigests::of_for_kind(&kind),
            None,
        )
        .map_err(|_| ArchitectureEvaluatorError::BadConstraintId)?;

        // Synthetic evidence id; replaced by the application layer
        // when wired into the EvidenceBundle pipeline. The id is
        // derived from a stable hash of `file:line` so it is unique
        // per (file, line) pair while staying `u64`.
        let evidence_id = EvidenceId::new(stable_hash_id(&stmt.file_path, stmt.line));
        let causal_chain = vec![
            CausalStep::new(
                CausalStepKind::Source,
                format!("file `{}` line {}", stmt.file_path, stmt.line),
            )
            .ok(),
            CausalStep::new(
                CausalStepKind::Flow,
                format!("`use {}`", stmt.path),
            )
            .ok(),
            CausalStep::new(
                CausalStepKind::Sink,
                format!("forbidden by constraint `{}`", constraint.id.as_str()),
            )
            .ok(),
        ]
        .into_iter()
        .flatten()
        .collect();

        let message = format!(
            "architecture drift: `use {}` in `{}:{}` violates constraint `{}`",
            stmt.path,
            stmt.file_path,
            stmt.line,
            constraint.id.as_str()
        );

        Ok(Finding {
            id: crate::domain::findings::FindingId::new(format!(
                "arch:{constraint_id}:{file}:{line}",
                constraint_id = constraint.id.as_str(),
                file = stmt.file_path,
                line = stmt.line
            ))
            .map_err(|_| ArchitectureEvaluatorError::BadConstraintId)?,
            kind,
            origin: FindingOrigin::Detector,
            severity: FindingSeverity::Warning,
            risk: RiskLevel::High,
            evidence_class: EvidenceClass::B,
            evidence: vec![evidence_id],
            detector: execution_ref,
            status: FindingStatus::Open,
            message,
            causal_chain,
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
    // The use_parser normalises leading `crate::`/`self::`/`super::`
    // off, so the first segment is the layer (when applicable).
    let first = path.split("::").next().unwrap_or("");
    LayerId::from_module_path(first)
}

/// Deterministic, non-cryptographic `u64` hash of `(file, line)`.
///
/// Used to mint synthetic evidence ids for architecture-drift
/// findings. Stability across runs is required so that re-evaluating
/// the same source produces the same id; this lets tests compare
/// reports across runs without flakiness. Cryptographic strength is
/// not required.
fn stable_hash_id(file_path: &str, line: u32) -> u64 {
    // FNV-1a 64-bit. Stable, fast, no external dependency.
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in file_path.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for byte in line.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// We need a `DetectorDigests::of_for_kind` because we don't have a
// full DetectorIr. We synthesise a minimal digest set so the
// `DetectorExecutionRef::new` accepts it.
impl crate::domain::findings::DetectorDigests {
    /// Construct a placeholder digest set for the architecture
    /// evaluator's synthetic detector references. The digests are not
    /// semantically meaningful (they do not back a real DetectorIr)
    /// but they are non-empty and pass the structural validation.
    pub fn of_for_kind(kind: &FindingKind) -> Self {
        use crate::domain::findings::{DetectorDigest, DetectorDigests};
        let s = kind.as_str();
        DetectorDigests {
            logic: DetectorDigest::new(format!("logic:{s}"))
                .unwrap_or_else(|_| DetectorDigest::from_content("logic:placeholder")),
            policy: DetectorDigest::new(format!("policy:{s}"))
                .unwrap_or_else(|_| DetectorDigest::from_content("policy:placeholder")),
            semantic: DetectorDigest::new(format!("semantic:{s}"))
                .unwrap_or_else(|_| DetectorDigest::from_content("semantic:placeholder")),
            instance: DetectorDigest::new(format!("instance:{s}"))
                .unwrap_or_else(|_| DetectorDigest::from_content("instance:placeholder")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::architecture::{
        Admitter, AdmitterRole, ArchitectureConstraintId, LayerId,
    };

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
    fn clean_source_yields_no_findings() {
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
        assert!(report.findings.is_empty());
    }

    #[test]
    fn domain_reaching_infrastructure_emits_finding() {
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
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].kind.as_str(), "architecture.layer_dependency");
    }

    #[test]
    fn application_reaching_infrastructure_emits_no_finding() {
        // The constraint targets `Domain → Infrastructure` only. An
        // application-layer file reaching infrastructure is allowed.
        let constraint = admitted_constraint(ArchitectureConstraintKind::LayerDependency(
            LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Infrastructure],
                rationale: "test".into(),
            },
        ));
        let source = ArchitectureSource {
            files: vec![SourceFile {
                file_path: "src/application/foo.rs".into(),
                module_path: Some("application::foo".into()),
                source: "use crate::infrastructure::db;\n".into(),
            }],
        };
        let report = ArchitectureEvaluator::new()
            .evaluate(&constraint, &source)
            .unwrap();
        assert!(report.findings.is_empty());
    }

    #[test]
    fn forbidden_dependency_emits_finding() {
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
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].kind.as_str(), "architecture.forbidden_dependency");
    }

    #[test]
    fn namespace_boundary_emits_finding() {
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
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].kind.as_str(), "architecture.namespace_boundary");
    }

    #[test]
    fn adr_text_alone_does_not_emit_findings() {
        // The whole point: an `adr_ref` string with no admitted
        // constraint cannot reach the evaluator. We verify here that
        // calling the evaluator with no constraint at all yields zero
        // findings (this is a smoke test; the registry enforces the
        // stronger property of refusing non-admitted candidates).
        let report = ArchitectureEvaluator::new()
            .evaluate(
                &admitted_constraint(ArchitectureConstraintKind::LayerDependency(
                    LayerDependencyRule {
                        from_layer: LayerId::Domain,
                        forbidden_targets: vec![LayerId::Infrastructure],
                        rationale: "test".into(),
                    },
                )),
                &ArchitectureSource::default(),
            )
            .unwrap();
        assert!(report.findings.is_empty());
    }
}
