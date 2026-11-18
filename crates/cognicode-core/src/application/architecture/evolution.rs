//! Compares completed architecture evaluations across immutable revisions.
//!
//! This projection reuses the canonical read model; it does not evaluate
//! constraints, infer causality, or grant gate authority.

use std::collections::{BTreeMap, BTreeSet};

use crate::application::architecture::control_query::wire_canonical_control_query;
use crate::application::architecture::{
    ArchitectureReadModel, ArchitectureSource, EvaluationStatus, SourceFile, ViolationRef,
};
use crate::domain::metrics::{METRICS_RULES_VERSION, MetricsCoverage, MetricsOutcome};
use crate::domain::scm::{BlobRead, ContentDigest, PathKey, RevisionReader, SourceRevision};
use crate::infrastructure::native_metrics::measure_by_extension;

const MAX_REVISION_SOURCE_FILES: usize = 256;
const MAX_REVISION_FILE_BYTES: u64 = 256 * 1024;
const MAX_REVISION_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const REVISION_READ_BATCH_SIZE: usize = 16;

/// The bounded source coverage used for one immutable revision's evaluation.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct RevisionArchitectureCoverage {
    /// Opaque provider revision handle.
    pub revision_ref: String,
    /// SCM content identity for the full tracked revision.
    pub content_digest: String,
    /// The exact source subset considered by this query.
    pub scope: String,
    /// Rust source files found in the tracked inventory.
    pub tracked_source_files: usize,
    /// Rust source files successfully decoded and evaluated.
    pub evaluated_source_files: usize,
    /// Source bytes passed to the architecture evaluator.
    pub bytes_read: usize,
    /// `full` only when every in-scope file was read within the budgets.
    pub coverage: String,
    /// Reasons this revision's architecture view is incomplete.
    pub gaps: Vec<String>,
    /// Constraint identifiers admitted into the evaluation.
    pub constraints: Vec<String>,
}

/// One revision's aggregate of native cyclomatic-complexity measurements.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct NativeMetricRevision {
    /// Opaque provider revision handle.
    pub revision_ref: String,
    /// SCM content identity for the full tracked revision.
    pub content_digest: String,
    /// `full` only when every in-scope source file was measured completely.
    pub coverage: String,
    /// Number of files with a complete metric result.
    pub measured_files: usize,
    /// Sum of per-function cyclomatic path counts; absent unless coverage is full.
    pub cyclomatic_paths: Option<u64>,
    /// Reasons this metric value is unavailable or incomplete.
    pub gaps: Vec<String>,
}

/// A native metric comparison with its producer and measurement semantics.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct NativeMetricEvolution {
    /// Stable measurement producer.
    pub producer: String,
    /// Version of the native metric rules.
    pub rules_version: String,
    /// Metric name.
    pub metric: String,
    /// Unit of the reported values.
    pub unit: String,
    /// Source subset the metric describes.
    pub scope: String,
    /// Whether both metric snapshots support a direct comparison.
    pub status: EvolutionStatus,
    /// Earlier revision measurement.
    pub base: NativeMetricRevision,
    /// Later revision measurement.
    pub candidate: NativeMetricRevision,
    /// Candidate minus base, present only when both sides are complete.
    pub delta: Option<i64>,
}

/// One architecture evolution query and the evidence coverage for both sides.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct ArchitectureEvolutionQuery {
    /// Stable identity of the CogniCode query contract.
    pub producer: String,
    /// The canonical architecture read models' observed difference.
    pub evolution: ArchitectureEvolution,
    /// Evolution of the existing native cyclomatic-complexity metric.
    pub metric_evolution: NativeMetricEvolution,
    /// Evidence basis for the earlier revision.
    pub base: RevisionArchitectureCoverage,
    /// Evidence basis for the later revision.
    pub candidate: RevisionArchitectureCoverage,
}

struct RevisionSourceRead {
    source: ArchitectureSource,
    coverage: RevisionArchitectureCoverage,
}

