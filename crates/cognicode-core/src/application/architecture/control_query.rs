//! Control Plane read model for executable architecture (CP1.0 WU2/WU3).
//!
//! Answers exactly one product question today:
//!
//! > "What is the architecture state of workspace W at snapshot S?"
//!
//! Design invariants (CP0 carry-forward):
//!
//! * **Read-only, authority-free, stateless.** Nothing here admits a
//!   constraint, mints a permit, or persists anything.
//! * **Projects existing truth** — [`crate::application::architecture`]
//!   surfaces — it never re-implements the evaluator.
//! * **Fail-closed semantics**: `unknown != clean`, `incomplete !=
//!   compliant`, and `zero violations != success` unless the evaluation
//!   actually completed ([`EvaluationStatus`]).
//! * DTOs carry **references** (ids, paths, grounding facts), never
//!   copies of canonical truth.

use crate::application::architecture::ArchitectureRegistry;
use crate::application::architecture::evaluator::{ArchitectureSource, SourceFile};
use crate::domain::architecture::{ArchitectureConstraint, ArchitectureConstraintKind};

/// Overall status of a read projection. The discriminator that keeps
/// the semantics fail-closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationStatus {
    /// The evaluation ran to completion over the supplied source.
    /// Only then does `violations: []` mean "no violations found".
    Evaluated,
    /// A required canonical input was unavailable (no admitted
    /// constraints, empty source) or an evaluation errored.
    /// `violations` is meaningless here.
    Incomplete,
}

/// A projected constraint reference. Identity + summary only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConstraintRef {
    pub id: String,
    pub kind: String,
    pub adr_ref: Option<String>,
}

/// A projected violation. References and coordinates only — the
/// canonical record stays in the domain/evidence kernel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ViolationRef {
    /// Deterministic violation id (display form).
    pub id: String,
    /// The constraint that fired.
    pub constraint_id: String,
    /// Source coordinates.
    pub file_path: String,
    pub line: u32,
    /// The dependency path that triggered the rule.
    pub dependency_path: String,
    /// Grounding fact id, when the violation is grounded. `None`
    /// means the violation is explanatory-only and cannot gate.
    #[serde(rename = "grounding_fact_id", skip_serializing_if = "Option::is_none")]
    pub grounding_fact: Option<String>,
}

/// The Control Plane read projection for one workspace architecture
/// query. Emitted by [`ControlQueryService::query_architecture`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArchitectureReadModel {
    /// Workspace reference (identity only; the caller already owns it).
    pub workspace_ref: String,
    /// Human-readable snapshot/analysis identity, if the caller has one.
    pub snapshot_ref: Option<String>,
    /// Fail-closed discriminator. See [`EvaluationStatus`].
    pub status: EvaluationStatus,
    /// Constraints that took part in the evaluation (admitted only).
    pub constraints: Vec<ConstraintRef>,
    /// Violations found. Empty ONLY when `status == Evaluated`.
    pub violations: Vec<ViolationRef>,
    /// Number of `use` statements actually examined.
    pub statements_examined: u64,
    /// Constraints that could not be evaluated (parse errors etc.).
    /// Non-empty implies `status == Incomplete`.
    pub unevaluated_constraints: Vec<String>,
}

/// Canonical kind name for a constraint kind (projected, not derived at runtime).
fn kind_name(kind: &ArchitectureConstraintKind) -> String {
    match kind {
        ArchitectureConstraintKind::LayerDependency(_) => "layer_dependency".into(),
        ArchitectureConstraintKind::ForbiddenDependency(_) => "forbidden_dependency".into(),
        ArchitectureConstraintKind::NamespaceBoundary(_) => "namespace_boundary".into(),
    }
}

/// Read-only, authority-free Control Plane query boundary (CP1.0 WU3).
///
/// HTTP handlers must go through this boundary, never directly to a
/// canonical store. The service is stateless: it composes a registry
/// (admission set + evaluator) with the source supplied per query.
#[derive(Debug)]
pub struct ControlQueryService {
    registry: ArchitectureRegistry,
}

impl ControlQueryService {
    /// Compose over the registry wired by the host. The registry owns
    /// the admission set; this service never mutates it. There is no
    /// `Default`: an empty registry would silently read as
    /// `Incomplete`, so hosts must wire it explicitly.
    pub fn new(registry: ArchitectureRegistry) -> Self {
        Self { registry }
    }

