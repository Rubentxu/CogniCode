//! Findings handlers — the published answer to «¿por qué se reporta este
//! hallazgo?».
//!
//! One tool answers it: `explain_finding`. The answer is not a
//! re-description of a finding. It carries the finding's kind, severity,
//! status, message, causal chain and the detector that produced it, and it
//! says whether the grounding behind it could be **verified**. A finding
//! whose grounding could not be verified is published with that stated, and
//! never as if it had one.
//!
//! The findings are not invented here. They come from the grounded finding
//! flow, which is the product's only path from a source file to a
//! [`Finding`]: the same admission, the
//! same detector execution, the same assembler and the same verifier. The
//! handler runs that flow over the requested file and projects what it says.
//!
//! # Why the tool is published only in the builds that carry the kernel
//!
//! The flow grounds its findings in the canonical evidence kernel, and that
//! kernel is an opt-in part of the build. A build without it cannot produce
//! a finding at all, so publishing the tool there would advertise a
//! capability that cannot answer. The catalogue declares the gate, the
//! dispatch arm carries the same one, and the output contract is derived
//! only when the tool is actually shipped.

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{HandlerContext, HandlerError, HandlerResult};

use crate::application::findings::grounded_finding_flow::run_grounded_finding_flow;
use crate::domain::evidence_kernel::bootstrap::bootstrap_registry;
use crate::domain::findings::digest::sha256_hex;
use crate::domain::findings::finding::{EvidenceClass, Finding, FindingGate, RiskLevel};
use crate::domain::kernel_ids::SnapshotId;
use crate::domain::value_objects::WorkspaceId;
use crate::infrastructure::evidence_kernel::{
    InMemoryEvidenceStore, InMemoryFactStore, InMemorySchemaRegistry,
};

// ============================================================================
// The published shape
// ============================================================================

/// Input for `explain_finding`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExplainFindingInput {
    /// The source file whose findings are explained.
    ///
    /// Relative to the workspace root, or absolute inside it. The path is
    /// validated before it is read: a path that leaves the workspace is
    /// refused, and a path that cannot be read is an error, not an empty
    /// answer.
    pub path: String,

    /// Only explain this finding.
    ///
    /// Absent means "every finding the pipeline reports for the file". The
    /// ids the answer publishes are the ones to ask about.
    #[serde(default)]
    pub finding_id: Option<String>,
}

/// What the answer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExplainOutcome {
    /// The pipeline reported the finding(s) and the answer explains them.
    Explained,
    /// The pipeline ran over the file and reported nothing.
    NoFindings,
    /// The pipeline ran and reported findings, but not the one that was
    /// asked for.
    NotReported,
}

/// Output of `explain_finding`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ExplainFindingOutput {
    /// What the answer is.
    pub outcome: ExplainOutcome,
    /// The file the answer is about, as it was asked for.
    pub path: String,
    /// The findings, each with its reason.
    ///
    /// Empty when `outcome` is `NoFindings` or `NotReported` — and in those
    /// cases the outcome says which of the two it is, so an empty list is
    /// never the whole answer.
    pub findings: Vec<FindingExplanation>,
    /// One line saying what the answer says.
    pub summary: String,
    /// What the run looked at, and what it could establish.
    pub coverage: FindingCoverage,
}

/// What the run looked at.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FindingCoverage {
    /// Source constructs the detector observed.
    pub constructs_observed: usize,
    /// Constructs whose canonical fact was resolved.
    pub constructs_grounded: usize,
    /// Constructs with no canonical fact, or with an ambiguous one.
    pub constructs_ungrounded: usize,
}

/// One finding, with the reason it is reported.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FindingExplanation {
    /// The finding's id, as the pipeline minted it.
    pub id: String,
    /// What was found.
    pub kind: String,
    /// Display severity.
    pub severity: String,
    /// Evaluated risk.
    pub risk: String,
    /// Lifecycle status.
    pub status: String,
    /// The human-readable message the detector produced.
    pub message: String,
    /// The strength of the supporting evidence.
    pub evidence_class: String,
    /// The causal chain, in order.
    pub causal_chain: Vec<CausalStepExplanation>,
    /// The detector as it was executed.
    pub detector: DetectorExplanation,
    /// The grounding behind the finding.
    pub grounding: GroundingExplanation,
    /// Whether the finding may block the gate.
    pub gateable: bool,
}