/// Reads tracked Rust source from the requested immutable revision under a
/// fixed file, per-file, batch, and total-byte budget.
///
/// Failed or over-budget reads remain explicit gaps; callers must not treat a
/// partial source set as a clean architecture evaluation.
pub fn query_architecture_evolution(
    reader: &dyn RevisionReader,
    base_revision: &SourceRevision,
    candidate_revision: &SourceRevision,
) -> ArchitectureEvolutionQuery {
    let base_read = read_revision_source(reader, base_revision);
    let candidate_read = read_revision_source(reader, candidate_revision);
    let service = wire_canonical_control_query();
    let base_workspace = revision_workspace(base_revision);
    let candidate_workspace = revision_workspace(candidate_revision);
    let mut base_model = service.query_architecture(
        &base_workspace,
        Some(base_revision.handle().as_str()),
        &base_read.source,
    );
    let mut candidate_model = service.query_architecture(
        &candidate_workspace,
        Some(candidate_revision.handle().as_str()),
        &candidate_read.source,
    );
    apply_source_coverage(&mut base_model, &base_read.coverage);
    apply_source_coverage(&mut candidate_model, &candidate_read.coverage);
    let metric_evolution = metric_evolution(
        &base_read,
        &candidate_read,
        base_workspace == candidate_workspace,
    );
    let base_coverage = coverage_with_constraints(base_read.coverage, &base_model);
    let candidate_coverage = coverage_with_constraints(candidate_read.coverage, &candidate_model);

    ArchitectureEvolutionQuery {
        producer: "cognicode.application.architecture.ControlQueryService".into(),
        evolution: compare_architecture(&base_model, &candidate_model),
        metric_evolution,
        base: base_coverage,
        candidate: candidate_coverage,
    }
}

fn revision_workspace(revision: &SourceRevision) -> String {
    format!(
        "{}:{}",
        revision.source().provider().as_str(),
        revision.source().project()
    )
}

