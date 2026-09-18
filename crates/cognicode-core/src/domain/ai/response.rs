//! LlmResponse — what an `LlmPort` returns.
//!
//! The response carries **provenance**, **lineage**, and the bounded
//! output. The output is a list of [`Hypothesis`], [`Critique`], or
//! raw advisory observations — never a `Fact`, never a gateable
//! `Finding`.
//!
//! Critical invariants:
//!
//! - The response echoes the request's `frame_id` and `provenance`
//!   verbatim. Mismatch is a port contract violation.
//! - The response's `observed_read_set` is the set of canonical facts
//!   the agent actually consulted. The lineage layer audits this
//!   against the frame's declared scope.
//! - The response's `output` is bounded by the frame's
//!   `max_response_bytes`.

use serde::{Deserialize, Serialize};

use crate::domain::ai::frame::InvestigationFrameId;
use crate::domain::ai::hypothesis::{Critique, Hypothesis};
use crate::domain::ai::patch::SourcePatchCandidate;
use crate::domain::ai::request::RequestProvenance;
use crate::domain::kernel_ids::FactId;
use crate::domain::readset::ReadSet;

/// What the port observed during inference — provenance for the
/// lineage trail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseProvenance {
    /// Stable provider identity (e.g. "fake-local", "openai-1",
    /// "anthropic-3"). Free-form but non-empty.
    pub provider: String,
    /// Model identifier within the provider (e.g. "gpt-4o-mini",
    /// "claude-opus", "fake-deterministic-v1"). Free-form but
    /// non-empty.
    pub model: String,
    /// Wall-clock timestamp supplied by the port. `None` if no clock.
    pub generated_at: Option<String>,
    /// Optional usage/budget diagnostics. Free-form.
    pub usage_diagnostics: Option<String>,
}

impl ResponseProvenance {
    /// Construct, rejecting empty provider or model.
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        generated_at: Option<String>,
        usage_diagnostics: Option<String>,
    ) -> Result<Self, &'static str> {
        let provider = provider.into();
        let model = model.into();
        if provider.trim().is_empty() {
            return Err("provider must not be empty");
        }
        if model.trim().is_empty() {
            return Err("model must not be empty");
        }
        Ok(Self {
            provider,
            model,
            generated_at,
            usage_diagnostics,
        })
    }
}

/// What the port produced.
///
/// A response is one of three kinds, and the constructor chooses
/// which by tagging the `kind` field. The output list is bounded by
/// the frame's `max_response_bytes` (enforced by the port).
///
/// Not `Serialize`/`Deserialize`: the embedded `ReadSet` is the
/// canonical lineage model and is intentionally not serde-serializable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmResponse {
    /// The frame this response was generated for.
    frame_id: InvestigationFrameId,
    /// The request's provenance, echoed verbatim.
    request_provenance: RequestProvenance,
    /// Stable digest of the request this response answers.
    request_digest: u64,
    /// Provenance of the port's invocation.
    response_provenance: ResponseProvenance,
    /// The actual canonical facts the agent consulted during
    /// inference. Audited against the frame's declared scope.
    observed_read_set: ReadSet,
    /// The bounded output.
    output: ResponseOutput,
}

/// The bounded output of one LlmPort invocation.
///
/// ## e80b: a fix candidate is a suggestion, not an action
///
/// [`Self::SourcePatchCandidate`] is how a model proposes a source change. It
/// is deliberately a *typed* candidate rather than an `Advisory` string, so no
/// parsing convention stands between free-form text and an action proposal.
/// The candidate is still not a `ChangeProposal`, not a mutation, not evidence,
/// and not authority: converting it into a proposal requires the trusted
/// `FixAgent` orchestration plus the [`validate_source_patch`] boundary.
///
/// [`validate_source_patch`]: crate::domain::ai::patch::validate_source_patch
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseOutput {
    /// A list of AI-generated hypotheses.
    Hypotheses(Vec<Hypothesis>),
    /// A list of AI-generated critiques of existing findings.
    Critiques(Vec<Critique>),
    /// A free-form advisory observation that is neither a hypothesis
    /// nor a critique. The content is bounded by the port; downstream
    /// callers that want to persist it must convert to `Hypothesis`.
    Advisory {
        /// A bounded summary string.
        summary: String,
    },
    /// An AI-produced source patch suggestion (e80b). Untrusted,
    /// non-authoritative: it must pass the validation boundary before it can
    /// back a `ChangeProposal::SourcePatch`.
    SourcePatchCandidate(SourcePatchCandidate),
}

