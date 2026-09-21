# Archive Manifest — cycle e63 — Intelligence Event Log foundation (M7.1, U50)

> Cycle: A-lite | Milestone: M7 | Phase: archive | Date: 2026-09-15

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e63 |
| Milestone | M7.1 — Intelligence Event Kernel (**complete**) |
| Requirement | U50 — **CLOSED** |
| Path | A-lite |
| Base HEAD | `95f5761e` (M6 closed) |
| Spec delta | none |

## Commits

| Commit | Kind | Summary |
|--------|------|---------|
| `de57775d` | `feat(cognicode-core)` | Event log domain + in-memory oracle + causal recorder + the U50 acceptance test |
| (archive) | `docs(openspec)` | e63 artifacts + state |

## Delivered

### WU0/WU1 — inventory, identities, kinds

- `domain::naming` — the shared `namespace.name` grammar lifted out of M6 (e56
  precedent), re-exported by `findings::namespaced`.
- `EventId` in the ungated `domain::kernel_ids`; `CorrelationId`;
  `EventKind` + `EventKinds` (published constructors, open vocabulary);
  `ActorKind` / `ActorRef`.
- Reuse of M6's `AnalysisScope` rather than a parallel execution-context type.

### WU2/WU6 — the event and its payload

- `NewIntelligenceEvent` / `IntelligenceEvent` (immutable, no setters).
- `EventTime` supplied by the caller (no clock in the domain → replayable).
- `ContentDigest`, `BoundedEventPayload` (budget enforced on mutation, stable
  field order), `EventPayloadRef::{Inline, Artifact}`.

### WU3/WU4/WU5 — the store

- `IntelligenceEventStore` with exactly `append`, `by_id`, `causal_chain`,
  `by_correlation`, `replay` (+ `len`, `is_empty`, `scope_of`) and no
  `update`/`delete`/`publish`/`subscribe`.
- `InMemoryEventLog`: global id sequence, all-or-nothing append, cause must
  exist in the same workspace or earlier in the batch, immutability, isolation.
- `causal_chain` root-first with cycle detection.

### WU7/WU8 — instrumented slice and acceptance

- `CausalRecorder` (application): `record_root` / `record_next` /
  `record_caused_by`; never guesses a cause.
- `tests/intelligence_event_log_e2e.rs`: the real kernel commit + the real M6
  executor + the canonical bridge, producing a finding that verifies and gates,
  with the events recorded at those boundaries.

## Evidence

| Check | Result |
|-------|--------|
| `domain::findings` | 114 passed |
| `domain::naming` | 6 passed |
| event-log suites (domain + infra + application) | 31 passed |
| `findings_canonical_grounding_e2e` (gated) | 10 passed |
| `intelligence_event_log_e2e` (gated, acceptance) | 4 passed |
| `cargo check --workspace --all-targets` | 0 errors (gated and ungated) |
| `cargo fmt --all --check` | clean |
| `scripts/check_known_failures.py` | exit 0 — 41 entries, unchanged |

## Durable knowledge learned

- A causal log and an event bus are different components; fusing commit with
  delivery makes history a function of who was listening.
- An event vocabulary must be data (validated namespaced names), not an enum
  that every milestone reopens.
- The payload model, not a convention, is what keeps "a million facts" from
  becoming a million events: `Inline(bounded) | Artifact(digest)`.
- A cause must be validated at append time; a dangling causal edge makes every
  later "why did this happen?" answer silently wrong.
- Id sequences: the evidence kernel wants per-`(workspace, snapshot)`, the event
  log wants one global sequence. The right answer depends on what the id is
  *used to prove* — here, cross-tenant detectability.
- `occurred_at` supplied by the caller is what makes replay reproducible.
- A recorder must never infer a cause; inferring produces plausible history that
  is not true.

## Deferred (open work, recorded)

- e64: behaviors, `BehaviorClass`, the authority table, U52.
- e65: budgets and observable exhaustion.
- e66: read sets, deduplication, truncation, invalidation (U61).
- A durable event-log adapter, an event bus / transport layer, projections, and
  a scheduler: none of them before the semantics pinned here are exercised
  across a restart.

## Milestone status

M7.1 (Intelligence Event Kernel) is complete and U50 is closed. M7.2 (execution
identity), M7.3 (behavior model, U52), M7.4 (budgets) and M7.5 (read sets, U61)
remain, in that order — memory first, then reflexes.