fn read_revision_source(
    reader: &dyn RevisionReader,
    revision: &SourceRevision,
) -> RevisionSourceRead {
    let mut coverage = RevisionArchitectureCoverage {
        revision_ref: revision.handle().as_str().to_string(),
        content_digest: revision.content().as_hex().to_string(),
        scope: "tracked Rust source files".into(),
        tracked_source_files: 0,
        evaluated_source_files: 0,
        bytes_read: 0,
        coverage: "full".into(),
        gaps: Vec::new(),
        constraints: Vec::new(),
    };
    let mut source = ArchitectureSource::default();
    let inventory = match reader.inventory(revision) {
        Ok(inventory)
            if inventory.revision_handle() == revision.handle()
                && inventory.content_digest() == revision.content().clone() =>
        {
            inventory
        }
        Ok(inventory) if inventory.revision_handle() != revision.handle() => {
            coverage
                .gaps
                .push("inventory revision identity mismatch".into());
            coverage.coverage = "partial".into();
            return RevisionSourceRead { source, coverage };
        }
        Ok(_) => {
            coverage
                .gaps
                .push("inventory content digest does not match revision identity".into());
            coverage.coverage = "partial".into();
            return RevisionSourceRead { source, coverage };
        }
        Err(error) => {
            coverage
                .gaps
                .push(format!("revision inventory unavailable: {error}"));
            coverage.coverage = "partial".into();
            return RevisionSourceRead { source, coverage };
        }
    };
    let mut paths: Vec<PathKey> = inventory
        .entries()
        .iter()
        .filter(|entry| entry.path.as_str().ends_with(".rs"))
        .map(|entry| entry.path.clone())
        .collect();
    coverage.tracked_source_files = paths.len();
    if paths.len() > MAX_REVISION_SOURCE_FILES {
        paths.truncate(MAX_REVISION_SOURCE_FILES);
        coverage.gaps.push(format!(
            "source file budget exceeded: {} tracked Rust files, limit is {MAX_REVISION_SOURCE_FILES}",
            coverage.tracked_source_files
        ));
    }

    'batches: for batch in paths.chunks(REVISION_READ_BATCH_SIZE) {
        let answers = reader.contents(revision, batch, MAX_REVISION_FILE_BYTES);
        if answers.len() != batch.len() {
            coverage.gaps.push(format!(
                "provider returned {} answers for {} requested source files",
                answers.len(),
                batch.len()
            ));
        }
        for (path, answer) in batch.iter().zip(answers) {
            let bytes = match answer {
                Ok(BlobRead::Bytes(bytes)) => bytes,
                Ok(BlobRead::TooLarge { bytes }) => {
                    coverage.gaps.push(format!(
                        "{} exceeds the per-file limit ({bytes} bytes)",
                        path.as_str()
                    ));
                    continue;
                }
                Err(error) => {
                    coverage
                        .gaps
                        .push(format!("{} could not be read: {error}", path.as_str()));
                    continue;
                }
            };
            if bytes.len() as u64 > MAX_REVISION_FILE_BYTES {
                coverage.gaps.push(format!(
                    "{} exceeds the per-file limit ({} bytes)",
                    path.as_str(),
                    bytes.len()
                ));
                continue;
            }
            let Some(expected) = inventory.find(path) else {
                coverage.gaps.push(format!(
                    "{} is absent from its revision inventory",
                    path.as_str()
                ));
                continue;
            };
            if expected.content != ContentDigest::from_bytes(&bytes) {
                coverage.gaps.push(format!(
                    "{} content does not match its revision inventory digest",
                    path.as_str()
                ));
                continue;
            }
            if coverage.bytes_read.saturating_add(bytes.len()) > MAX_REVISION_SOURCE_BYTES {
                coverage.gaps.push(format!(
                    "total source byte budget exceeded: limit is {MAX_REVISION_SOURCE_BYTES} bytes"
                ));
                break 'batches;
            }
            let text = match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    coverage
                        .gaps
                        .push(format!("{} is not UTF-8 source text", path.as_str()));
                    continue;
                }
            };
            coverage.bytes_read += text.len();
            coverage.evaluated_source_files += 1;
            source.files.push(SourceFile {
                file_path: path.as_str().to_string(),
                module_path: module_path(path.as_str()),
                source: text,
            });
        }
    }
    if !coverage.gaps.is_empty() {
        coverage.coverage = "partial".into();
    }
    RevisionSourceRead { source, coverage }
}

fn module_path(path: &str) -> Option<String> {
    let module = path.strip_suffix(".rs")?;
    let segments: Vec<_> = module.split('/').collect();
    let first_module = segments
        .iter()
        .rposition(|segment| *segment == "src")
        .map_or(0, |index| index + 1);
    let segments: Vec<_> = segments
        .into_iter()
        .skip(first_module)
        .filter(|segment| !matches!(*segment, "mod" | "lib" | "main"))
        .collect();
    (!segments.is_empty()).then(|| segments.join("::"))
}

fn apply_source_coverage(
    model: &mut ArchitectureReadModel,
    coverage: &RevisionArchitectureCoverage,
) {
    if coverage.coverage != "full" {
        model.status = EvaluationStatus::Incomplete;
        model
            .unevaluated_constraints
            .extend(coverage.gaps.iter().cloned());
    }
}

fn coverage_with_constraints(
    mut coverage: RevisionArchitectureCoverage,
    model: &ArchitectureReadModel,
) -> RevisionArchitectureCoverage {
    coverage.constraints = model
        .constraints
        .iter()
        .map(|constraint| constraint.id.clone())
        .collect();
    coverage
}

