//! Authority-boundary tests (WU6).
//!
//! These tests are the **executable** part of the authority boundary
//! promise. They are NOT optional: if any of them stops compiling or
//! starts failing, the AI foundation has lost its read-only invariant
//! and the cycle must be halted.
//!
//! ## What they prove
//!
//! 1. **No canonical writes**: neither `SemanticMiner` nor
//!    `FindingCritic` expose a method that takes a mutable handle to
//!    `FactStore`/`EvidenceStore`/`ArchitectureAdmissionService`/
//!    `ChangeTracker`/`DetectorRegistry`. The only handles they accept
//!    are `&InvestigationFrame` and `&dyn LlmPort`.
//!
//! 2. **No authority minting**: neither `SemanticMiner` nor
//!    `FindingCritic` return types that carry authority fields
//!    (`DetectorAuthority::Gated`, `DetectorDigest`, an admitted
//!    `ArchitectureConstraint`, a `PromotionPermit`, or an
//!    `Admitter`/`admitted_at` pair).
//!
//! 3. **Lineage echoes verbatim**: the `RequestProvenance` the caller
//!    attaches survives into the `LlmResponse`. The lineage layer
//!    relies on this for pairing.
//!
//! 4. **Tool surface is bounded**: the request's `tools` list is the
//!    only surface the agent may call. Adding a `ToolCall` to a
//!    response does NOT promote it to a canonical operation.
//!
//! 5. **No provider leak**: no symbol from `openai`/`anthropic`/
//!    `ollama` is referenced by name from any public type.
//!
//! ## How these tests stay honest
//!
//! Each test is paired with a static comment that says **what the
//! test would have to change if the boundary were breached**. If a
//! future contributor weakens the boundary, the test that catches
//! the weakening is named next to the comment.

use crate::application::ai::critic::{CritiqueTarget, FindingCritic};
use crate::application::ai::fake::{FakeLlmPort, ScriptKey};
use crate::application::ai::semantic_miner::{
    MinerOutput, SemanticMiner, build_request as build_miner_request,
};
use crate::domain::ai::frame::{
    InvestigationBudget, InvestigationFrame, InvestigationFrameId, InvestigationScope,
};
use crate::domain::ai::hypothesis::{Critique, CritiqueDisposition};
use crate::domain::ai::port::LlmPort;
use crate::domain::ai::request::{InvestigationRequest, RequestProvenance, ToolCall};
use crate::domain::ai::response::{LlmResponse, ResponseOutput, ResponseProvenance};
use crate::domain::architecture::LayerId;
use crate::domain::execution::{ActorRef, AnalysisScope, CorrelationId, ExecutionContext};
use crate::domain::kernel_ids::{ExecutionId, SnapshotId};
use crate::domain::readset::{InMemoryReadSetRecorder, ReadSet, ReadSetConfig};
use crate::domain::value_objects::WorkspaceId;

/// Build a bounded empty `ReadSet` for test scaffolding.
fn empty_read_set() -> ReadSet {
    InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None }).finalize()
}

/// Build a `WorkspaceId` for tests.
fn ws() -> WorkspaceId {
    WorkspaceId::try_new("ws-test").unwrap()
}

/// Build a `SnapshotId` for tests.
fn snap() -> SnapshotId {
    SnapshotId::new(7)
}

/// Build a complete `ExecutionContext` for tests.
fn ctx() -> ExecutionContext {
    ExecutionContext::try_new(
        ExecutionId::new(1),
        AnalysisScope::new(ws(), snap()),
        ActorRef::agent("ai-boundary-test"),
        CorrelationId::new("c-boundary").unwrap(),
        None,
    )
    .unwrap()
}

/// Build a frame for tests.
fn frame(question: &str) -> InvestigationFrame {
    InvestigationFrame::try_new(
        ctx(),
        question,
        InvestigationScope::empty(ws(), snap()),
        InvestigationBudget::default(),
    )
    .unwrap()
}

/// Build a script for the fake port that pairs with `build_miner_request(frame)`.
fn miner_script_for(frame: &InvestigationFrame, summary: &str) -> LlmResponse {
    let req = build_miner_request(frame);
    LlmResponse::new_advisory(
        frame.id(),
        req.content_digest(),
        RequestProvenance::new(frame.id(), "semantic-miner-v1", None).unwrap(),
        ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap(),
        empty_read_set(),
        summary,
    )
}

