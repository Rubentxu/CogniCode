//! Discovery of the Terraform roots and Helm charts of a workspace, and the
//! pack run over them.
//!
//! Files are found through the project's one walk
//! ([`real_files`](crate::infrastructure::graph::source_files::real_files)).
//! Each Terraform root and each chart is then read by its own loader, so the
//! goal does not interpret either format.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{
    FamilyVerdict, FindingReport, GoalSecurity, HelmCoverage, HelmReport, MatrixRow, RenderReport,
    SecurityInput, TerraformCoverage, TerraformReport, Unanalysed, UnitReport, VerdictKind,
    VerdictTotals,
};
use crate::application::findings::{PackRun, RunAuthority, RunContext, SecurityPack};
use crate::domain::execution::CorrelationId;
use crate::domain::findings::semantic_model::{
    BlockingLevel, CapabilityEntry, ObservedUnit, PACK_VERSION, PackVerdict, SecurityFamily,
    SourceLanguage, StructuralFamily, Support,
};
use crate::domain::findings::{AnalysisScope, GroundingRef};
use crate::domain::iac::helm::{
    ChartAnalysis, ChartRenderer, RenderCoverage, RenderStatus, analyze_chart,
};
use crate::domain::iac::terraform::TfAnalysis;
use crate::domain::kernel_ids::{ExecutionId, SnapshotId};
use crate::domain::value_objects::WorkspaceId;
use crate::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;
use crate::infrastructure::graph::source_files::real_files;
use crate::infrastructure::helm::{RELEASE_NAME, read_chart};
use crate::infrastructure::terraform_project::load_terraform_project;

/// What this report does not claim, published with every answer.
const LIMITS: [&str; 6] = [
    "This report is not a gate: findings are produced as candidates and are not grounded in the project graph, so none of them is counted as blocking.",
    "not observed is scoped to the constructs a family models in the analysed unit; it is never a statement that the unit is safe.",
    "unknown means checks that could not decide remain (an unbound variable, a module that was not opened, a chart that was not rendered); it is not a negative.",
    "unsupported means the pack has no model for that language and family: nothing was analysed for the pair.",
    "Terraform is read statically from configuration text: no terraform run, no provider, no remote module and no credential is involved.",
    "Helm charts are rendered with their default values only, by the official engine when it is installed; a vendored subchart is judged through its parent chart.",
];

pub(super) fn security(
    workspace: &Path,
    renderer: &dyn ChartRenderer,
    input: &SecurityInput,
) -> Result<GoalSecurity, String> {
    if !workspace.is_dir() {
        return Err(format!("{} is not a directory", workspace.display()));
    }
    let discovered = discover(workspace);
    let pack = SecurityPack::standard();
    let mut runner = Runner::new(&pack);
    let mut unanalysed = Vec::new();

    let terraform = terraform_reports(
        workspace,
        &discovered.terraform,
        &mut runner,
        &mut unanalysed,
    );
    let helm = helm_reports(
        workspace,
        &discovered.charts,
        renderer,
        &mut runner,
        &mut unanalysed,
    );

    let totals = runner.totals;
    let summary = summary(terraform.len(), helm.len(), unanalysed.len(), &totals);
    Ok(GoalSecurity {
        summary,
        schema_version: "cognicode.goal.security/v1".to_string(),
        goal: "security".to_string(),
        pack_version: PACK_VERSION.to_string(),
        terraform,
        helm,
        unanalysed,
        totals,
        matrix: matrix_rows(&pack, input.all_languages),
        limits: LIMITS.iter().map(|limit| (*limit).to_string()).collect(),
    })
}

