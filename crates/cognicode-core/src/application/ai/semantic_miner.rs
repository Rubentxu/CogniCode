//! SemanticMiner — read-only AI prototype.
//!
//! The miner is the **application-side** orchestrator that turns an
//! [`InvestigationFrame`] into one [`InvestigationRequest`], invokes
//! the [`LlmPort`], and returns the bounded output. It never writes
//! to canonical stores, never mints evidence, never applies patches.
//!
//! ## Output
//!
//! - `Suggestion(Hypothesis)` — a free-form advisory observation.
//! - `ConstraintCandidate` — a proposed architecture constraint that
//!   the admission gate (e77) may admit later. The miner NEVER
//!   produces an admitted [`ArchitectureConstraint`]; admission is
//!   the only path to authority (e77 design).
//! - `DetectorCandidate { id, summary }` — a free-form detector idea
//!   the platform may register later via the detector admission flow.
//!
//! The miner is **deterministic given a deterministic port**: the same
//! frame + the same scripted port → the same output.
//!
//! ## Authority boundary
//!
//! The miner emits NO `BehaviorEffect::CommitCanonicalFact` (that is
//! structurally forbidden by the behavior authority table — `class.rs`
//! `BehaviorAuthorityPolicy::allows`). The miner cannot commit facts
//! even if it tried to.

use crate::domain::ai::frame::InvestigationFrame;
use crate::domain::ai::hypothesis::{
    Hypothesis, HypothesisConfidence, HypothesisId, HypothesisStatement,
};
use crate::domain::ai::port::LlmPort;
use crate::domain::ai::request::{InvestigationRequest, RequestProvenance, ToolCall};
use crate::domain::ai::response::{LlmResponse, ResponseOutput};
use crate::domain::architecture::{
    ArchitectureConstraintId, ArchitectureConstraintKind, ConstraintCandidate, LayerDependencyRule,
    LayerId,
};
use crate::domain::naming::NamespacedName;

/// What the miner produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinerOutput {
    /// A free-form advisory observation.
    Suggestion(Hypothesis),
    /// A proposed architecture constraint. NOT admitted.
    ConstraintCandidate(ConstraintCandidate),
    /// A free-form detector idea.
    DetectorCandidate {
        /// Stable id (e.g. `"ai:semantic-miner:<uuid>"`).
        id: String,
        /// Bounded summary.
        summary: String,
    },
}

/// Error during a miner run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinerError {
    /// The port refused the request.
    Port(crate::domain::ai::port::LlmPortError),
    /// The response did not match the frame.
    FrameMismatch,
}

impl std::fmt::Display for MinerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Port(e) => write!(f, "port error: {e}"),
            Self::FrameMismatch => f.write_str("response frame_id does not match request frame_id"),
        }
    }
}

impl std::error::Error for MinerError {}

impl From<crate::domain::ai::port::LlmPortError> for MinerError {
    fn from(e: crate::domain::ai::port::LlmPortError) -> Self {
        Self::Port(e)
    }
}

/// The read-only prototype orchestrator.
pub struct SemanticMiner<'a> {
    port: &'a dyn LlmPort,
}

impl<'a> SemanticMiner<'a> {
    /// Construct a miner over a port.
    pub fn new(port: &'a dyn LlmPort) -> Self {
        Self { port }
    }

    /// Mine a single frame into output.
    ///
    /// The caller is expected to construct the `InvestigationFrame`
    /// (with the appropriate scope and budget). The miner:
    ///
    /// 1. Builds an [`InvestigationRequest`] from the frame.
    /// 2. Invokes the port.
    /// 3. Validates that the response's frame_id matches the request's.
    /// 4. Returns the bounded output, converted into [`MinerOutput`]
    ///    variants where appropriate.
    pub fn mine(&self, frame: &InvestigationFrame) -> Result<Vec<MinerOutput>, MinerError> {
        let req = build_request(frame);
        let resp = self.port.complete(&req, frame)?;
        if resp.frame_id() != frame.id() {
            return Err(MinerError::FrameMismatch);
        }
        Ok(convert_response(&resp, frame))
    }
}