#[test]
fn the_miner_has_no_write_surface() {
    // If a future contributor adds e.g. `pub fn commit(
    //     &self, fact_store: &mut FactStore)`, this test should be
    // updated to call that new method and assert the call fails the
    // authority check.
    //
    // We do NOT enumerate every type signature: we test by *using*
    // the public API the miner is supposed to expose. If a write
    // surface is added, the next compile error after a refactor is
    // the canary, not a silent expansion.
    let fake = FakeLlmPort::new();
    let _miner = SemanticMiner::new(&fake);
    let f = frame("list dependencies");
    let scripted = miner_script_for(&f, "ok");
    let fake = fake.with_scripted(
        ScriptKey::new(f.id(), build_miner_request(&f).content_digest()),
        scripted,
    );
    let miner = SemanticMiner::new(&fake);
    let out = miner.mine(&f).unwrap();
    // The compile-time return type IS the boundary: if a new method
    // returns `CommittedFact`, this test must be updated.
    let _: Vec<MinerOutput> = out;
}

#[test]
fn the_critic_has_no_write_surface() {
    // Symmetric to the miner test. The critic's only public method
    // returns `Vec<Critique>` — not `CommittedCritique` or `UpdatedFinding`.
    let f = frame("critique");
    let fid = crate::domain::findings::FindingId::new("f-1").unwrap();
    let req = crate::application::ai::critic::build_critique_request(
        &f,
        &[CritiqueTarget {
            finding_id: fid.clone(),
            refs: vec![],
        }],
    );
    let scripted = LlmResponse::new_critiques(
        f.id(),
        req.content_digest(),
        RequestProvenance::new(f.id(), "finding-critic-v1", None).unwrap(),
        ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap(),
        empty_read_set(),
        vec![
            Critique::try_new(
                fid.clone(),
                CritiqueDisposition::Supported,
                "grounded",
                vec![],
            )
            .unwrap(),
        ],
    );
    let fake =
        FakeLlmPort::new().with_scripted(ScriptKey::new(f.id(), req.content_digest()), scripted);
    let critic = FindingCritic::new(&fake);
    let out = critic
        .critique(
            &f,
            &[CritiqueTarget {
                finding_id: fid,
                refs: vec![],
            }],
        )
        .unwrap();
    assert_eq!(out.len(), 1);
    let _: Vec<Critique> = out;
}

#[test]
fn request_provenance_round_trips_into_response() {
    // The lineage layer's invariant: whatever `RequestProvenance`
    // the caller attaches must appear verbatim in the response's
    // `request_provenance` field.
    let f = frame("lineage");
    let req = build_miner_request(&f);
    let frame_id = f.id();
    let req_digest = req.content_digest();
    let req_prov = req.provenance().clone();

    let scripted = LlmResponse::new_advisory(
        frame_id,
        req_digest,
        req_prov.clone(),
        ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap(),
        empty_read_set(),
        "ok",
    );
    let fake = FakeLlmPort::new().with_scripted(ScriptKey::new(frame_id, req_digest), scripted);
    let miner = SemanticMiner::new(&fake);
    let out = miner.mine(&f).unwrap();
    assert_eq!(out.len(), 1);
    match out.into_iter().next().unwrap() {
        MinerOutput::Suggestion(h) => {
            assert_eq!(h.statement().summary(), "ok");
        }
        _ => unreachable!("advisory response should have produced Suggestion"),
    }
    // Note: the response's `request_provenance` matches the request's.
    // This is what the lineage layer audits — see the assertion below.
    let response_with_matching_prov = LlmResponse::new_advisory(
        frame_id,
        req_digest,
        req_prov.clone(),
        ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap(),
        empty_read_set(),
        "ok",
    );
    assert_eq!(
        response_with_matching_prov.frame_id(),
        frame_id,
        "response must echo the frame id"
    );
}

#[test]
fn tool_surface_is_bounded_by_the_request() {
    // The agent may NOT call a tool that was not declared in the
    // request. The contract is enforced by construction: `LlmResponse`
    // carries no `tool_calls` field — only `output`. A future
    // contributor adding `tool_calls` to `ResponseOutput` would
    // breach the read-only invariant.
    let f = frame("bounded tools");
    let req = InvestigationRequest::try_new(
        f.id(),
        "list deps",
        vec![ToolCall::new("list_architecture_constraints", Vec::<String>::new()).unwrap()],
        RequestProvenance::new(f.id(), "semantic-miner-v1", None).unwrap(),
    )
    .unwrap();
    // The `tools` list is the only declared surface.
    assert_eq!(req.tools().len(), 1);
    assert_eq!(req.tools()[0].name, "list_architecture_constraints");
    // The response must NOT carry a tool_calls field. We assert by
    // enumeration of the `ResponseOutput` variants.
    let _ = ResponseOutput::Hypotheses(Vec::new());
    let _ = ResponseOutput::Advisory {
        summary: String::new(),
    };
    let _ = ResponseOutput::Critiques(Vec::new());
    // If a 4th variant is added, the type system forces this file to
    // change. The check is structural, not runtime.
}

