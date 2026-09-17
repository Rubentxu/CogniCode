//! FixAgent — bounded AI fix suggestion → `ChangeProposal` (e80b, M11 task 12.5).
//!
//! ## The load-bearing invariant
//!
//! ```text
//! AI may author a proposed source change
//!         ↓
//! ChangeProposal { requested_by = LlmAgent }
//!         ↓
//! STOP
//! ```
//!
//! The FixAgent never possesses or mints authority. Its terminal effect is the
//! creation and return of a `ChangeProposal`; testing, evaluation, authority,
//! and apply all live outside it.
//!
//! ## What the model may and may not choose
//!
//! The model produces a [`SourcePatchCandidate`]. The **trusted envelope** —
//! `ChangeProposalId`, `base_world`, and `RequestedBy` — comes from this
//! orchestrator's configuration and call site, never from model output.
//! In particular `requested_by` is always
//! `RequestedBy::LlmAgent { agent_ref: <configured> }`; there is no field in
//! the candidate through which a model could claim `Human`.
//!
//! ## Authority boundary (WU6)
//!
//! This module does NOT, and structurally cannot, call: `issue_promotion_permit`,
//! `PromotionAuthorizationPolicy::authorize`, `apply_with_permit`, any source
//! writer, any Git operation, or any canonical-fact/evidence/finding minter.
//! `authority_boundary_tests.rs` audits this.
//!
//! ## Separation (WU7)
//!
//! The FixAgent MUST NOT trial or evaluate its own proposal:
//!
//! ```text
//! authoring  !=  testing  !=  evaluation  !=  authority
//! ```
//!
//! `TrialExecutor` remains a distinct abstraction, driven by external
//! orchestration.
//!
//! ## Lineage (WU10)
//!
//! Every inference input is preserved in [`FixLineage`]: frame, request digest,
//! provider/model provenance, observed read count, and the configured agent
//! identity. The module emits no events: it has no event seam and introducing
//! an EventBus here would be architecture without a consumer. The typed
//! outcomes (`FixOutcome`, `FixAgentError`) are the adapter seam — a
//! composition-root caller with a real event log can map them onto
//! `ai.fix.proposed` / `change.proposal.created`.

use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::software_world::world::SoftwareWorldId;
use crate::domain::ai::frame::{InvestigationFrame, InvestigationFrameId};
use crate::domain::ai::patch::{
    PatchArtifactError, PatchArtifactSink, PatchBudget, PatchRef, PatchValidationError,
    ValidatedSourcePatch, validate_source_patch,
};
use crate::domain::ai::port::{LlmPort, LlmPortError};
use crate::domain::ai::request::{InvestigationRequest, RequestProvenance, ToolCall};
use crate::domain::ai::response::ResponseOutput;

/// Caller label recorded in the request provenance.
pub const FIX_AGENT_CALLER: &str = "fix-agent-v1";

/// Audit lineage preserved from the inference that produced a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixLineage {
    /// The frame the model answered.
    pub frame_id: InvestigationFrameId,
    /// Digest of the request the model answered.
    pub request_digest: u64,
    /// Provider identity reported by the port.
    pub provider: String,
    /// Model identity reported by the port.
    pub model: String,
    /// The configured agent identity stamped onto the proposal.
    pub agent_ref: String,
    /// How many canonical facts the model observed.
    pub observed_read_count: usize,
}

/// What a successful fix run produces.
///
/// The `proposal` is the terminal effect. `patch` / `patch_ref` / `lineage` are
/// audit material; none of them grants authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixOutcome {
    /// The proposal. Author is always `LlmAgent`.
    pub proposal: ChangeProposal,
    /// Content address of the stored patch artifact.
    pub patch_ref: PatchRef,
    /// The validated patch that backs the proposal.
    pub patch: ValidatedSourcePatch,
    /// Preserved inference lineage.
    pub lineage: FixLineage,
}

/// Why a fix run failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixAgentError {
    /// The configured agent identity was empty.
    EmptyAgentIdentity,
    /// The port refused the request.
    Port(LlmPortError),
    /// The response was not a source patch candidate.
    ResponseNotAPatchCandidate {
        /// The output kind that was returned instead.
        actual: &'static str,
    },
    /// The candidate failed validation.
    Validation(PatchValidationError),
    /// The patch artifact could not be stored.
    Artifact(PatchArtifactError),
}

impl std::fmt::Display for FixAgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyAgentIdentity => f.write_str("fix agent identity must not be empty"),
            Self::Port(e) => write!(f, "llm port error: {e}"),
            Self::ResponseNotAPatchCandidate { actual } => {
                write!(f, "expected a source patch candidate, got {actual}")
            }
            Self::Validation(e) => write!(f, "patch validation failed: {e}"),
            Self::Artifact(e) => write!(f, "patch artifact error: {e}"),
        }
    }
}