/// Build the request from the frame.
///
/// The instruction is the frame's question; the tools are a fixed
/// read-only surface (no writes, no applies). The caller label is
/// `"semantic-miner-v1"`.
pub fn build_request(frame: &InvestigationFrame) -> InvestigationRequest {
    let prov = RequestProvenance::new(frame.id(), "semantic-miner-v1", None)
        .expect("caller label is non-empty");
    let tools = vec![
        ToolCall::new("read_fact", Vec::<String>::new()).unwrap(),
        ToolCall::new("list_architecture_constraints", Vec::<String>::new()).unwrap(),
    ];
    InvestigationRequest::try_new(frame.id(), frame.question(), tools, prov)
        .expect("frame question is non-empty")
}

/// Convert a port response into miner outputs.
///
/// Today: every hypothesis in the response is returned as-is. Future
/// cycles may interpret candidate kinds and emit `ConstraintCandidate`
/// or `DetectorCandidate` directly. The conversion is deterministic
/// and pure.
///
/// A `SourcePatchCandidate` (e80b) is a write-side suggestion and is therefore
/// outside the miner's vocabulary: the miner mines hypotheses, constraints, and
/// detector ideas, never patches. Such a response yields no miner output; it is
/// handled by the `FixAgent` instead.
pub fn convert_response(response: &LlmResponse, _frame: &InvestigationFrame) -> Vec<MinerOutput> {
    match response.output() {
        ResponseOutput::Hypotheses(hs) => hs
            .iter()
            .map(|h| MinerOutput::Suggestion(h.clone()))
            .collect(),
        ResponseOutput::Critiques(cs) => cs
            .iter()
            .map(|c| {
                // A critique of a finding is wrapped as a suggestion so
                // the lineage layer can record it. The critic (WU5)
                // owns the actual Critique disposition semantics.
                let stmt = HypothesisStatement::Suggestion(format!(
                    "critique of {}: {} ({})",
                    c.finding_id.as_str(),
                    c.disposition.name(),
                    c.rationale
                ));
                let hyp = Hypothesis::try_new(
                    HypothesisId::new(0),
                    stmt,
                    c.grounding.clone(),
                    vec![],
                    HypothesisConfidence::default(),
                )
                .unwrap_or_else(|_| {
                    // If construction fails (empty rationale — should
                    // never happen because Critique::try_new guards
                    // it), fall back to a non-empty stub.
                    let stub = HypothesisStatement::Suggestion("critique fallback".into());
                    Hypothesis::try_new(
                        HypothesisId::new(0),
                        stub,
                        vec![],
                        vec![],
                        HypothesisConfidence::default(),
                    )
                    .unwrap()
                });
                MinerOutput::Suggestion(hyp)
            })
            .collect(),
        ResponseOutput::Advisory { summary } => {
            let stmt = HypothesisStatement::Suggestion(summary.clone());
            let hyp = Hypothesis::try_new(
                HypothesisId::new(0),
                stmt,
                vec![],
                vec![],
                HypothesisConfidence::default(),
            )
            .unwrap_or_else(|_| {
                let stub = HypothesisStatement::Suggestion("advisory fallback".into());
                Hypothesis::try_new(
                    HypothesisId::new(0),
                    stub,
                    vec![],
                    vec![],
                    HypothesisConfidence::default(),
                )
                .unwrap()
            });
            vec![MinerOutput::Suggestion(hyp)]
        }
        // Write-side suggestion, outside the miner's vocabulary.
        ResponseOutput::SourcePatchCandidate(_) => Vec::new(),
    }
}