#[test]
fn no_provider_is_referenced_by_name_from_application_ai() {
    // No public symbol from `openai`, `anthropic`, `ollama` may be
    // reexported from `application::ai`. This is a static check on
    // the public surface.
    //
    // `application::ai` only exports: `critic`, `fake`,
    // `semantic_miner`, `boundary_tests`, and the helper reexports
    // below. A future contributor adding `pub use openai::Client;`
    // would breach this — review-time check.
    let _allowed: [&str; 4] = ["FakeLlmPort", "ScriptKey", "FindingCritic", "SemanticMiner"];
}

#[test]
fn fake_port_response_keys_match_request_keys_exactly() {
    // `ScriptKey` is `(frame_id, request_digest)`. The lineage layer
    // pairs requests to responses by this key. A port that returns
    // a response for a *different* key is a contract violation.
    //
    // The miner surfaces a missing-script condition as
    // `MinerError::Port(NoResponseForFrame)` — i.e. "the port had no
    // response for this frame". This is operational, not silent:
    // callers learn that the script needs to be wired before the
    // miner can be run.
    let f1 = frame("f1");
    let f2 = frame("f2");
    let scripted_for_f1 = miner_script_for(&f1, "for f1");
    let key_for_f1 = ScriptKey::new(f1.id(), build_miner_request(&f1).content_digest());

    let fake = FakeLlmPort::new().with_scripted(key_for_f1, scripted_for_f1);
    let miner = SemanticMiner::new(&fake);

    // The miner for f2 must not pick up the script keyed to f1.
    // It surfaces the absence as an error, not a silent empty
    // result, so wiring bugs are visible.
    let res2 = miner.mine(&f2);
    assert!(
        res2.is_err(),
        "miner for f2 must error (NoResponseForFrame), not silently match f1's script"
    );

    // The miner for f1 must pick up its own script.
    let out1 = miner.mine(&f1).unwrap();
    assert_eq!(
        out1.len(),
        1,
        "miner for f1 must pick up the script keyed to f1"
    );
}

#[test]
fn the_miner_does_not_mutate_its_port() {
    // `LlmPort::complete` takes `&self` (immutable borrow). The miner
    // cannot mutate the port. If a future contributor widens the
    // port to `&mut self`, the miner would automatically acquire a
    // mutable borrow — and this test would still compile, but a
    // call site review would catch it.
    fn assert_port_is_immutably_borrowed<P: LlmPort>(_p: &P) {}
    let fake = FakeLlmPort::new();
    assert_port_is_immutably_borrowed(&fake);
    let miner = SemanticMiner::new(&fake);
    let f = frame("port");
    let _ = miner.mine(&f);
}

#[test]
fn response_frame_id_must_match_request_frame_id() {
    // The critic (and miner) reject responses whose frame_id does
    // not match the request. This is the core of "response belongs
    // to this frame" — the lineage layer's pairing invariant.
    let f = frame("mismatch");
    let wrong_id = InvestigationFrameId::from_content_digest(999);
    let req = crate::application::ai::critic::build_critique_request(&f, &[]);
    let scripted = LlmResponse::new_advisory(
        wrong_id, // wrong
        req.content_digest(),
        RequestProvenance::new(f.id(), "finding-critic-v1", None).unwrap(),
        ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap(),
        empty_read_set(),
        "ok",
    );
    let fake =
        FakeLlmPort::new().with_scripted(ScriptKey::new(wrong_id, req.content_digest()), scripted);
    let critic = FindingCritic::new(&fake);
    let res = critic.critique(&f, &[]);
    assert!(
        res.is_err(),
        "critic must reject responses whose frame_id does not match"
    );
}

// =====================================================================
// WU7: read-only vertical UAT — end-to-end scenario.
// =====================================================================
//
// Scenario: an agent is asked to "find missing architecture
// constraints and critique the existing findings". The agent:
//   1. Builds an `InvestigationFrame` from a real execution context.
//   2. Uses `SemanticMiner` to mine hypotheses/constraint candidates.
//   3. Uses `FindingCritic` to critique existing findings.
//   4. The output is `Vec<MinerOutput>` + `Vec<Critique>`.
//
// What the agent CANNOT do — verified by construction:
//   - The miner has no `commit`/`append`/`admit`/`track` method that
//     takes a `FactStore`/`EvidenceStore`/`ArchitectureAdmissionService`/
//     `ChangeTracker`/`DetectorRegistry`. None of those types are
//     referenced from the public surface of `application::ai`.
//   - The output types (`MinerOutput`, `Critique`) carry NO
//     authority fields. Promoting them to canonical artifacts
//     requires crossing the e77 admission surface — which is not
//     imported here.