impl LlmResponse {
    /// Construct a response with hypotheses.
    pub fn new_hypotheses(
        frame_id: InvestigationFrameId,
        request_digest: u64,
        request_provenance: RequestProvenance,
        response_provenance: ResponseProvenance,
        observed_read_set: ReadSet,
        hypotheses: Vec<Hypothesis>,
    ) -> Self {
        Self {
            frame_id,
            request_provenance,
            request_digest,
            response_provenance,
            observed_read_set,
            output: ResponseOutput::Hypotheses(hypotheses),
        }
    }

    /// Construct a response with critiques.
    pub fn new_critiques(
        frame_id: InvestigationFrameId,
        request_digest: u64,
        request_provenance: RequestProvenance,
        response_provenance: ResponseProvenance,
        observed_read_set: ReadSet,
        critiques: Vec<Critique>,
    ) -> Self {
        Self {
            frame_id,
            request_provenance,
            request_digest,
            response_provenance,
            observed_read_set,
            output: ResponseOutput::Critiques(critiques),
        }
    }

    /// Construct an advisory response.
    pub fn new_advisory(
        frame_id: InvestigationFrameId,
        request_digest: u64,
        request_provenance: RequestProvenance,
        response_provenance: ResponseProvenance,
        observed_read_set: ReadSet,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            frame_id,
            request_provenance,
            request_digest,
            response_provenance,
            observed_read_set,
            output: ResponseOutput::Advisory {
                summary: summary.into(),
            },
        }
    }

    /// Construct a source-patch-candidate response (e80b).
    ///
    /// The candidate is untrusted: it must cross
    /// [`validate_source_patch`](crate::domain::ai::patch::validate_source_patch)
    /// before it can back a proposal.
    pub fn new_source_patch_candidate(
        frame_id: InvestigationFrameId,
        request_digest: u64,
        request_provenance: RequestProvenance,
        response_provenance: ResponseProvenance,
        observed_read_set: ReadSet,
        candidate: SourcePatchCandidate,
    ) -> Self {
        Self {
            frame_id,
            request_provenance,
            request_digest,
            response_provenance,
            observed_read_set,
            output: ResponseOutput::SourcePatchCandidate(candidate),
        }
    }

    /// The frame id this response was generated for.
    pub fn frame_id(&self) -> InvestigationFrameId {
        self.frame_id
    }

    /// The request's provenance (echoed verbatim).
    pub fn request_provenance(&self) -> &RequestProvenance {
        &self.request_provenance
    }

    /// The digest of the request this response answers.
    pub fn request_digest(&self) -> u64 {
        self.request_digest
    }

    /// The port's invocation provenance.
    pub fn response_provenance(&self) -> &ResponseProvenance {
        &self.response_provenance
    }

    /// The set of canonical facts the agent *actually* read.
    pub fn observed_read_set(&self) -> &ReadSet {
        &self.observed_read_set
    }

    /// The bounded output.
    pub fn output(&self) -> &ResponseOutput {
        &self.output
    }

    /// Whether the response observed any reads at all.
    pub fn read_anything(&self) -> bool {
        !self.observed_read_set.is_empty()
    }

    /// Whether the observed read set contains a given fact.
    pub fn observed_fact(&self, fact: FactId) -> bool {
        self.observed_read_set.contains(&fact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ai::InvestigationFrameId;
    use crate::domain::readset::{InMemoryReadSetRecorder, ReadSet, ReadSetConfig};

    fn prov() -> ResponseProvenance {
        ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap()
    }

    fn req_prov() -> RequestProvenance {
        RequestProvenance::new(InvestigationFrameId::from_content_digest(1), "test", None).unwrap()
    }

    fn empty_read_set() -> ReadSet {
        InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None }).finalize()
    }

    #[test]
    fn response_provenance_rejects_empty_provider() {
        assert!(ResponseProvenance::new("", "m", None, None).is_err());
        assert!(ResponseProvenance::new("p", "", None, None).is_err());
    }

    #[test]
    fn a_hypotheses_response_is_constructible() {
        let id = InvestigationFrameId::from_content_digest(1);
        let r = LlmResponse::new_hypotheses(id, 42, req_prov(), prov(), empty_read_set(), vec![]);
        assert_eq!(r.frame_id(), id);
        assert_eq!(r.request_digest(), 42);
        assert!(!r.read_anything());
    }

    #[test]
    fn an_advisory_response_carries_its_summary() {
        let id = InvestigationFrameId::from_content_digest(1);
        let r = LlmResponse::new_advisory(
            id,
            1,
            req_prov(),
            prov(),
            empty_read_set(),
            "look at line 17",
        );
        match r.output() {
            ResponseOutput::Advisory { summary } => assert_eq!(summary, "look at line 17"),
            _ => panic!("expected Advisory output"),
        }
    }
}