fn summary(terraform: usize, helm: usize, unanalysed: usize, totals: &VerdictTotals) -> String {
    let skipped = if unanalysed > 0 {
        format!("; {unanalysed} project(s) could not be analysed")
    } else {
        String::new()
    };
    if terraform + helm == 0 {
        let why = if unanalysed > 0 {
            "every Terraform root or Helm chart found failed to load"
        } else {
            "no Terraform root or Helm chart was found"
        };
        return format!("Security pack: nothing was analysed, {why}{skipped}.");
    }
    format!(
        "Security pack: {terraform} Terraform root(s) and {helm} Helm chart(s); over the \
         unit x family pairs: {} found, {} not observed, {} unknown, {} unsupported{skipped}.",
        totals.found, totals.not_observed, totals.unknown, totals.unsupported
    )
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

struct Discovered {
    /// Directories with at least one `.tf` file, relative to the workspace.
    terraform: BTreeSet<PathBuf>,
    /// Directories with a `Chart.yaml` that are not inside another chart.
    charts: BTreeSet<PathBuf>,
}

fn discover(workspace: &Path) -> Discovered {
    let mut terraform = BTreeSet::new();
    let mut charts = BTreeSet::new();
    for file in real_files(workspace) {
        let Ok(relative) = file.strip_prefix(workspace) else {
            continue;
        };
        let directory = relative.parent().map(Path::to_path_buf).unwrap_or_default();
        match file.file_name().and_then(|name| name.to_str()) {
            Some("Chart.yaml") => {
                charts.insert(directory);
            }
            Some(name) if name.ends_with(".tf") => {
                terraform.insert(directory);
            }
            _ => {}
        }
    }
    let top_level: BTreeSet<PathBuf> = charts
        .iter()
        .filter(|chart| {
            !charts
                .iter()
                .any(|other| other != *chart && chart.starts_with(other))
        })
        .cloned()
        .collect();
    Discovered {
        terraform,
        charts: top_level,
    }
}

/// A directory as the report names it: relative to the workspace, `.` for the
/// workspace itself.
fn shown(relative: &Path) -> String {
    if relative.as_os_str().is_empty() {
        ".".to_string()
    } else {
        relative.to_string_lossy().into_owned()
    }
}

// ---------------------------------------------------------------------------
// The pack run
// ---------------------------------------------------------------------------

fn no_grounding(_path: &str, _line: u32) -> Option<GroundingRef> {
    None
}

struct Runner<'a> {
    pack: &'a SecurityPack,
    store: InMemoryEvidenceStore,
    scope: AnalysisScope,
    next_execution: u64,
    totals: VerdictTotals,
}

impl<'a> Runner<'a> {
    fn new(pack: &'a SecurityPack) -> Self {
        let scope = AnalysisScope::new(
            WorkspaceId::try_new("workspace").expect("a constant workspace id is valid"),
            SnapshotId::new(1),
        );
        Self {
            pack,
            store: InMemoryEvidenceStore::with_scope(scope.clone()),
            scope,
            next_execution: 1,
            totals: VerdictTotals::default(),
        }
    }

    fn context(&mut self) -> RunContext {
        let execution_id = ExecutionId::new(self.next_execution);
        self.next_execution += 1;
        RunContext {
            scope: self.scope.clone(),
            execution_id,
            correlation: CorrelationId::new("goal.security")
                .expect("a constant correlation id is valid"),
            authority: RunAuthority::Candidate,
        }
    }

    /// Every structural family over one unit. A family without a model for the
    /// unit's language comes back as `unsupported`; a run that could not be
    /// carried out comes back as `unknown`. Neither is ever a negative.
    fn unit(&mut self, unit: &ObservedUnit) -> UnitReport {
        let families = StructuralFamily::ALL
            .into_iter()
            .map(|family| {
                let context = self.context();
                let verdict =
                    match self
                        .pack
                        .analyse_structural(family, unit, &context, &mut self.store)
                    {
                        Ok(run) => verdict_of(&run),
                        Err(error) => FamilyVerdict {
                            family: word(&SecurityFamily::of_structural(family)),
                            verdict: VerdictKind::Unknown,
                            detail: format!("the run could not be carried out: {error}"),
                            unresolved: Vec::new(),
                            findings: Vec::new(),
                            diagnostics: Vec::new(),
                        },
                    };
                self.count(verdict.verdict);
                verdict
            })
            .collect();
        UnitReport {
            path: unit.path.clone(),
            families,
        }
    }

