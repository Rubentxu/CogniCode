//! FindingCritic — read-only AI orchestrator.
//!
//! Consumes existing `Finding`s + canonical evidence refs and returns
//! [`Critique`] dispositions. The critic advises; the policy gate
//! decides.
//!
//! ## Authority boundary
//!
//! - The critic never deletes a finding.
//! - The critic never promotes severity or authority.
//! - The critic never mints a gateable finding of its own.
//! - The critic's `Critique` is a *disposition*, not a `Finding` —
//!   it cannot enter the policy gate's decision path.
//!
//! ## Useful property
//!
//! ```text
//! critic cannot make a Finding more authoritative than before
//! ```
//!
//! This is enforced structurally: the critic's only return type is
//! `Vec<Critique>`, and `Critique` carries no authority fields
//! (no `Admitter`, no `admitted_at`, no `DetectorAuthority`).

use crate::domain::ai::frame::InvestigationFrame;
pub use crate::domain::ai::hypothesis::CritiqueError;
use crate::domain::ai::hypothesis::{Critique, CritiqueDisposition, HypothesisRef};
use crate::domain::ai::port::LlmPort;
use crate::domain::ai::request::{InvestigationRequest, RequestProvenance, ToolCall};
use crate::domain::ai::response::{LlmResponse, ResponseOutput};
use crate::domain::findings::FindingId;

/// What the critic was asked to critique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CritiqueTarget {
    /// The finding under critique.
    pub finding_id: FindingId,
    /// Canonical references the critic may consult.
    pub refs: Vec<HypothesisRef>,
}

/// Error during a critic run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingCriticError {
    /// The port refused the request.
    Port(crate::domain::ai::port::LlmPortError),
    /// The response did not match the frame.
    FrameMismatch,
    /// A critique construction failed (empty rationale, etc.).
    Critique(CritiqueError),
}

impl std::fmt::Display for FindingCriticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Port(e) => write!(f, "port error: {e}"),
            Self::FrameMismatch => f.write_str("response frame_id does not match request frame_id"),
            Self::Critique(e) => write!(f, "critique construction: {e}"),
        }
    }
}

impl std::error::Error for FindingCriticError {}

impl From<crate::domain::ai::port::LlmPortError> for FindingCriticError {
    fn from(e: crate::domain::ai::port::LlmPortError) -> Self {
        Self::Port(e)
    }
}

impl From<CritiqueError> for FindingCriticError {
    fn from(e: CritiqueError) -> Self {
        Self::Critique(e)
    }
}

/// The critic.
pub struct FindingCritic<'a> {
    port: &'a dyn LlmPort,
}

impl<'a> FindingCritic<'a> {
    /// Construct a critic over a port.
    pub fn new(port: &'a dyn LlmPort) -> Self {
        Self { port }
    }

    /// Critique the given targets.
    ///
    /// The caller is expected to construct the `InvestigationFrame`
    /// with the appropriate scope and budget. The critic:
    ///
    /// 1. Builds an `InvestigationRequest` whose tools include
    ///    `critique_finding`.
    /// 2. Invokes the port.
    /// 3. Validates the response's frame_id.
    /// 4. Returns the bounded critique list.
    pub fn critique(
        &self,
        frame: &InvestigationFrame,
        targets: &[CritiqueTarget],
    ) -> Result<Vec<Critique>, FindingCriticError> {
        let req = build_critique_request(frame, targets);
        let resp = self.port.complete(&req, frame)?;
        if resp.frame_id() != frame.id() {
            return Err(FindingCriticError::FrameMismatch);
        }
        Ok(extract_critiques(&resp))
    }
}

/// Build the critique request. The instruction embeds the target
/// list as bounded text. The tool surface is fixed (read-only).
pub fn build_critique_request(
    frame: &InvestigationFrame,
    targets: &[CritiqueTarget],
) -> InvestigationRequest {
    let mut instruction = String::from("critique the following findings: ");
    for (i, t) in targets.iter().enumerate() {
        if i > 0 {
            instruction.push_str(", ");
        }
        instruction.push_str(t.finding_id.as_str());
    }
    let prov = RequestProvenance::new(frame.id(), "finding-critic-v1", None)
        .expect("caller label is non-empty");
    let tools = vec![ToolCall::new("critique_finding", Vec::<String>::new()).unwrap()];
    InvestigationRequest::try_new(frame.id(), instruction, tools, prov)
        .expect("instruction is non-empty")
}

/// Extract `Critique`s from a port response.
///
/// The critic accepts responses whose output is `Critiques` (the
/// natural shape) and `Advisory` (treated as `NeedsInvestigation`
/// for every target). Hypotheses are ignored — the critic must
/// return critiques, not suggestions. A `SourcePatchCandidate` is a
/// write-side suggestion (e80b) and is likewise out of the critic's
/// vocabulary: it is ignored here and handled by the `FixAgent`.
pub fn extract_critiques(response: &LlmResponse) -> Vec<Critique> {
    match response.output() {
        ResponseOutput::Critiques(cs) => cs.clone(),
        ResponseOutput::Advisory { .. } => Vec::new(), // No critique without explicit disposition.
        ResponseOutput::Hypotheses(_) => Vec::new(),
        ResponseOutput::SourcePatchCandidate(_) => Vec::new(),
    }
}

