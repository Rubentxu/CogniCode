# e66 Proposal — M7.5 Read-Sets Foundation (Bounded Slice)

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets` |
| Phase | specify |
| Path | a-lite |
| Base SHA | `7df55d49f5d11fbf24a3c618768a24260a827c93` |
| Author | jcode-orchestrator |
| Date | 2026-09-16 |

## Goal

Establish the foundational `ReadSet` data structure and recorder in the
cognicode-core domain, with deterministic deduplication, truncation marker,
and basic invalidation semantics — enough to satisfy UAT-U61.

## Scope (this cycle — bounded slice)

**In scope:**

1. `ReadSet` data structure in `crates/cognicode-core/src/domain/readset.rs`
   - ordered list of `FactId` references
   - deduplicated on insert
   - truncation marker if bounded
2. `ReadSetRecorder` port trait (in `crates/cognicode-core/src/domain/ports/`)
   - `record(&mut self, fact_id: FactId) -> Result<(), ReadSetError>`
   - `finalize(self) -> ReadSet`
   - `is_truncated(&self) -> bool`
3. `InvalidationQuery` port trait
   - `is_stale(execution_read_set: &ReadSet, changed_facts: &[FactId]) -> bool`
   - Returns `true` only if any changed fact is in the execution's read set
4. Unit tests covering:
   - Deduplication
   - Truncation marker
   - Invalid: E read A,B; C changes → not stale (UAT-U61)

**Out of scope (deferred to later cycles):**

- Full reactive runtime (M8+)
- Configurable truncation policies
- Cache integration
- Other UAT milestones (U60, U62-U67)

## Requirements (specify phase)

### REQ-RDS-001 — Read dependency recording

Executions configured for read tracing MUST persist ordered/deduplicated
references to canonical knowledge consumed, with an explicit truncation
marker if bounded.

**Scenario 1.1 (UAT-U61 — the only one)**: Changed unread fact does not invalidate

- GIVEN execution E recorded read-set containing facts A and B only
- WHEN unrelated fact C changes (recorded as `changed_facts = [C]`)
- THEN `is_stale(&read_set, &[C])` returns `false`

### REQ-RDS-002 — Truncation is observable

When `ReadSetRecorder` reaches its bound, subsequent records MUST set
the truncation marker (visible via `is_truncated() == true`).

**Scenario 2.1**: Bound=3, record A,B,C,D
- THEN `finalize()` returns a ReadSet with `is_truncated() == true`

### REQ-RDS-003 — Insertion order preserved

`ReadSet::iter()` returns fact IDs in insertion order, with duplicates
collapsed but their first occurrence's position kept.

**Scenario 3.1**: Record A, B, A, C → iter yields A, B, C

## Work-unit decomposition

Single work unit for this bounded slice:

| WU | Description | Acceptance |
|----|-------------|------------|
| WU1 | ReadSet domain type + recorder + invalidation port + 3 tests | `cargo test --package cognicode-core --lib readset` 3/3 PASS |

Single work unit, single commit, single verify.

## Risks

1. **Integration with existing behavior runtime** (e64/e65) — must not regress
   - Mitigation: ReadSet is a NEW type, doesn't touch existing ExecutionContext or BehaviorAuthority
2. **FactId type** — does this exist yet?
   - Will verify in design phase. If not, define a minimal `FactId` (probably `pub struct FactId(String)` newtype)

## Decision

PROCEED to design with single WU.

See `design.md` (next phase) for implementation strategy.