/// One step of a finding's causal chain.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CausalStepExplanation {
    /// The role the step plays in the explanation.
    pub kind: String,
    /// The human-readable detail of the step.
    pub detail: String,
    /// The canonical fact the step rests on, when it rests on one.
    pub fact: Option<u64>,
    /// The evidence handle the step rests on, when it rests on one.
    pub evidence: Option<u64>,
    /// The entity the step concerns, when it names one.
    pub subject: Option<u64>,
}

/// The detector as it was executed.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DetectorExplanation {
    /// The detector's id.
    pub id: String,
    /// The detector's version at production time.
    pub version: String,
    /// The authority the detector held when it produced the finding.
    pub authority: String,
    /// The digest of the detector's logic.
    pub logic_digest: String,
    /// The digest of the detector's finding policy.
    pub policy_digest: String,
    /// The digest of logic and policy together.
    pub semantic_digest: String,
    /// The digest of the whole definition.
    pub instance_digest: String,
}

/// The grounding behind a finding.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GroundingExplanation {
    /// Whether the grounding could be verified: the evidence resolves and
    /// the causal chain agrees with it.
    ///
    /// `false` with an empty evidence list means the finding claims nothing;
    /// `false` with a reason means it claims something that could not be
    /// resolved. The two are not the same, and the reason says which.
    pub verified: bool,
    /// Why the grounding could not be verified, when it could not.
    pub reason: Option<String>,
    /// The evidence handles the finding claims.
    pub evidence: Vec<u64>,
    /// The canonical facts the causal chain rests on.
    pub facts: Vec<u64>,
}

// ============================================================================
// The handler
// ============================================================================

/// Handler for `explain_finding` — why a finding is reported.
///
/// The tool answers for one source file at a time, because that is the unit
/// the findings pipeline runs over. It returns every finding the pipeline
/// reports for that file, each with the message, the causal chain and the
/// detector that produced it, plus whether the grounding behind it could be
/// verified.
#[cognicode_macros::aix_tool(
    name = "explain_finding",
    description = "Explain why a finding is reported. Takes the path of a source file; the findings pipeline runs over that file and the answer names each finding's kind, severity, status, message, causal chain and detector, and states whether the grounding behind it could be verified. A file with no findings says so instead of returning an empty list.",
    input_schema = ExplainFindingInput
)]
pub async fn handle_explain_finding(
    ctx: &HandlerContext,
    input: ExplainFindingInput,
) -> HandlerResult<ExplainFindingOutput> {
    let path = super::authorise_file_path(ctx, &input.path)?;
    let source = std::fs::read_to_string(&path)
        .map_err(|e| HandlerError::NotFound(format!("{} could not be read: {e}", input.path)))?;

    // The content hash is the file's own, so the provenance of the facts
    // the run commits points at the bytes that were analysed.
    let hash = format!("sha256:{}", sha256_hex(source.as_bytes()));

    let facts = fresh_fact_store()?;
    let evidence = Arc::new(InMemoryEvidenceStore::new());
    let workspace = WorkspaceId::default();
    let snapshot = SnapshotId::new(1);

    let report = run_grounded_finding_flow(
        facts,
        evidence,
        &workspace,
        snapshot,
        &path,
        &source,
        &hash,
        findings_gate(),
    )
    .await
    .map_err(|e| HandlerError::Internal(format!("the findings pipeline failed: {e}")))?;

    let explanations: Vec<FindingExplanation> = report
        .findings
        .iter()
        .enumerate()
        .map(|(index, finding)| {
            explain(
                finding,
                report.grounding.get(index),
                report.gateable.get(index),
            )
        })
        .collect();

    let coverage = FindingCoverage {
        constructs_observed: report.construct_count,
        constructs_grounded: report.grounded_count,
        constructs_ungrounded: report.ungrounded_count,
    };

    let (outcome, findings, summary) = match input.finding_id.as_deref() {
        Some(wanted) => {
            let selected: Vec<FindingExplanation> = explanations
                .iter()
                .filter(|f| f.id == wanted)
                .cloned()
                .collect();
            if selected.is_empty() {
                (
                    ExplainOutcome::NotReported,
                    Vec::new(),
                    format!(
                        "the pipeline reported {} finding(s) for {}, and none of them is `{}`",
                        explanations.len(),
                        input.path,
                        wanted
                    ),
                )
            } else {
                (
                    ExplainOutcome::Explained,
                    selected,
                    format!(
                        "the pipeline reported {} finding(s) for {}; `{}` is explained below",
                        explanations.len(),
                        input.path,
                        wanted
                    ),
                )
            }
        }
        None if explanations.is_empty() => (
            ExplainOutcome::NoFindings,
            Vec::new(),
            format!(
                "the findings pipeline ran over {} and reported no findings: the detector matched nothing in that file",
                input.path
            ),
        ),
        None => {
            let count = explanations.len();
            (
                ExplainOutcome::Explained,
                explanations,
                format!(
                    "the pipeline reported {} finding(s) for {}; each one is explained below",
                    count, input.path
                ),
            )
        }
    };

    Ok(ExplainFindingOutput {
        outcome,
        path: input.path,
        findings,
        summary,
        coverage,
    })
}