    /// Borrow the underlying registry (for hosts that need to wire
    /// admission receipts). Read-only escape hatch.
    pub fn registry(&self) -> &ArchitectureRegistry {
        &self.registry
    }

    /// Answer: "What is the architecture state of workspace W at
    /// snapshot S?"
    ///
    /// * Evaluates every **admitted** constraint against `source`.
    /// * A parse failure in any constraint demotes the whole projection
    ///   to [`EvaluationStatus::Incomplete`] while keeping the partial
    ///   results visible (never a silent `[]`).
    /// * No admitted constraints => [`EvaluationStatus::Incomplete`]
    ///   with empty `constraints` — NOT a clean verdict.
    pub fn query_architecture(
        &self,
        workspace_ref: &str,
        snapshot_ref: Option<&str>,
        source: &ArchitectureSource,
    ) -> ArchitectureReadModel {
        let admitted: Vec<ArchitectureConstraint> = self.registry.admission.admitted().to_vec();
        if admitted.is_empty() {
            return ArchitectureReadModel {
                workspace_ref: workspace_ref.to_string(),
                snapshot_ref: snapshot_ref.map(str::to_string),
                status: EvaluationStatus::Incomplete,
                constraints: Vec::new(),
                violations: Vec::new(),
                statements_examined: 0,
                unevaluated_constraints: Vec::new(),
            };
        }

        let mut constraints = Vec::with_capacity(admitted.len());
        let mut violations = Vec::new();
        let mut unevaluated = Vec::new();
        let mut statements_examined = 0u64;

        for constraint in &admitted {
            constraints.push(ConstraintRef {
                id: constraint.id.as_str().to_string(),
                kind: kind_name(&constraint.kind),
                adr_ref: constraint.adr_ref.clone(),
            });
            match self.registry.evaluate(constraint, source) {
                Ok(report) => {
                    statements_examined += report.statements_examined as u64;
                    for v in report.violations {
                        violations.push(ViolationRef {
                            id: v.id.to_string(),
                            constraint_id: v.constraint_id.as_str().to_string(),
                            file_path: v.file_path,
                            line: v.line,
                            dependency_path: v.dependency_path,
                            grounding_fact: v.grounding.as_ref().map(|g| g.fact.to_string()),
                        });
                    }
                }
                Err(_) => {
                    unevaluated.push(constraint.id.as_str().to_string());
                }
            }
        }

        let status = if unevaluated.is_empty() {
            EvaluationStatus::Evaluated
        } else {
            EvaluationStatus::Incomplete
        };

        ArchitectureReadModel {
            workspace_ref: workspace_ref.to_string(),
            snapshot_ref: snapshot_ref.map(str::to_string),
            status,
            constraints,
            violations,
            statements_examined,
            unevaluated_constraints: unevaluated,
        }
    }
}

/// Convenience constructor mirroring the evaluator input shape.
pub fn source_from_files(files: Vec<(String, Option<String>, String)>) -> ArchitectureSource {
    ArchitectureSource {
        files: files
            .into_iter()
            .map(|(file_path, module_path, source)| SourceFile {
                file_path,
                module_path,
                source,
            })
            .collect(),
    }
}

