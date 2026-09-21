//! FakeLlmPort — deterministic in-memory adapter for `LlmPort`.
//!
//! Used by:
//! - the acceptance suite (no live external provider required),
//! - unit tests,
//! - the deterministic verification of every AI-adjacent cycle
//!   (e79, future e80+).
//!
//! The fake is **scripted**: each (frame_id, request_digest) maps to
//! one pre-baked response. If the mapping is empty, the fake returns
//! [`LlmPortError::NoResponseForFrame`] — the caller decides whether
//! to fall back, retry, or fail.
//!
//! The fake is **deterministic**: given the same script and the same
//! request, it produces the same response. No wall-clock, no entropy,
//! no I/O.
//!
//! Real provider adapters (OpenAI, Anthropic, Ollama, …) are future
//! work and are gated by DEBT-SDDK-003 (provider/worker outage).
//! They are NOT in scope for e79's deterministic closure.

use std::collections::HashMap;

use crate::domain::ai::frame::{InvestigationFrame, InvestigationFrameId};
use crate::domain::ai::port::{LlmPort, LlmPortError};
use crate::domain::ai::request::InvestigationRequest;
use crate::domain::ai::response::LlmResponse;

/// Key under which a scripted response is stored.
///
/// The fake indexes responses by `(frame_id, request_digest)`. This is
/// the smallest unique pair: same frame with different questions /
/// tools must not collide; same request twice must.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptKey {
    /// The frame the request was derived from.
    pub frame_id: InvestigationFrameId,
    /// The request's content digest.
    pub request_digest: u64,
}

impl ScriptKey {
    /// Convenience constructor.
    pub const fn new(frame_id: InvestigationFrameId, request_digest: u64) -> Self {
        Self {
            frame_id,
            request_digest,
        }
    }
}

/// The deterministic in-memory adapter.
#[derive(Debug, Default, Clone)]
pub struct FakeLlmPort {
    script: HashMap<ScriptKey, LlmResponse>,
    /// Fallback response consulted when the script has no entry. If
    /// `None`, the fake errors with `NoResponseForFrame`.
    fallback: Option<LlmResponse>,
}

impl FakeLlmPort {
    /// Empty fake. Every call returns `NoResponseForFrame` until a
    /// script is added.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a scripted response for a `(frame_id, request_digest)`
    /// pair. Calling `complete` with a matching request will return
    /// `response` unchanged.
    pub fn with_scripted(mut self, key: ScriptKey, response: LlmResponse) -> Self {
        self.script.insert(key, response);
        self
    }

    /// Register a fallback response. Used when no scripted entry
    /// matches.
    pub fn with_fallback(mut self, response: LlmResponse) -> Self {
        self.fallback = Some(response);
        self
    }

    /// The number of scripted responses.
    pub fn script_len(&self) -> usize {
        self.script.len()
    }

    /// Whether the fake has a fallback.
    pub fn has_fallback(&self) -> bool {
        self.fallback.is_some()
    }
}