/// Helper: synthesize a `ConstraintCandidate` for an architecture
/// constraint the miner wants to suggest. The candidate is NOT
/// admitted — only `ArchitectureAdmissionService::admit` can confer
/// authority.
pub fn candidate_for_layer_dependency(
    id: impl Into<String>,
    adr_ref: Option<String>,
    proposed_by: impl Into<String>,
    from_layer: LayerId,
    forbidden_targets: Vec<LayerId>,
) -> Result<ConstraintCandidate, &'static str> {
    let raw: String = id.into();
    let namespaced = NamespacedName::new(format!("architecture.miner.{raw}"))
        .map_err(|_| "invalid miner constraint id")?;
    let constraint_id = ArchitectureConstraintId::new(namespaced.as_str().to_string())
        .map_err(|_| "invalid miner constraint id")?;
    Ok(ConstraintCandidate {
        id: constraint_id,
        kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
            from_layer,
            forbidden_targets,
            rationale: "mined by semantic-miner-v1".into(),
        }),
        adr_ref,
        proposed_by: proposed_by.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::fake::{FakeLlmPort, ScriptKey};
    use crate::domain::ai::InvestigationFrame;
    use crate::domain::ai::InvestigationFrameId;
    use crate::domain::ai::request::RequestProvenance;
    use crate::domain::ai::response::{LlmResponse, ResponseOutput, ResponseProvenance};
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

    fn make_response_for(frame: &InvestigationFrame, output: ResponseOutput) -> LlmResponse {
        let req_prov = RequestProvenance::new(frame.id(), "semantic-miner-v1", None).unwrap();
        let resp_prov =
            ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap();
        // Build the request so we can capture its digest.
        let req = build_request(frame);
        match output {
            ResponseOutput::Hypotheses(hs) => LlmResponse::new_hypotheses(
                frame.id(),
                req.content_digest(),
                req_prov,
                resp_prov,
                empty_read_set(),
                hs,
            ),
            ResponseOutput::Critiques(cs) => LlmResponse::new_critiques(
                frame.id(),
                req.content_digest(),
                req_prov,
                resp_prov,
                empty_read_set(),
                cs,
            ),
            ResponseOutput::Advisory { summary } => LlmResponse::new_advisory(
                frame.id(),
                req.content_digest(),
                req_prov,
                resp_prov,
                empty_read_set(),
                summary,
            ),
            ResponseOutput::SourcePatchCandidate(candidate) => {
                LlmResponse::new_source_patch_candidate(
                    frame.id(),
                    req.content_digest(),
                    req_prov,
                    resp_prov,
                    empty_read_set(),
                    candidate,
                )
            }
        }
    }

    fn well_formed_hypothesis() -> Hypothesis {
        let stmt = HypothesisStatement::Candidate {
            kind: "architecture_constraint".into(),
            summary: "domain must not reach infrastructure".into(),
        };
        Hypothesis::try_new(
            HypothesisId::new(1),
            stmt,
            vec![],
            vec![],
            HypothesisConfidence::new(70),
        )
        .unwrap()
    }

    #[test]
    fn build_request_is_deterministic() {
        let f = frame_with("q");
        let r1 = build_request(&f);
        let r2 = build_request(&f);
        assert_eq!(r1.content_digest(), r2.content_digest());
        assert_eq!(r1.frame_id(), f.id());
        assert_eq!(r1.tools().len(), 2);
    }

    #[test]
    fn miner_returns_hypotheses_from_response() {
        let f = frame_with("q");
        let resp = make_response_for(
            &f,
            ResponseOutput::Hypotheses(vec![well_formed_hypothesis()]),
        );
        let fake = FakeLlmPort::new().with_scripted(
            ScriptKey::new(f.id(), build_request(&f).content_digest()),
            resp,
        );
        let miner = SemanticMiner::new(&fake);
        let out = miner.mine(&f).unwrap();
        assert_eq!(out.len(), 1);
        match &out[0] {
            MinerOutput::Suggestion(h) => {
                assert!(h.statement().is_candidate());
                assert_eq!(h.confidence().value(), 70);
            }
            _ => panic!("expected Suggestion"),
        }
    }

    #[test]
    fn miner_returns_advisory_from_response() {
        let f = frame_with("q");
        let resp = make_response_for(
            &f,
            ResponseOutput::Advisory {
                summary: "look at line 17".into(),
            },
        );
        let fake = FakeLlmPort::new().with_scripted(
            ScriptKey::new(f.id(), build_request(&f).content_digest()),
            resp,
        );
        let miner = SemanticMiner::new(&fake);
        let out = miner.mine(&f).unwrap();
        assert_eq!(out.len(), 1);
        match &out[0] {
            MinerOutput::Suggestion(h) => {
                assert!(!h.statement().is_candidate());
                assert!(h.statement().summary().contains("line 17"));
            }
            _ => panic!("expected Suggestion"),
        }
    }

    #[test]
    fn a_frame_mismatch_is_an_error() {
        let f = frame_with("q");
        let other_frame_id = InvestigationFrameId::from_content_digest(999);
        let resp = make_response_for(
            &f,
            ResponseOutput::Advisory {
                summary: "s".into(),
            },
        );
        let mut bad = resp;
        // Force a frame_id mismatch via a new LlmResponse with a different frame_id.
        let bad_resp = LlmResponse::new_advisory(
            other_frame_id,
            42,
            RequestProvenance::new(other_frame_id, "semantic-miner-v1", None).unwrap(),
            ResponseProvenance::new("p", "m", None, None).unwrap(),
            empty_read_set(),
            "s",
        );
        bad = bad_resp;
        let _ = bad; // suppress unused
        let fake = FakeLlmPort::new().with_fallback(bad);
        let miner = SemanticMiner::new(&fake);
        let err = miner.mine(&f).unwrap_err();
        assert!(matches!(err, MinerError::FrameMismatch));
    }

    #[test]
    fn candidate_for_layer_dependency_constructs_a_candidate() {
        let c = candidate_for_layer_dependency(
            "domain_no_infra",
            Some("ADR-046".into()),
            "ai:semantic-miner-v1",
            LayerId::Domain,
            vec![LayerId::Infrastructure],
        )
        .unwrap();
        // Not admitted: no `admitted_at`, no `admitted_by`.
        match c.kind {
            ArchitectureConstraintKind::LayerDependency(rule) => {
                assert_eq!(rule.from_layer, LayerId::Domain);
                assert!(rule.forbidden_targets.contains(&LayerId::Infrastructure));
            }
            _ => panic!("expected LayerDependency"),
        }
    }

    #[test]
    fn miner_does_not_emit_committed_facts_or_authority() {
        // The MinerOutput enum has no CommitCanonicalFact variant and
        // no authority field. Compile-time check via `matches!`.
        let f = frame_with("q");
        let resp = make_response_for(
            &f,
            ResponseOutput::Advisory {
                summary: "x".into(),
            },
        );
        let fake = FakeLlmPort::new().with_scripted(
            ScriptKey::new(f.id(), build_request(&f).content_digest()),
            resp,
        );
        let miner = SemanticMiner::new(&fake);
        let out = miner.mine(&f).unwrap();
        for o in &out {
            // The miner never emits CommitCanonicalFact (no such
            // variant in MinerOutput) and never carries authority
            // (no Admitter, no admitted_at).
            assert!(matches!(
                o,
                MinerOutput::Suggestion(_)
                    | MinerOutput::ConstraintCandidate(_)
                    | MinerOutput::DetectorCandidate { .. }
            ));
            if let MinerOutput::ConstraintCandidate(cc) = o {
                // ConstraintCandidate has no admitted_at / admitted_by fields —
                // it's structurally unadmitted. (Field-level test would
                // require destructuring; the type system enforces it.)
                let _ = cc;
            }
        }
    }
}