    fn count(&mut self, kind: VerdictKind) {
        let slot = match kind {
            VerdictKind::Found => &mut self.totals.found,
            VerdictKind::NotObserved => &mut self.totals.not_observed,
            VerdictKind::Unknown => &mut self.totals.unknown,
            VerdictKind::Unsupported => &mut self.totals.unsupported,
        };
        *slot += 1;
    }
}

/// The wire word of a value that serialises to a string.
fn word<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn verdict_of(run: &PackRun) -> FamilyVerdict {
    let (verdict, detail, unresolved) = match &run.verdict {
        PackVerdict::Found { findings, .. } => (
            VerdictKind::Found,
            format!("{findings} finding(s)"),
            Vec::new(),
        ),
        PackVerdict::NotObserved { within } => {
            (VerdictKind::NotObserved, within.clone(), Vec::new())
        }
        PackVerdict::Unknown { unresolved } => (
            VerdictKind::Unknown,
            format!("{} check(s) could not decide", unresolved.len()),
            unresolved.clone(),
        ),
        PackVerdict::Unsupported { reason } => {
            (VerdictKind::Unsupported, reason.clone(), Vec::new())
        }
    };
    FamilyVerdict {
        family: word(&run.family),
        verdict,
        detail,
        unresolved,
        findings: run
            .findings
            .iter()
            .map(|finding| FindingReport {
                kind: finding.kind.as_str().to_string(),
                risk: word(&finding.risk),
                evidence_class: word(&finding.evidence_class),
                message: finding.message.clone(),
            })
            .collect(),
        diagnostics: run
            .diagnostics
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Terraform
// ---------------------------------------------------------------------------

/// The Terraform roots of the candidate directories.
///
/// A directory that an earlier root opened as a local module is part of that
/// root, not a root of its own. An ancestor sorts before its descendants, so a
/// module is always reached after the root that calls it.
fn terraform_reports(
    workspace: &Path,
    candidates: &BTreeSet<PathBuf>,
    runner: &mut Runner<'_>,
    unanalysed: &mut Vec<Unanalysed>,
) -> Vec<TerraformReport> {
    let mut covered: BTreeSet<PathBuf> = BTreeSet::new();
    let mut reports = Vec::new();
    for relative in candidates {
        if covered.contains(relative) {
            continue;
        }
        let loaded = match load_terraform_project(&workspace.join(relative)) {
            Ok(loaded) => loaded,
            Err(error) => {
                unanalysed.push(Unanalysed {
                    path: shown(relative),
                    kind: "terraform".to_string(),
                    reason: error.to_string(),
                });
                continue;
            }
        };
        covered.extend(loaded.project.modules.keys().map(|key| relative.join(key)));
        let analysis = TfAnalysis::run(&loaded.project, "", loaded.bindings);
        let units = crate::domain::iac::terraform::observed_units(&analysis, &no_grounding)
            .into_iter()
            .map(|mut unit| {
                unit.path = relative.join(&unit.path).to_string_lossy().into_owned();
                runner.unit(&unit)
            })
            .collect();
        reports.push(TerraformReport {
            root: shown(relative),
            coverage: terraform_coverage(&analysis),
            units,
        });
    }
    reports
}

fn terraform_coverage(analysis: &TfAnalysis) -> TerraformCoverage {
    let coverage = analysis.coverage();
    let unbound: Vec<String> = analysis
        .unbound_variables()
        .iter()
        .map(ToString::to_string)
        .collect();
    TerraformCoverage {
        complete: coverage.is_complete() && unbound.is_empty(),
        resources: coverage.resources,
        assessed: coverage.assessed,
        not_evaluated_types: coverage.not_evaluated_types.iter().cloned().collect(),
        unresolved_modules: coverage
            .unresolved_modules
            .iter()
            .map(|module| format!("{} ({})", module.address, module.source))
            .collect(),
        unread: coverage
            .unread
            .iter()
            .map(|source| format!("{}: {}", source.path, source.reason))
            .collect(),
        unbound_variables: unbound,
    }
}

// ---------------------------------------------------------------------------
// Helm
// ---------------------------------------------------------------------------

fn helm_reports(
    workspace: &Path,
    charts: &BTreeSet<PathBuf>,
    renderer: &dyn ChartRenderer,
    runner: &mut Runner<'_>,
    unanalysed: &mut Vec<Unanalysed>,
) -> Vec<HelmReport> {
    let mut reports = Vec::new();
    for relative in charts {
        let model = match read_chart(&workspace.join(relative)) {
            Ok(model) => model,
            Err(error) => {
                unanalysed.push(Unanalysed {
                    path: shown(relative),
                    kind: "helm".to_string(),
                    reason: error.to_string(),
                });
                continue;
            }
        };
        let analysis = analyze_chart(&model, &[], RELEASE_NAME, renderer);
        let units = crate::domain::iac::helm::observed_units(&analysis, &no_grounding)
            .iter()
            .map(|unit| runner.unit(unit))
            .collect();
        reports.push(HelmReport {
            path: shown(relative),
            chart: analysis.chart.clone(),
            version: analysis.chart_version.clone(),
            render: render_report(&analysis.render),
            coverage: helm_coverage(&analysis),
            limits: analysis.limits.clone(),
            units,
        });
    }
    reports
}

fn render_report(status: &RenderStatus) -> RenderReport {
    match status {
        RenderStatus::Rendered { documents } => RenderReport {
            status: "rendered".to_string(),
            documents: Some(*documents),
            cause: None,
        },
        RenderStatus::NotRendered(cause) => RenderReport {
            status: "not_rendered".to_string(),
            documents: None,
            cause: Some(cause.describe()),
        },
    }
}

fn helm_coverage(analysis: &ChartAnalysis) -> HelmCoverage {
    let (state, reasons) = match &analysis.coverage {
        RenderCoverage::Complete => ("complete", Vec::new()),
        RenderCoverage::Partial { reasons } => {
            ("partial", reasons.iter().map(|r| r.describe()).collect())
        }
        RenderCoverage::Unknown { reasons } => {
            ("unknown", reasons.iter().map(|r| r.describe()).collect())
        }
    };
    HelmCoverage {
        state: state.to_string(),
        reasons,
    }
}

// ---------------------------------------------------------------------------
// The capability matrix
// ---------------------------------------------------------------------------

fn matrix_rows(pack: &SecurityPack, all_languages: bool) -> Vec<MatrixRow> {
    pack.capability_matrix()
        .entries
        .iter()
        .filter(|entry| {
            all_languages
                || matches!(
                    entry.language,
                    SourceLanguage::Terraform | SourceLanguage::Helm
                )
        })
        .map(matrix_row)
        .collect()
}

fn matrix_row(entry: &CapabilityEntry) -> MatrixRow {
    let (support, backend, evidence_ceiling, can_block, reason) = match &entry.support {
        Support::Supported {
            backend,
            evidence_ceiling,
            can_block,
            ..
        } => (
            "supported",
            Some(backend.clone()),
            Some(word(evidence_ceiling)),
            *can_block,
            None,
        ),
        Support::Unsupported { reason } => ("unsupported", None, None, false, Some(reason.clone())),
    };
    MatrixRow {
        language: entry.language.name().to_string(),
        family: word(&entry.family),
        support: support.to_string(),
        algorithm: entry.algorithm.clone(),
        backend,
        evidence_ceiling,
        can_block,
        nature: word(&entry.nature),
        blocking: match entry.blocking {
            BlockingLevel::AdvisoryOnly => "advisory_only".to_string(),
            BlockingLevel::Gated {
                min_evidence_class,
                min_risk,
            } => format!(
                "gated: evidence class {} or stronger and risk {} or higher",
                word(&min_evidence_class),
                word(&min_risk)
            ),
        },
        reason,
        limits: entry.limits.clone(),
    }
}