fn metric_evolution(
    base: &RevisionSourceRead,
    candidate: &RevisionSourceRead,
    same_source: bool,
) -> NativeMetricEvolution {
    let base_metric = measure_revision_cyclomatic(base);
    let candidate_metric = measure_revision_cyclomatic(candidate);
    let delta = if same_source {
        base_metric
            .cyclomatic_paths
            .zip(candidate_metric.cyclomatic_paths)
            .and_then(|(base, candidate)| {
                i64::try_from(candidate)
                    .ok()?
                    .checked_sub(i64::try_from(base).ok()?)
            })
    } else {
        None
    };
    let status = if delta.is_some() {
        EvolutionStatus::Evaluated
    } else {
        EvolutionStatus::Unknown
    };
    let mut base_metric = base_metric;
    let mut candidate_metric = candidate_metric;
    if !same_source {
        base_metric
            .gaps
            .push("source provider or project differs between revisions".into());
        candidate_metric
            .gaps
            .push("source provider or project differs between revisions".into());
    }
    NativeMetricEvolution {
        producer: "cognicode.infrastructure.native_metrics".into(),
        rules_version: METRICS_RULES_VERSION.into(),
        metric: "cyclomatic_complexity_sum".into(),
        unit: "paths".into(),
        scope: "tracked Rust source files".into(),
        status,
        base: base_metric,
        candidate: candidate_metric,
        delta,
    }
}

fn measure_revision_cyclomatic(read: &RevisionSourceRead) -> NativeMetricRevision {
    let mut cyclomatic_paths = 0_u64;
    let mut measured_files = 0;
    let mut gaps = read.coverage.gaps.clone();
    if read.coverage.evaluated_source_files == 0 {
        gaps.push("no Rust source files were measured".into());
    }
    for file in &read.source.files {
        match measure_by_extension(&file.file_path, &file.source) {
            MetricsOutcome::Measured { metrics } => match metrics.coverage {
                MetricsCoverage::Complete => {
                    let file_paths = metrics
                        .functions
                        .iter()
                        .map(|function| u64::from(function.cyclomatic))
                        .sum::<u64>();
                    match cyclomatic_paths.checked_add(file_paths) {
                        Some(total) => {
                            cyclomatic_paths = total;
                            measured_files += 1;
                        }
                        None => gaps.push(format!(
                            "cyclomatic complexity sum overflowed at {}",
                            file.file_path
                        )),
                    }
                }
                MetricsCoverage::Partial { reasons } => gaps.extend(
                    reasons
                        .into_iter()
                        .map(|reason| format!("{}: {reason}", file.file_path)),
                ),
            },
            MetricsOutcome::Unsupported { reason, .. } | MetricsOutcome::Failed { reason } => {
                gaps.push(format!("{}: {reason}", file.file_path));
            }
        }
    }
    if read.coverage.coverage != "full" && gaps.is_empty() {
        gaps.push("source revision coverage is partial".into());
    }
    let full = gaps.is_empty();
    NativeMetricRevision {
        revision_ref: read.coverage.revision_ref.clone(),
        content_digest: read.coverage.content_digest.clone(),
        coverage: if full { "full" } else { "partial" }.into(),
        measured_files,
        cyclomatic_paths: full.then_some(cyclomatic_paths),
        gaps,
    }
}

/// Whether both revision evaluations support a meaningful comparison.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionStatus {
    /// Both revisions were fully evaluated under the same workspace and constraints.
    Evaluated,
    /// At least one revision lacks the identity or evidence needed to compare.
    Unknown,
}

/// The observed change in architecture violations between two revisions.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct ArchitectureEvolution {
    /// Workspace whose evaluations were compared.
    pub workspace_ref: String,
    /// Identity of the earlier evaluation, when supplied.
    pub base_snapshot_ref: Option<String>,
    /// Identity of the later evaluation, when supplied.
    pub candidate_snapshot_ref: Option<String>,
    /// Fail-closed comparison status.
    pub status: EvolutionStatus,
    /// Violations present only in the candidate evaluation.
    pub added_violations: Vec<ViolationRef>,
    /// Violations present only in the base evaluation.
    pub removed_violations: Vec<ViolationRef>,
    /// Scope and interpretation of the comparison.
    pub note: String,
}