impl std::error::Error for FixAgentError {}

impl From<LlmPortError> for FixAgentError {
    fn from(e: LlmPortError) -> Self {
        Self::Port(e)
    }
}

impl From<PatchValidationError> for FixAgentError {
    fn from(e: PatchValidationError) -> Self {
        Self::Validation(e)
    }
}

impl From<PatchArtifactError> for FixAgentError {
    fn from(e: PatchArtifactError) -> Self {
        Self::Artifact(e)
    }
}

/// The bounded fix orchestrator.
///
/// Hold one per configured agent identity. `port` and `sink` are borrowed so
/// the agent owns no infrastructure.
pub struct FixAgent<'a> {
    port: &'a dyn LlmPort,
    sink: &'a dyn PatchArtifactSink,
    agent_ref: String,
    budget: PatchBudget,
}

impl<'a> FixAgent<'a> {
    /// Construct over a port and a patch sink, with the configured trusted
    /// agent identity. The identity is rejected when empty.
    pub fn new(
        port: &'a dyn LlmPort,
        sink: &'a dyn PatchArtifactSink,
        agent_ref: impl Into<String>,
    ) -> Result<Self, FixAgentError> {
        let agent_ref = agent_ref.into();
        if agent_ref.trim().is_empty() {
            return Err(FixAgentError::EmptyAgentIdentity);
        }
        Ok(Self {
            port,
            sink,
            agent_ref,
            budget: PatchBudget::default(),
        })
    }

    /// Override the patch budget.
    pub fn with_budget(mut self, budget: PatchBudget) -> Self {
        self.budget = budget;
        self
    }

    /// The configured agent identity.
    pub fn agent_ref(&self) -> &str {
        &self.agent_ref
    }

    /// Run one fix: frame → candidate → validated patch → artifact →
    /// `ChangeProposal`.
    ///
    /// `proposal_id` and `base_world` are the **trusted** envelope; they are
    /// supplied by the caller, never by the model.
    pub fn propose(
        &self,
        proposal_id: ChangeProposalId,
        base_world: SoftwareWorldId,
        frame: &InvestigationFrame,
    ) -> Result<FixOutcome, FixAgentError> {
        let request = build_fix_request(frame);
        let response = self.port.complete(&request, frame)?;

        let candidate = match response.output() {
            ResponseOutput::SourcePatchCandidate(candidate) => candidate.clone(),
            other => {
                return Err(FixAgentError::ResponseNotAPatchCandidate {
                    actual: output_kind_name(other),
                });
            }
        };

        let patch = validate_source_patch(frame, &request, &response, &candidate, self.budget)?;
        let patch_ref = self.sink.store(patch.clone())?;

        // Author is stamped from configuration, never from the response.
        let proposal = ChangeProposal::new(
            proposal_id,
            base_world,
            ProposalKind::SourcePatch {
                patch_ref: patch_ref.as_str().to_string(),
            },
            RequestedBy::LlmAgent {
                agent_ref: self.agent_ref.clone(),
            },
        );

        let lineage = FixLineage {
            frame_id: response.frame_id(),
            request_digest: response.request_digest(),
            provider: response.response_provenance().provider.clone(),
            model: response.response_provenance().model.clone(),
            agent_ref: self.agent_ref.clone(),
            observed_read_count: response.observed_read_set().len(),
        };

        Ok(FixOutcome {
            proposal,
            patch_ref,
            patch,
            lineage,
        })
    }
}

/// Build the fix request from a frame. Read-only tool surface.
pub fn build_fix_request(frame: &InvestigationFrame) -> InvestigationRequest {
    let provenance = RequestProvenance::new(frame.id(), FIX_AGENT_CALLER, None)
        .expect("caller label is non-empty");
    let tools = vec![
        ToolCall::new("read_source", Vec::<String>::new()).expect("tool name is non-empty"),
        ToolCall::new("read_fact", Vec::<String>::new()).expect("tool name is non-empty"),
    ];
    InvestigationRequest::try_new(frame.id(), frame.question(), tools, provenance)
        .expect("frame question is non-empty")
}

fn output_kind_name(output: &ResponseOutput) -> &'static str {
    match output {
        ResponseOutput::Hypotheses(_) => "hypotheses",
        ResponseOutput::Critiques(_) => "critiques",
        ResponseOutput::Advisory { .. } => "advisory",
        ResponseOutput::SourcePatchCandidate(_) => "source-patch-candidate",
    }
}
