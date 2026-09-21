# Tasks — cycle e64 — execution identity + behavior authority (M7.2/M7.3, U52)

> Cycle: A-lite | Milestone: M7 | Phase: tasks | Date: 2026-09-15

Requirement: U52 (umbrella `cognicode-living-software-intelligence`). This
evolution adds no spec delta of its own.

## WU0 — e63 preflight hardening

- [x] `EventStoreError::ScopeWorkspaceMismatch`: an event's own scope must name
      the workspace it is appended to.
- [x] `EventError::InvalidScope`: a scope pinned to `SnapshotId::NONE` is
      refused by the store's validation gate.
- [x] `IntelligenceEvent::depth()` removed (it could only ever return 0 or 1).
- [x] Stale "ids are per workspace" comment corrected.
- [x] Tests: `an_event_may_not_claim_another_workspaces_scope`,
      `an_unpinned_scope_is_refused`, `an_unpinned_scope_is_rejected`.

## WU1 — generic execution vocabulary

- [x] `domain::execution::{AnalysisScope, ActorRef, ActorKind, CorrelationId,
      ExecutionContext}` + shims at every old path.
- [x] `domain::trust::AdmissionSource` + shim in `findings::admission`.
- [x] `ExecutionContext { execution_id, scope, actor, correlation, trigger_event }`
      rejecting an unpinned scope.

## WU2 — M6 adoption

- [x] `DetectorExecutionRef.context: Option<ExecutionContext>` replaces
      `execution_id` + `scope`; accessors keep call sites readable.
- [x] `ExecutionPermit::execution_ref(Option<ExecutionContext>)`.
- [x] `ExecutionRequest` + `ExecutionRequest::detector` + `triggered_by`.
- [x] `ExecutionError::InputScopeMismatch` when the views' scope and the
      execution's claimed scope disagree.
- [x] Zero semantic change: all four findings E2E suites and both acceptance
      tests pass unchanged.

## WU3 — behavior domain

- [x] `BehaviorClass { PureDerivation, ReactiveAnalysis, AgentBehavior }`.
- [x] `BehaviorEffectKind` (5 effects) and `BehaviorAuthorityPolicy` with the
      15-cell table asserted row by row.

## WU4 — behavior trust boundary

- [x] `BehaviorId`, `BehaviorDefinition` (rejecting a self-contradicting
      definition), `AdmittedBehavior`, `BehaviorAdmission`.
- [x] Sealed `BehaviorPermit` (private fields + seal, no serde, no public ctor).
- [x] Downgrade: `AiGenerated`/`Imported` → `AgentBehavior`; a declaration never
      escalates.

## WU5/WU6 — minimal runtime and policy rejection

- [x] `BehaviorRuntime::run(permit, context, behavior, sink)`.
- [x] `behavior.started` caused by `trigger_event`; authorize every effect
      against the effective class; refuse before the sink.
- [x] `PolicyViolation` + `policy.behavior_output_rejected` with a bounded
      payload (behavior_id, class, effect, reason, execution_id).
- [x] `behavior.completed` only when nothing was refused.

## WU7 — U52 acceptance

- [x] `tests/behavior_authority_e2e.rs` (feature `evidence-kernel`).
- [x] The agent attempts a **valid** fact draft; refused by class, not by
      provenance.
- [x] `FactStore before == after`, sink untouched, `effective_class ==
      AgentBehavior`.
- [x] `causal_chain(rejection) == [trigger, behavior.started,
      policy.behavior_output_rejected]`, with every `caused_by` edge and the
      event actor checked.
- [x] One correlation end to end; payload fields navigable.
- [x] Positive control: the same draft from a curated `PureDerivation` is
      accepted and actually commits.
- [x] A downgraded behavior keeps what its class still allows
      (`ProposeChange`); `ExecuteAdmittedAnalysis` grants no new authority.

## Deliberately not built (recorded)

- [ ] e65: budgets and observable exhaustion.
- [ ] e66: read sets, deduplication, truncation, invalidation (U61).
- [ ] Scheduler / event matching / retry / workers / durable queue / transport.
- [ ] A durable event-log adapter (e63's honest boundary, unchanged).