/// Compares two canonical architecture read models without re-evaluating either.
///
/// Violation identity is the constraint and dependency path, not source line or
/// file. Moving a violation during a refactor therefore does not create a false
/// regression. If either evaluation is incomplete, the workspace or admitted
/// constraints differ, or a snapshot identity is absent, the result is Unknown
/// and publishes no added/removed verdicts. The observed history is correlation,
/// not evidence that one change caused another.
pub fn compare_architecture(
    base: &ArchitectureReadModel,
    candidate: &ArchitectureReadModel,
) -> ArchitectureEvolution {
    let note = "Historical co-evolution is correlation, not causation.".to_string();
    let same_constraints = constraint_keys(base) == constraint_keys(candidate);
    let comparable = base.workspace_ref == candidate.workspace_ref
        && base.snapshot_ref.is_some()
        && candidate.snapshot_ref.is_some()
        && base.status == EvaluationStatus::Evaluated
        && candidate.status == EvaluationStatus::Evaluated
        && same_constraints;

    if !comparable {
        return ArchitectureEvolution {
            workspace_ref: candidate.workspace_ref.clone(),
            base_snapshot_ref: base.snapshot_ref.clone(),
            candidate_snapshot_ref: candidate.snapshot_ref.clone(),
            status: EvolutionStatus::Unknown,
            added_violations: Vec::new(),
            removed_violations: Vec::new(),
            note: format!(
                "Architecture evolution is unknown because revision identity, workspace, evaluation completeness, or admitted constraints differ. {note}"
            ),
        };
    }

    let base_violations = violations_by_key(&base.violations);
    let candidate_violations = violations_by_key(&candidate.violations);
    let keys: BTreeSet<_> = base_violations
        .keys()
        .chain(candidate_violations.keys())
        .cloned()
        .collect();
    let mut added_violations = Vec::new();
    let mut removed_violations = Vec::new();
    for key in keys {
        let base_matches = base_violations.get(&key).map_or(&[][..], Vec::as_slice);
        let candidate_matches = candidate_violations
            .get(&key)
            .map_or(&[][..], Vec::as_slice);
        if candidate_matches.len() > base_matches.len() {
            added_violations.extend(candidate_matches[base_matches.len()..].iter().cloned());
        } else if base_matches.len() > candidate_matches.len() {
            removed_violations.extend(base_matches[candidate_matches.len()..].iter().cloned());
        }
    }

    ArchitectureEvolution {
        workspace_ref: candidate.workspace_ref.clone(),
        base_snapshot_ref: base.snapshot_ref.clone(),
        candidate_snapshot_ref: candidate.snapshot_ref.clone(),
        status: EvolutionStatus::Evaluated,
        added_violations,
        removed_violations,
        note,
    }
}

fn constraint_keys(model: &ArchitectureReadModel) -> BTreeSet<(String, String, Option<String>)> {
    model
        .constraints
        .iter()
        .map(|constraint| {
            (
                constraint.id.clone(),
                constraint.kind.clone(),
                constraint.adr_ref.clone(),
            )
        })
        .collect()
}

