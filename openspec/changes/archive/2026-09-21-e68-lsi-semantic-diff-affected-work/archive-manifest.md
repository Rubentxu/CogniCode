# Archive Manifest — cycle e68 — Semantic Fact Diff + Affected Work Planner

> Cycle: A-lite (degraded-but-governed) | Milestone: M8 — Scheduler + CI | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e68 |
| Milestone | M8 — Scheduler + CI (foundation: semantic change → affected work mapping) |
| Requirement | U61 follow-on: turn the read-set invalidation into a planning decision |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `781dc50e` (e67 WU3) |
| Spec delta | none (foundation cycle; spec lives in `proposal.md`) |

## Lifecycle note (honest)

**e68 was never instantiated as a formal SDDK cycle in the ledger.** Same pattern as e67:
three work units authored with deterministic checkpoints, single commit `02592ff4` carrying
all three modules together (the modules are not independently compilable — WU2 depends on
WU1 types, and WU3 lives as adversarial/invariant tests inside the WU2 module). Verification
report (already on disk) is the durable record.

This archive commit closes the on-disk artifact gap (proposal + verification-report were
already committed; archive-manifest was missing). The closure pattern is the same as e67:
on-disk record is the verification report + archive-manifest + this commit; no formal SDDK
ledger row.

## Commits

| Commit | Kind | Summary |
|--------|------|---------|
| `02592ff4` | `feat(e68)` | WU1+WU2+WU3 in a single commit (1698 insertions) |

## Delivered

### WU1 — Semantic Fact Delta (`crates/cognicode-core/src/domain/evidence_kernel/semantic_diff.rs`)

- `FactSemanticKey = (subject, predicate, object)` — the diff identity.
- `FactId` and `SnapshotId` deliberately ABSENT from semantic equality (renumbering facts
  across snapshots MUST NOT produce a spurious diff).
- `SemanticFactDelta { removed: Vec<FactSemanticKey>, added: Vec<FactSemanticKey> }`.
- "Modify = removed + added": `A --calls--> B` becoming `A --calls--> C` decomposes as
  `removed(A,calls,B) + added(A,calls,C)`. No `Changed` variant.
- Additions + truncated/incomplete dependency information yield `Unknown` / conservative
  fallback — NEVER `Unaffected`.
- 9/9 unit tests: empty-delta-on-fact-id-only-change, ordering invariance, duplicates
  collapse, distinct-predicate-namespace isolation, same-snapshot rejection, etc.

### WU2 — Affected Work Planner (`crates/cognicode-core/src/application/change_tracking/planner.rs`)

- Maps `SemanticFactDelta` → `AffectedWorkPlan { decisions: Vec<SchedulingDecision> }`.
- `SchedulingDecision { work_id: WorkId, reason: SchedulingReason, ... }`.
- `SchedulingReason`: `Affected { changed_facts }` / `Unaffected` / `Unknown { why }`.
- Reuses e66 `ReadSet` + `InvalidationQuery` port (no new authority path).
- 16/16 tests covering the UAT (positive: removed/changed read fact → affected;
  unrelated change → unaffected; addition without removals → unknown; truncated read set →
  unknown) + adversarial + invariants (determinism under permutation, empty delta → all
  unaffected, addition + unrelated change does NOT collapse to unaffected).

### WU3 — adversarial + invariant tests (inside WU2 module)

- 6-case adversarial matrix per the e67 precedent: cross-snapshot identity stability,
  empty inputs, ambiguous canonical source fail-closed, etc.
- Property-based invariants over the planner.

## Architectural decisions (per proposal)

1. Diff granularity is `Fact` semantics, NOT `GroundingRef` and NOT `EvidenceDescriptor`.
   `GroundingRef` only links a projection to a Fact; `EvidenceDescriptor` belongs to verification.
2. `FactId` is snapshot-scoped; MUST NOT participate in semantic equality.
3. Modify = removed + added (no Changed variant).
4. Additions + truncated info → Unknown (NEVER Unaffected).
5. Logical work vs execution instance is explicit:
   `WorkId` opaque logical identity + `ExecutionDependency { work, execution, read_set }`.
   Never `ExecutionId(734) must rerun`. Always `WorkId("e2e-suite:auth") must rerun`.

## Non-goals (deferred to e69+)

- No job execution.
- No CI orchestration (GitHub Actions, Jenkins).
- No `EvidenceBundle` producer.
- No `PolicyGate`.
- No `why_scheduled` CLI.
- No general query-dependency model (`Query(QueryDependency)`,
  `Predicate(RelationKind)`, `EntitySet(...)`).
- No persisted `WorkPlan` — `FactDelta` and `AffectedWorkPlan` are DERIVED operational
  values, not canonical Facts. The kernel holds truth about the software; the planner
  derives decisions.
- No event bus, no packs.

## Acceptance

| UAT analog | Outcome |
|------------|---------|
| removed read fact yields affected | GREEN |
| read fact semantic change yields affected | GREEN |
| unrelated change yields unaffected | GREEN |
| addition without removals yields unknown | GREEN |
| truncated read set yields unknown | GREEN |
| addition + unrelated change does NOT collapse to unaffected | GREEN |
| planner is deterministic under input permutation | GREEN |
| empty delta yields all unaffected | GREEN |

## Static gates (all clean)

- `cargo fmt --check -p cognicode-core` → clean.
- `cargo clippy -p cognicode-core --all-targets --features evidence-kernel -- -D warnings`
  → clean on touched paths.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — release route blocked; closure via direct push.
- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode.

## Related cycles

- **e66** (predecessor): ReadSet + InvalidationQuery — the foundation this planner reuses.
- **e67** (peer): production grounding activation — provides the first real Fact producer
  whose deltas e68 can plan against.
- **e69+** (successor): EvidenceBundle + PolicyGate (U52/U57 follow-on).
