# Archive Manifest — cycle e64 — execution identity + behavior authority (M7.2/M7.3, U52)

> Cycle: A-lite | Milestone: M7 | Phase: archive | Date: 2026-09-15

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e64 |
| Milestone | M7.2 (execution identity) + M7.3 (behavior model) — **complete** |
| Requirement | U52 — **CLOSED** |
| Path | A-lite |
| Base HEAD | `9a70c133` (e63 archive) |
| Spec delta | none |

## Commits

| Commit | Kind | Summary |
|--------|------|---------|
| `5f906d58` | `fix` | e63 preflight: scope/workspace agreement, unpinned scope refused, `depth()` removed |
| `b483b6b5` | `refactor` | execution vocabulary lifted; the detector path adopts `ExecutionContext` |
| `d6755c68` | `feat` | behavior class, admission, sealed permit, governed runtime, U52 |
| (archive) | `docs(openspec)` | e64 artifacts + state |

## Delivered

### WU0 — e63 preflight

- `EventStoreError::ScopeWorkspaceMismatch` (the store and the event must agree
  on whose history this is).
- `EventError::InvalidScope` (an unpinned scope is refused by the store's gate,
  since the field is public).
- `IntelligenceEvent::depth()` removed: it could only ever return 0 or 1.

### WU1 — general execution vocabulary

- `domain::execution::{AnalysisScope, ActorKind, ActorRef, CorrelationId,
  ExecutionContext}`, `domain::trust::AdmissionSource`, each with re-export shims
  at its old path.
- `ExecutionContext` rejects an unpinned scope; `trigger_event` is deliberately
  not `caused_by`.

### WU2 — M6 adoption

- `DetectorExecutionRef.context` replaces the separate `execution_id`/`scope`
  fields; accessors keep call sites readable; authority stays outside.
- `ExecutionRequest` (execution_id, scope, actor, correlation, trigger_event)
  turned into a context *by the executor*, so the trust boundary cannot be
  skipped.
- `ExecutionError::InputScopeMismatch`: the views' scope and the execution's
  claimed scope must agree.

### WU3/WU4 — behavior authority

- `BehaviorClass`, `BehaviorEffectKind`, `BehaviorAuthorityPolicy` (the 15-cell
  table asserted row by row).
- `BehaviorDefinition`, `AdmissionError`, `AdmittedBehavior`,
  `BehaviorAdmission`, sealed `BehaviorPermit` (private fields + seal, no serde).
- Downgrade rule: `AiGenerated`/`Imported` → `AgentBehavior`. A declaration never
  escalates.

### WU5/WU6/WU7 — runtime and acceptance

- `BehaviorRuntime` (permit → start → authorize → accept/refuse → record), with
  the refusal behind a structural branch that never reaches the sink.
- `PolicyViolation` + `policy.behavior_output_rejected` with a bounded payload.
- U52 over a real fact store, with a positive control proving the refusal is
  about the class and not the payload.

## Evidence

| Check | Result |
|-------|--------|
| `domain::behaviors` | 13 passed |
| `domain::execution` | 10 passed |
| `domain::findings` | 112 passed |
| event log (domain + infra + application) | 32 passed |
| findings E2E (AST/Graph/Dataflow/Axiom) | 8 / 4 / 7 / 3 passed |
| `findings_canonical_grounding_e2e` (gated) | 10 passed |
| `intelligence_event_log_e2e` (gated) | 4 passed |
| `behavior_authority_e2e` (gated, acceptance) | 6 passed |
| `cargo check --workspace --all-targets` | 0 errors (gated and ungated) |
| `cargo fmt --all --check` | clean |
| `scripts/check_known_failures.sh` baseline | exit 0 — 41 entries, unchanged |

## Durable knowledge learned

- Identity and authority must be separate types: "who ran it" must never become
  "what it may do".
- `trigger_event` (an execution's origin) and `caused_by` (an edge between two
  events) are different relations; sharing one word gives a causal graph edges
  that were never true.
- The class of a behavior is not derived from the kind of actor: a human can run
  an agent behavior and an agent can request a trusted analysis.
- Model **effects**, not deliverables. "May fabricate a finding" would have
  opened a second path into findings and undone M6's single seam.
- A declaration never escalates: downgrade at admission rather than reject, so an
  AI-proposed pure derivation still *runs* (as an agent) instead of failing.
- Refuse before the adapter, never after: a rejection an adapter must undo is not
  a boundary.
- Public fields make constructor checks decorative. Both e63 holes came from
  validating only at construction.
- Two "scope" questions (where the views came from / what the run claims) are
  different questions that must have the same answer.

## Deferred (open work, recorded)

- e65: budgets and observable exhaustion (`behavior.budget_exhausted` is reserved
  vocabulary today).
- e66: read sets, deduplication, truncation, invalidation (U61).
- Scheduler / event matching / retry / workers / durable queue / transport.
- A durable event-log adapter (e63's honest boundary, unchanged).

## Milestone status

M7.1 (event kernel), M7.2 (execution identity) and M7.3 (behavior model, U52) are
complete. M7.4 (budgets, e65) and M7.5 (read sets, U61, e66) remain — the system
can now say who ran and what they may do; next it must say how much they may do,
and what they read.