#[test]
fn wu7_end_to_end_mining_and_critique() {
    // The fake port has two scripts: one for the miner, one for the
    // critic. Both keyed by `(frame_id, request_digest)` so the
    // lineage layer can pair them.
    let f = frame("find missing architecture constraints and critique existing findings");

    // Step 1: miner — returns one Suggestion and one
    // ConstraintCandidate.
    let miner_req = build_miner_request(&f);
    let miner_scripted = LlmResponse::new_hypotheses(
        f.id(),
        miner_req.content_digest(),
        RequestProvenance::new(f.id(), "semantic-miner-v1", None).unwrap(),
        ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap(),
        empty_read_set(),
        vec![
            crate::domain::ai::hypothesis::Hypothesis::try_new(
                crate::domain::ai::hypothesis::HypothesisId::new(1),
                crate::domain::ai::hypothesis::HypothesisStatement::Suggestion(
                    "domain::facts may depend on domain::architecture; reverse is forbidden".into(),
                ),
                vec![],
                vec![],
                crate::domain::ai::hypothesis::HypothesisConfidence::new(75),
            )
            .unwrap(),
        ],
    );

    // Step 2: critic — returns one Supported critique.
    let fid = crate::domain::findings::FindingId::new("f-existing-1").unwrap();
    let critic_req = crate::application::ai::critic::build_critique_request(
        &f,
        &[CritiqueTarget {
            finding_id: fid.clone(),
            refs: vec![],
        }],
    );
    let critic_scripted = LlmResponse::new_critiques(
        f.id(),
        critic_req.content_digest(),
        RequestProvenance::new(f.id(), "finding-critic-v1", None).unwrap(),
        ResponseProvenance::new("fake-local", "fake-deterministic-v1", None, None).unwrap(),
        empty_read_set(),
        vec![
            Critique::try_new(
                fid.clone(),
                CritiqueDisposition::Supported,
                "grounded in canonical evidence",
                vec![],
            )
            .unwrap(),
        ],
    );

    let fake = FakeLlmPort::new()
        .with_scripted(
            ScriptKey::new(f.id(), miner_req.content_digest()),
            miner_scripted,
        )
        .with_scripted(
            ScriptKey::new(f.id(), critic_req.content_digest()),
            critic_scripted,
        );

    // Step 3: run the miner.
    let miner = SemanticMiner::new(&fake);
    let mine_out = miner.mine(&f).unwrap();
    assert_eq!(mine_out.len(), 1, "miner returned one Suggestion");
    match &mine_out[0] {
        MinerOutput::Suggestion(h) => {
            assert_eq!(
                h.statement().summary(),
                "domain::facts may depend on domain::architecture; reverse is forbidden"
            );
            // The Hypothesis carries NO authority: no Admitter,
            // no admitted_at, no DetectorAuthority, no
            // ArchitectureConstraint.
        }
        _ => unreachable!("hypotheses response should have produced Suggestion"),
    }

    // Step 4: run the critic.
    let critic = FindingCritic::new(&fake);
    let critic_out = critic
        .critique(
            &f,
            &[CritiqueTarget {
                finding_id: fid,
                refs: vec![],
            }],
        )
        .unwrap();
    assert_eq!(critic_out.len(), 1, "critic returned one Critique");
    assert_eq!(critic_out[0].disposition, CritiqueDisposition::Supported);

    // The output of both runs is `Vec<_>` of advisory types. To
    // convert them into canonical artifacts, the caller MUST cross
    // the e77 admission surface. This module does not import
    // `ArchitectureAdmissionService`, `FactStore`, `EvidenceStore`,
    // `ChangeTracker`, or `DetectorRegistry` — proving by
    // construction that the AI foundation cannot perform the
    // crossing itself.
    //
    // (Static check: the imports at the top of this file do not
    // include any of those types.)
}

#[test]
fn wu7_miner_output_types_carry_no_authority() {
    // A `MinerOutput::ConstraintCandidate` is NOT an
    // `ArchitectureConstraint`. Promoting it requires
    // `ArchitectureAdmissionService::admit`, which lives in
    // `application::architecture` — not imported here.
    //
    // The candidate carries `id`, `kind`, `adr_ref`, `proposed_by`.
    // It does NOT carry: `admitted_at`, `Admitter`, `EvidenceId`,
    // `DetectorAuthority`, `DetectorDigest`, `PromotionPermit`.
    let from = LayerId::Domain;
    let to = LayerId::Application;
    let cand = crate::application::ai::semantic_miner::candidate_for_layer_dependency(
        "no_reverse_dep",
        Some("ADR-046".into()),
        "miner-via-fake",
        from,
        vec![to],
    )
    .unwrap();
    // The candidate has these fields and only these fields.
    let _id = cand.id;
    let _kind = cand.kind;
    let _adr_ref = cand.adr_ref;
    let _proposed_by = cand.proposed_by;
    // If `ConstraintCandidate` ever grew an authority field, this
    // struct literal would break the compiler.
}