fn violations_by_key(violations: &[ViolationRef]) -> BTreeMap<(String, String), Vec<ViolationRef>> {
    let mut by_key: BTreeMap<(String, String), Vec<ViolationRef>> = BTreeMap::new();
    for violation in violations {
        by_key
            .entry((
                violation.constraint_id.clone(),
                violation.dependency_path.clone(),
            ))
            .or_default()
            .push(violation.clone());
    }
    for matches in by_key.values_mut() {
        matches.sort_by(|left, right| {
            (&left.file_path, left.line).cmp(&(&right.file_path, right.line))
        });
    }
    by_key
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::architecture::{ConstraintRef, EvaluationStatus};

    fn violation(file_path: &str, line: u32, dependency_path: &str) -> ViolationRef {
        ViolationRef {
            id: format!("violation-{dependency_path}"),
            constraint_id: "layer-boundary".into(),
            file_path: file_path.into(),
            line,
            dependency_path: dependency_path.into(),
            grounding_fact: None,
        }
    }

    fn evaluated(snapshot: &str, violations: Vec<ViolationRef>) -> ArchitectureReadModel {
        ArchitectureReadModel {
            workspace_ref: "workspace".into(),
            snapshot_ref: Some(snapshot.into()),
            status: EvaluationStatus::Evaluated,
            constraints: vec![ConstraintRef {
                id: "layer-boundary".into(),
                kind: "layer_dependency".into(),
                adr_ref: None,
            }],
            violations,
            statements_examined: 4,
            unevaluated_constraints: Vec::new(),
        }
    }

    #[test]
    fn module_paths_are_relative_to_the_nearest_src_root() {
        assert_eq!(
            module_path("crates/service/src/domain/adapter.rs"),
            Some("domain::adapter".into())
        );
        assert_eq!(module_path("src/lib.rs"), None);
    }

    #[test]
    fn line_and_module_moves_do_not_look_like_a_new_architecture_violation() {
        let base = evaluated("base", vec![violation("src/old.rs", 3, "ui->domain")]);
        let candidate = evaluated(
            "candidate",
            vec![violation("src/new/module.rs", 28, "ui->domain")],
        );

        let evolution = compare_architecture(&base, &candidate);

        assert_eq!(evolution.status, EvolutionStatus::Evaluated);
        assert!(evolution.added_violations.is_empty());
        assert!(evolution.removed_violations.is_empty());
        assert!(evolution.note.contains("correlation"));
    }

    #[test]
    fn repeated_dependency_paths_keep_their_occurrence_count() {
        let base = evaluated("base", vec![violation("src/a.rs", 3, "ui->domain")]);
        let candidate = evaluated(
            "candidate",
            vec![
                violation("src/a.rs", 3, "ui->domain"),
                violation("src/b.rs", 8, "ui->domain"),
            ],
        );

        let evolution = compare_architecture(&base, &candidate);

        assert_eq!(evolution.status, EvolutionStatus::Evaluated);
        assert_eq!(evolution.added_violations.len(), 1);
        assert!(evolution.removed_violations.is_empty());
    }

    #[test]
    fn a_new_violation_is_reported_only_when_both_revisions_were_evaluated() {
        let base = evaluated("base", Vec::new());
        let candidate = evaluated("candidate", vec![violation("src/lib.rs", 12, "ui->db")]);

        let evolution = compare_architecture(&base, &candidate);

        assert_eq!(evolution.status, EvolutionStatus::Evaluated);
        assert_eq!(evolution.added_violations.len(), 1);
        assert_eq!(evolution.added_violations[0].dependency_path, "ui->db");
    }

    #[test]
    fn incomplete_input_is_unknown_not_clean() {
        let base = evaluated("base", Vec::new());
        let mut candidate = evaluated("candidate", vec![violation("src/lib.rs", 12, "ui->db")]);
        candidate.status = EvaluationStatus::Incomplete;

        let evolution = compare_architecture(&base, &candidate);

        assert_eq!(evolution.status, EvolutionStatus::Unknown);
        assert!(evolution.added_violations.is_empty());
        assert!(evolution.note.contains("evaluation completeness"));
    }

    #[test]
    fn cross_workspace_input_is_unknown_not_clean() {
        let base = evaluated("base", Vec::new());
        let mut candidate = evaluated("candidate", vec![violation("src/lib.rs", 12, "ui->db")]);
        candidate.workspace_ref = "another-workspace".into();

        let evolution = compare_architecture(&base, &candidate);

        assert_eq!(evolution.status, EvolutionStatus::Unknown);
        assert!(evolution.added_violations.is_empty());
    }
}
