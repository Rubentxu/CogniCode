# Tasks — cycle e63 — Intelligence Event Log foundation (M7.1, U50)

> Cycle: A-lite | Milestone: M7 | Phase: tasks | Date: 2026-09-15

Requirement: U50 (umbrella `cognicode-living-software-intelligence`). This
evolution adds no spec delta of its own.

## WU0 — inventory and contracts

- [x] Boundary written down: log = history, bus = delivery, runtime = consumer.
- [x] Reuse M6's `AnalysisScope` for an event's scope (no parallel notion of
      execution context; M7.2 generalises, it does not replace).
- [x] Reuse the `namespace.name` grammar by lifting it to `domain::naming`
      (e56 precedent) instead of writing a second validator.

## WU1 — identities and kinds

- [x] `EventId` in the ungated `domain::kernel_ids` (`evt:N` Display).
- [x] `CorrelationId` (non-empty, opaque, serde-validated).
- [x] `EventKind` = validated namespaced name + `EventKinds` published
      constructors + "every published kind is well formed" test.
- [x] `ActorKind` / `ActorRef` with `(kind, id)` and a rejecting constructor.

## WU2 — the event

- [x] `NewIntelligenceEvent` (no id) and `IntelligenceEvent` (immutable).
- [x] `EventTime` supplied by the caller — no clock in the domain.
- [x] `validate` on what can be checked before an id exists.

## WU6 — payload model

- [x] `ContentDigest` (`sha256:<64 hex>`, content-addressed, serde-validated).
- [x] `BoundedEventPayload` with the byte budget enforced on every mutation and
      stable field ordering.
- [x] `EventPayloadRef::{Inline, Artifact}` + constructors and accessors.

## WU3 — the store port

- [x] `IntelligenceEventStore`: `append`, `by_id`, `causal_chain`,
      `by_correlation`, `replay`, `len`/`is_empty`, `scope_of`.
- [x] No `update`, `delete`, `publish`, `subscribe`.
- [x] `EventStoreError` including `UnknownCause`, `CauseInAnotherWorkspace`,
      `SelfCaused`, `Corrupt`, `NoPreviousEvent`.

## WU4 — in-memory append-only oracle

- [x] `InMemoryEventLog`: global id sequence, atomic append, cause must exist,
      immutability, workspace isolation.

## WU5 — causal traversal and correlation

- [x] `causal_chain` root-first, cycle-detecting, failing loud on a dangling
      edge.
- [x] `replay` deterministic and incremental (`after`).

## WU7 — instrument a real causal slice

- [x] `CausalRecorder` (application): `record_root` / `record_next` /
      `record_caused_by`, never guessing a cause.
- [x] `summary_payload` / `counted_payload` helpers.

## WU8 — the U50 acceptance test

- [x] `tests/intelligence_event_log_e2e.rs` (feature `evidence-kernel`) drives
      the real kernel commit, the real M6 executor and the canonical bridge, and
      asserts a finding that verifies and gates.
- [x] `causal_chain == [SourceDelta, FactBatchCommitted, AnalysisCompleted,
      FindingProduced]`, with every `caused_by` edge checked.
- [x] Replay reproducible and incremental.
- [x] 500 facts → one artifact event whose digest identifies the bytes.
- [x] Correlation and actor attribution across two actors.

## Deliberately not built (recorded)

- [ ] Behaviors / `BehaviorClass` / authority table (e64, U52).
- [ ] Budgets and observable exhaustion (M7.4 / e65).
- [ ] Read sets and invalidation (e66, U61).
- [ ] Scheduler, transport, CloudEvents, event bus, durable adapter.