/// A deterministic default critique disposition for a target whose
/// grounding is empty (i.e. nothing to consult). Use only when the
/// caller has canonical knowledge that the target has no grounding.
pub fn missing_evidence_default(target: &CritiqueTarget) -> Result<Critique, CritiqueError> {
    Critique::try_new(
        target.finding_id.clone(),
        CritiqueDisposition::MissingEvidence,
        "no canonical evidence grounding for this finding",
        target.refs.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::fake::{FakeLlmPort, ScriptKey};
    use crate::domain::ai::InvestigationFrame;
    use crate::domain::ai::request::RequestProvenance;
    use crate::domain::ai::response::{ResponseOutput, ResponseProvenance};
    use crate::domain::execution::{AnalysisScope, CorrelationId, ExecutionContext};
    use crate::domain::kernel_ids::{ExecutionId, SnapshotId};
    use crate::domain::readset::{InMemoryReadSetRecorder, ReadSet, ReadSetConfig};
    use crate::domain::value_objects::WorkspaceId;

    fn empty_read_set() -> ReadSet {
        InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None }).finalize()
    }

    fn ctx() -> ExecutionContext {
        ExecutionContext::try_new(
            ExecutionId::new(1),
            AnalysisScope::new(WorkspaceId::try_new("ws").unwrap(), SnapshotId::new(7)),
            crate::domain::execution::ActorRef::agent("explorer"),
            CorrelationId::new("c-1").unwrap(),
            None,
        )
        .unwrap()
    }

    fn frame_with(question: &str) -> InvestigationFrame {
        InvestigationFrame::try_new(
            ctx(),
            question,
            crate::domain::ai::InvestigationScope::empty(
                WorkspaceId::try_new("ws").unwrap(),
                SnapshotId::new(7),
            ),
            crate::domain::ai::InvestigationBudget::default(),
        )
        .unwrap()
    }

    #[test]
    fn the_critique_request_is_deterministic() {
        let f = frame_with("critique");
        let targets = vec![CritiqueTarget {
            finding_id: FindingId::new("f-1").unwrap(),
            refs: vec![],
        }];
        let r1 = build_critique_request(&f, &targets);
        let r2 = build_critique_request(&f, &targets);
        assert_eq!(r1.content_digest(), r2.content_digest());
    }

    #[test]
    fn extract_critiques_returns_empty_for_hypotheses_output() {
        let id = crate::domain::ai::InvestigationFrameId::from_content_digest(1);
        let resp = LlmResponse::new_hypotheses(
            id,
            1,
            RequestProvenance::new(id, "test", None).unwrap(),
            ResponseProvenance::new("p", "m", None, None).unwrap(),
            empty_read_set(),
            vec![],
        );
        assert!(extract_critiques(&resp).is_empty());
    }

    #[test]
    fn extract_critiques_returns_critiques_when_output_is_critiques() {
        let id = crate::domain::ai::InvestigationFrameId::from_content_digest(1);
        let fid = FindingId::new("f-1").unwrap();
        let c = Critique::try_new(
            fid.clone(),
            CritiqueDisposition::Supported,
            "grounded",
            vec![],
        )
        .unwrap();
        let resp = LlmResponse::new_critiques(
            id,
            1,
            RequestProvenance::new(id, "test", None).unwrap(),
            ResponseProvenance::new("p", "m", None, None).unwrap(),
            empty_read_set(),
            vec![c.clone()],
        );
        let out = extract_critiques(&resp);
        assert_eq!(out, vec![c]);
    }

    #[test]
    fn the_critic_does_not_emit_authority() {
        // The critic's return type is `Vec<Critique>`; `Critique`
        // carries no authority fields. This is a compile-time check
        // — if `Critique` ever grew an authority field, this code
        // would have to be rewritten.
        let f = frame_with("critique");
        let req = build_critique_request(&f, &[]);
        assert!(!req.provenance().caller.contains("admit"));
        assert_eq!(req.tools().len(), 1);
        assert_eq!(req.tools()[0].name, "critique_finding");
    }

    #[test]
    fn missing_evidence_default_is_constructible() {
        let t = CritiqueTarget {
            finding_id: FindingId::new("f-1").unwrap(),
            refs: vec![],
        };
        let c = missing_evidence_default(&t).unwrap();
        assert_eq!(c.disposition, CritiqueDisposition::MissingEvidence);
        assert_eq!(c.finding_id.as_str(), "f-1");
    }

    #[test]
    fn the_critic_returns_critiques_via_fake_port() {
        let f = frame_with("critique");
        let fid = FindingId::new("f-1").unwrap();
        let c = Critique::try_new(
            fid.clone(),
            CritiqueDisposition::WeaklySupported,
            "two of three claims are grounded",
            vec![],
        )
        .unwrap();
        let id = f.id();
        let resp = LlmResponse::new_critiques(
            id,
            build_critique_request(&f, &[]).content_digest(),
            RequestProvenance::new(id, "finding-critic-v1", None).unwrap(),
            ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap(),
            empty_read_set(),
            vec![c.clone()],
        );
        let fake = FakeLlmPort::new().with_scripted(
            ScriptKey::new(id, build_critique_request(&f, &[]).content_digest()),
            resp,
        );
        let critic = FindingCritic::new(&fake);
        let out = critic.critique(&f, &[]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].finding_id.as_str(), "f-1");
        assert_eq!(out[0].disposition, CritiqueDisposition::WeaklySupported);
    }
}