/// Build an [`ArchitectureSource`] by scanning the Rust source files of
/// a workspace source root. `module_path` is derived from the path
/// relative to the root (`src/domain/service.rs` -> `domain::service`),
/// which is what `LayerId::from_module_path` resolves against. Files
/// that cannot be read are skipped; the query fail-closed status
/// handles reduced coverage.
pub fn source_from_source_root(root: &std::path::Path) -> ArchitectureSource {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path
                    .file_name()
                    .is_some_and(|n| n == "target" || n == "node_modules")
                {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let Ok(source) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let rel = path.strip_prefix(root).unwrap_or(&path).with_extension("");
                let segments: Vec<String> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .filter(|seg| seg != "mod" && seg != "lib" && seg != "main" && seg != "src")
                    .collect();
                files.push(SourceFile {
                    file_path: path.display().to_string(),
                    module_path: (!segments.is_empty()).then(|| segments.join("::")),
                    source,
                });
            }
        }
    }
    ArchitectureSource { files }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::architecture::admission::ArchitectureAdmissionService;
    use crate::application::architecture::{
        ArchitectureClock, ArchitectureEvaluator, SystemArchitectureClock,
    };
    use crate::domain::architecture::{
        Admitter, AdmitterRole, ArchitectureConstraintId, ConstraintCandidate, LayerDependencyRule,
        LayerId,
    };

    fn promoted_admitter() -> Admitter {
        Admitter {
            id: "cp-test@human".into(),
            role: AdmitterRole::HumanPromoter,
        }
    }

    fn layer_candidate(id: &str) -> ConstraintCandidate {
        ConstraintCandidate {
            id: ArchitectureConstraintId::new(id).unwrap(),
            kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
                from_layer: LayerId::Domain,
                forbidden_targets: vec![LayerId::Infrastructure],
                rationale: "domain must not reach infrastructure".into(),
            }),
            adr_ref: None,
            proposed_by: "cp1-test".into(),
        }
    }

    fn service_with(candidate: ConstraintCandidate) -> ControlQueryService {
        let mut admission = ArchitectureAdmissionService::new();
        let clock = SystemArchitectureClock;
        let outcome = admission.admit(candidate, &promoted_admitter(), &clock);
        assert!(outcome.result.is_ok(), "admission must succeed in fixture");
        let registry = ArchitectureRegistry {
            admission,
            evaluator: ArchitectureEvaluator::new(),
        };
        ControlQueryService::new(registry)
    }

    fn source_with(use_line: &str) -> ArchitectureSource {
        source_from_files(vec![(
            "src/domain/service.rs".into(),
            // module_path must resolve via LayerId::from_module_path ("domain::...")
            Some("domain::service".into()),
            use_line.to_string(),
        )])
    }

    fn empty_source() -> ArchitectureSource {
        source_from_files(vec![(
            "src/domain/empty.rs".into(),
            Some("domain::empty".into()),
            String::new(),
        )])
    }

    #[test]
    fn no_admitted_constraints_is_incomplete_not_clean() {
        // C4 fail-closed: empty admission set must never read as
        // "zero violations => compliant".
        let svc = ControlQueryService::new(ArchitectureRegistry::new());
        let model = svc.query_architecture("ws-1", Some("snap-1"), &source_with("use std::io;"));
        assert_eq!(model.status, EvaluationStatus::Incomplete);
        assert!(model.constraints.is_empty());
        assert!(model.violations.is_empty());
    }

    #[test]
    fn admitted_and_clean_source_evaluates_with_zero_violations() {
        // C2: admitted constraint + clean source => Evaluated, [].
        let svc = service_with(layer_candidate("architecture.cp.domain_no_infra"));
        let model = svc.query_architecture("ws-1", None, &source_with("use std::collections;"));
        assert_eq!(model.status, EvaluationStatus::Evaluated);
        assert_eq!(model.constraints.len(), 1);
        assert_eq!(model.constraints[0].id, "architecture.cp.domain_no_infra");
        assert!(model.violations.is_empty());
        assert!(model.unevaluated_constraints.is_empty());
        assert_eq!(model.statements_examined, 1);
    }

    #[test]
    fn violation_is_projected_with_reference_not_truth() {
        // C3: real violation => projected reference with coordinates.
        let svc = service_with(layer_candidate("architecture.cp.domain_no_infra"));
        let model = svc.query_architecture(
            "ws-1",
            None,
            &source_with("use crate::infrastructure::db::Pool;"),
        );
        assert_eq!(model.status, EvaluationStatus::Evaluated);
        assert_eq!(model.violations.len(), 1);
        let v = &model.violations[0];
        assert_eq!(v.constraint_id, "architecture.cp.domain_no_infra");
        assert_eq!(v.file_path, "src/domain/service.rs");
        assert!(v.line >= 1);
        assert!(v.dependency_path.contains("infrastructure"));
        // Ungrounded violation: no grounding fact => cannot gate.
        assert!(v.grounding_fact.is_none());
    }

    #[test]
    fn parse_failure_is_incomplete_even_with_partial_violations() {
        // C4: evaluator error on any constraint => Incomplete, never [].
        let svc = service_with(layer_candidate("architecture.cp.bad_id_surface"));
        let model = svc.query_architecture("ws-1", None, &empty_source());
        assert_eq!(model.status, EvaluationStatus::Incomplete);
        assert_eq!(model.unevaluated_constraints.len(), 1);
    }

    #[test]
    fn service_is_read_only_no_admission_leak() {
        // C1/C5: the query boundary exposes no way to admit. Runtime
        // pin: a query must not change the admission set.
        let svc = service_with(layer_candidate("architecture.cp.readonly"));
        let before = svc.registry().admission.admitted().len();
        let _ = svc.query_architecture("ws", None, &source_with("use std::fmt;"));
        let after = svc.registry().admission.admitted().len();
        assert_eq!(before, after);
    }
}
