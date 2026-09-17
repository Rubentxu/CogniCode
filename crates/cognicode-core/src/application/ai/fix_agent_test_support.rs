//! Shared fixtures for the e80b FixAgent test suites.
//!
//! Used by `fix_agent_tests.rs` (WU8 adversarial cases + WU9 integration) and
//! `patch_sink.rs` unit tests.

use crate::application::ai::fake::FakeLlmPort;
use crate::application::ai::fix_agent::build_fix_request;
use crate::domain::ai::frame::{
    InvestigationBudget, InvestigationFrame, InvestigationScope,
};
use crate::domain::ai::patch::SourcePatchCandidate;
use crate::domain::ai::request::InvestigationRequest;
use crate::domain::ai::response::{LlmResponse, ResponseProvenance};
use crate::domain::execution::{ActorRef, AnalysisScope, CorrelationId, ExecutionContext};
use crate::domain::kernel_ids::{ExecutionId, FactId, SnapshotId};
use crate::domain::readset::{InMemoryReadSetRecorder, ReadSet, ReadSetConfig};
use crate::domain::value_objects::WorkspaceId;

/// The fixture workspace.
pub fn workspace() -> WorkspaceId {
    WorkspaceId::try_new("ws").expect("non-empty workspace")
}

/// The fixture snapshot.
pub fn snapshot() -> SnapshotId {
    SnapshotId::new(7)
}

/// Build a `ReadSet` recording the given facts.
pub fn read_set(facts: &[FactId]) -> ReadSet {
    let mut recorder = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
    for fact in facts {
        let _ = recorder.record(*fact);
    }
    recorder.finalize()
}

/// A frame whose declared read set contains `declared`.
pub fn frame_with_declared(declared: &[FactId]) -> InvestigationFrame {
    let execution = ExecutionContext::try_new(
        ExecutionId::new(1),
        AnalysisScope::new(workspace(), snapshot()),
        ActorRef::agent("fix-agent"),
        CorrelationId::new("c-1").expect("non-empty correlation"),
        None,
    )
    .expect("valid execution context");
    let mut scope = InvestigationScope::empty(workspace(), snapshot());
    scope.declared_read_set = read_set(declared);
    InvestigationFrame::try_new(execution, "fix the failed test", scope, InvestigationBudget::default())
        .expect("valid frame")
}

/// A plain frame (empty declared read set).
pub fn frame() -> InvestigationFrame {
    frame_with_declared(&[])
}

/// The frame plus the request the FixAgent will build for it.
pub fn frame_and_request() -> (InvestigationFrame, InvestigationRequest) {
    let frame = frame();
    let request = build_fix_request(&frame);
    (frame, request)
}

/// Provider provenance for fixtures.
pub fn provenance() -> ResponseProvenance {
    ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None)
        .expect("non-empty provider/model")
}

/// A patch-candidate response with an explicit digest and observed read set.
pub fn response_with(
    frame: &InvestigationFrame,
    request: &InvestigationRequest,
    candidate: SourcePatchCandidate,
    observed: &[FactId],
    request_digest: u64,
) -> LlmResponse {
    LlmResponse::new_source_patch_candidate(
        frame.id(),
        request_digest,
        request.provenance().clone(),
        provenance(),
        read_set(observed),
        candidate,
    )
}

/// A well-formed patch-candidate response: correct frame, correct request
/// digest, and an empty observed read set.
pub fn response_for(frame: &InvestigationFrame, candidate: SourcePatchCandidate) -> LlmResponse {
    let request = build_fix_request(frame);
    response_with(frame, &request, candidate, &[], request.content_digest())
}

/// Backwards-friendly alias used by the sink tests.
pub fn passing_response_with_candidate(
    frame: &InvestigationFrame,
    request: &InvestigationRequest,
    candidate: SourcePatchCandidate,
) -> LlmResponse {
    response_with(frame, request, candidate, &[], request.content_digest())
}

/// A `FakeLlmPort` scripted to return `response` for the request the FixAgent
/// builds from `frame`.
pub fn scripted_port(frame: &InvestigationFrame, response: LlmResponse) -> FakeLlmPort {
    let request = build_fix_request(frame);
    FakeLlmPort::new().with_scripted(
        crate::application::ai::fake::ScriptKey::new(frame.id(), request.content_digest()),
        response,
    )
}