/// The gate the findings are judged against.
///
/// The gate decides whether a finding may *block*. It does not decide
/// whether the finding is reported, and it is not part of the reason: the
/// reason is the message and the causal chain, and the gate verdict travels
/// beside them.
fn findings_gate() -> FindingGate {
    FindingGate::new(EvidenceClass::C, RiskLevel::Low)
}

/// A fact store with the canonical vocabulary registered.
///
/// The registry is the schema the kernel validates facts against, and the
/// canonical `core:*` relations are part of it. Registering them is what
/// lets the run commit facts at all.
fn fresh_fact_store() -> Result<Arc<InMemoryFactStore>, HandlerError> {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).map_err(|e| {
        HandlerError::Internal(format!(
            "the canonical fact vocabulary did not register: {e:?}"
        ))
    })?;
    Ok(Arc::new(InMemoryFactStore::new(Arc::new(registry))))
}

/// Project one finding, with its grounding, into the published shape.
fn explain(
    finding: &Finding,
    grounding: Option<&crate::application::findings::grounded_finding_flow::FindingGrounding>,
    gateable: Option<&bool>,
) -> FindingExplanation {
    let grounding = grounding.cloned().unwrap_or_else(|| {
        crate::application::findings::grounded_finding_flow::FindingGrounding {
            verified: false,
            reason: Some(
                "the pipeline reported this finding without a grounding verdict".to_string(),
            ),
            evidence: Vec::new(),
            facts: Vec::new(),
        }
    });

    FindingExplanation {
        id: finding.id.as_str().to_string(),
        kind: finding.kind.as_str().to_string(),
        severity: finding.severity.to_string(),
        risk: finding.risk.to_string(),
        status: finding.status.to_string(),
        message: finding.message.clone(),
        evidence_class: finding.evidence_class.name().to_string(),
        causal_chain: finding
            .causal_chain
            .iter()
            .map(|step| CausalStepExplanation {
                kind: step.kind.name().to_string(),
                detail: step.detail.clone(),
                fact: step.fact.map(|f| f.get()),
                evidence: step.evidence.map(|e| e.get()),
                subject: step.subject.map(|s| s.get()),
            })
            .collect(),
        detector: DetectorExplanation {
            id: finding.detector.id.as_str().to_string(),
            version: finding.detector.version.clone(),
            authority: format!("{:?}", finding.detector.authority_at_execution).to_lowercase(),
            logic_digest: finding.detector.digests.logic.as_str().to_string(),
            policy_digest: finding.detector.digests.policy.as_str().to_string(),
            semantic_digest: finding.detector.digests.semantic.as_str().to_string(),
            instance_digest: finding.detector.digests.instance.as_str().to_string(),
        },
        grounding: GroundingExplanation {
            verified: grounding.verified,
            reason: grounding.reason,
            evidence: grounding.evidence.iter().map(|e| e.get()).collect(),
            facts: grounding.facts.iter().map(|f| f.get()).collect(),
        },
        gateable: gateable.copied().unwrap_or(false),
    }
}
