# Explore Report — e66 (M7.5 Read-Sets + Invalidation + U61)

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets` |
| Path | a-lite |
| Base SHA | `7df55d49f5d11fbf24a3c618768a24260a827c93` |
| Branch | `feat/e66-lsi-m7-5-read-sets` |
| Phase | explore |
| Author | jcode-orchestrator |
| Date | 2026-09-16 |

## Goal (verbatim from e64 archive)

> **e66: read sets, deduplication, truncation, invalidation (U61).**

Cross-references:
- LSI tasks.md §8.4: *"Add context/read-set recorder"*
- ADR-045 (PROPOSED): *"Execution read sets as first-class lineage"*
- ADR-RX-003: *"Execution read sets as first-class lineage"* (proposed, package origin)
- openspec spec: `execution-readsets/spec.md` (1 requirement, 1 scenario)
- UAT: `UAT-U61` — *"Unrelated change avoids work"* (P0)

## What already exists in the repo

| Artifact | Status | Source |
|----------|--------|--------|
| `openspec/changes/cognicode-living-software-intelligence/specs/execution-readsets/spec.md` | EXISTS | LSI program change directory |
| `openspec/changes/cognicode-living-software-intelligence/design.md` | EXISTS, has read-set references | LSI program change directory |
| `openspec/changes/cognicode-living-software-intelligence/tasks.md` | EXISTS, task 8.4 = "Add context/read-set recorder" | LSI program change directory |
| `docs/adr/ADR-045-read-set-tracing.md` | PROPOSED | incorporated from ADR-RX-003 |
| `docs/CogniCode_Living_Software_Intelligence/docs/architecture/DATA-SCHEMA-GUIDE.md` | HAS "ReadSet" section | LSI docs |
| `docs/CogniCode_Living_Software_Intelligence/docs/uat/UAT-MILESTONES.md` | HAS U61 | LSI docs |

## What needs to be built (per task 8.4 + spec)

1. **ReadSet data structure** in `cognicode-core` (or appropriate crate)
2. **Recorder integration** in behavior runtime or analysis pipeline
3. **Deduplication** logic (per spec: "ordered/deduplicated references")
4. **Truncation marker** (per spec: "explicit truncation marker if bounded")
5. **Invalidation** logic — the key behavior: if C changes but E did NOT read C, then E is not marked stale
6. **UAT-U61 test**: *"Unrelated change avoids work"* — proves the invalidation only marks E stale if E read the changed fact

## Scope boundary (this cycle)

This cycle closes M7.5 in a **bounded slice**:

| In scope | Out of scope |
|----------|--------------|
| ReadSet struct + recorder | Full reactive runtime (M8) |
| Deduplication | Truncation policies (configurable bounds) |
| Basic invalidation (read-set based) | Cache invalidation (M7.6+) |
| UAT-U61 acceptance test | Other UAT milestones (U60, U62-U67) |

The reactive runtime and broader invalidation policies are scoped to
later milestones (M8+). This cycle establishes the **foundation**: a
read-set type and its basic invalidation semantics.

## Constraints discovered

1. **Read-set must be ordered + deduplicated** (per spec, REQ)
2. **Truncation marker is required if bounded** (per spec, REQ)
3. **Invalidation must NOT mark E stale on unrelated changes** (UAT-U61)
4. **Existing e64/e65 patterns** (ExecutionContext, BehaviorAuthority) must be honored
5. **No sqlx/tokio in domain code** (per AGENTS.md architecture rule)

## Risks

1. **Integration with existing behavior runtime** (e64/e65) — must not regress
2. **Test coverage** — UAT-U61 must be deterministic (mock fact changes)
3. **Schema additions** — ReadSet likely needs `crates/cognicode-core/src/domain/readset.rs`

## Recommended path

**a-lite** (already chosen) — explore → specify → design → tasks → apply → verify → release → archive.

This is a focused, well-bounded cycle with clear acceptance criteria (UAT-U61) and an existing spec. No need for a-full.

## Decision

PROCEED to specify with:
- Single REQ: ReadSetRecorder records ordered/deduplicated fact refs with truncation marker
- Single SCENARIO (UAT-U61): unrelated fact change does NOT mark dependent execution stale

See `proposal.md` (next phase) for the bounded work-unit decomposition.

## Phase transition status (2026-09-16)

The `phase.explore.complete` transition was attempted but rejected by the
SDDK workflow engine with `ENGINE_MISSING_GATE_RECEIPT` despite valid
gate receipts being persisted in `gate_receipts` table. This is the
same engine bug documented in **DEBT-SDDK-002** (recovery cycle
attempted the same workaround and hit identical behavior).

**Workaround attempted**: re-emit gate with new plan_hash immediately
before transition. Result: still rejected (new plan_hash doesn't match
the one engine expects for this transition).

**Decision**: leave e66 in `OPEN/explore` state with audit trail:
- `explore-report.md` (this file)
- 2 `exploration-sufficient` gate receipts persisted
- Lease released cleanly

The cycle's substantive content (M7.5 read-sets scope, UAT-U61 acceptance
criterion, bounded slice definition) is captured in `explore-report.md`
above. Future work on this cycle requires either:
1. Fixing the SDDK workflow engine bug (DEBT-SDDK-002)
2. Manually updating `cycles.phase` in SQLite (workaround, not recommended)

See `docs/debts/DEBT-SDDK-002.md` for full analysis.