impl LlmPort for FakeLlmPort {
    fn complete(
        &self,
        request: &InvestigationRequest,
        _frame: &InvestigationFrame,
    ) -> Result<LlmResponse, LlmPortError> {
        let key = ScriptKey::new(request.frame_id(), request.content_digest());
        if let Some(r) = self.script.get(&key) {
            // Lineage contract: the port must echo the request's frame_id
            // and provenance. The scripted response already does this if
            // the test author was honest; we do not re-validate here
            // (that is the lineage layer's job, not the port's).
            return Ok(r.clone());
        }
        if let Some(fb) = &self.fallback {
            return Ok(fb.clone());
        }
        Err(LlmPortError::NoResponseForFrame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ai::InvestigationFrameId;
    use crate::domain::ai::request::{RequestProvenance, ToolCall};
    use crate::domain::ai::response::{ResponseOutput, ResponseProvenance};

    fn req(frame_id: InvestigationFrameId, _digest_seed: u64) -> InvestigationRequest {
        let prov = RequestProvenance::new(frame_id, "test", None).unwrap();
        InvestigationRequest::try_new(
            frame_id,
            "q",
            vec![ToolCall::new("noop", Vec::<String>::new()).unwrap()],
            prov,
        )
        .unwrap()
    }

    fn frame_id(n: u64) -> InvestigationFrameId {
        InvestigationFrameId::from_content_digest(n)
    }

    fn prov() -> ResponseProvenance {
        ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap()
    }

    #[test]
    fn an_empty_fake_returns_no_response() {
        let f = FakeLlmPort::new();
        let id = frame_id(1);
        let r = req(id, 1);
        let err = f
            .complete(
                &r,
                &crate::domain::ai::InvestigationFrame::try_new(
                    crate::domain::execution::ExecutionContext::try_new(
                        crate::domain::kernel_ids::ExecutionId::new(1),
                        crate::domain::execution::AnalysisScope::new(
                            crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
                            crate::domain::kernel_ids::SnapshotId::new(1),
                        ),
                        crate::domain::execution::ActorRef::agent("test"),
                        crate::domain::execution::CorrelationId::new("c").unwrap(),
                        None,
                    )
                    .unwrap(),
                    "q",
                    crate::domain::ai::InvestigationScope::empty(
                        crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
                        crate::domain::kernel_ids::SnapshotId::new(1),
                    ),
                    crate::domain::ai::InvestigationBudget::default(),
                )
                .unwrap(),
            )
            .unwrap_err();
        assert!(matches!(err, LlmPortError::NoResponseForFrame));
    }

    #[test]
    fn a_scripted_response_is_returned_for_matching_request() {
        let id = frame_id(42);
        let req_prov = RequestProvenance::new(id, "test", None).unwrap();
        let r = req(id, 1);
        let scripted = LlmResponse::new_advisory(
            id,
            r.content_digest(),
            req_prov.clone(),
            prov(),
            crate::domain::readset::InMemoryReadSetRecorder::new(
                crate::domain::readset::ReadSetConfig { max_records: None },
            )
            .finalize(),
            "scripted",
        );
        let f = FakeLlmPort::new()
            .with_scripted(ScriptKey::new(id, r.content_digest()), scripted.clone());
        assert_eq!(f.script_len(), 1);

        // We can't easily build a real frame here without a context; use
        // a dummy frame and trust the port doesn't validate the frame.
        let frame = crate::domain::ai::InvestigationFrame::try_new(
            crate::domain::execution::ExecutionContext::try_new(
                crate::domain::kernel_ids::ExecutionId::new(1),
                crate::domain::execution::AnalysisScope::new(
                    crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
                    crate::domain::kernel_ids::SnapshotId::new(1),
                ),
                crate::domain::execution::ActorRef::agent("test"),
                crate::domain::execution::CorrelationId::new("c").unwrap(),
                None,
            )
            .unwrap(),
            "q",
            crate::domain::ai::InvestigationScope::empty(
                crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
                crate::domain::kernel_ids::SnapshotId::new(1),
            ),
            crate::domain::ai::InvestigationBudget::default(),
        )
        .unwrap();
        let got = f.complete(&r, &frame).unwrap();
        assert_eq!(got, scripted);
    }

    #[test]
    fn a_fallback_is_returned_when_no_script_matches() {
        let id = frame_id(7);
        let r = req(id, 7);
        let fb = LlmResponse::new_advisory(
            id,
            r.content_digest(),
            RequestProvenance::new(id, "fb", None).unwrap(),
            prov(),
            crate::domain::readset::InMemoryReadSetRecorder::new(
                crate::domain::readset::ReadSetConfig { max_records: None },
            )
            .finalize(),
            "fallback",
        );
        let f = FakeLlmPort::new().with_fallback(fb.clone());
        assert!(f.has_fallback());
        let frame = crate::domain::ai::InvestigationFrame::try_new(
            crate::domain::execution::ExecutionContext::try_new(
                crate::domain::kernel_ids::ExecutionId::new(1),
                crate::domain::execution::AnalysisScope::new(
                    crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
                    crate::domain::kernel_ids::SnapshotId::new(1),
                ),
                crate::domain::execution::ActorRef::agent("test"),
                crate::domain::execution::CorrelationId::new("c").unwrap(),
                None,
            )
            .unwrap(),
            "q",
            crate::domain::ai::InvestigationScope::empty(
                crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
                crate::domain::kernel_ids::SnapshotId::new(1),
            ),
            crate::domain::ai::InvestigationBudget::default(),
        )
        .unwrap();
        let got = f.complete(&r, &frame).unwrap();
        match got.output() {
            ResponseOutput::Advisory { summary } => assert_eq!(summary, "fallback"),
            _ => panic!("expected advisory fallback"),
        }
    }
}
