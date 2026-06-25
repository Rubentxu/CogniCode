# ADR-001: Parked Crates — Activation Criterion

- **Status**: ACCEPTED
- **Date**: 2026-06-25
- **Deciders**: Orchestrator + user (during `quality-stack-pg-canonical` cycle)

## Context

The CogniCode workspace has three crates currently commented out of `Cargo.toml` (`crates/cognicode-axiom`, `crates/cognicode-quality`, `crates/cognicode-rule-test-harness`). They were parked during the Graph Intelligence v2 cleanup (commit `e4b232c feat: move graph federation from explorer to core behind multimodal feature`, 2026-06-09) when the workspace was being slimmed to a Postgres-canonical stack.

Each parked crate retains a working `Cargo.toml`, source tree, and CI configuration; the only thing missing for reactivation is un-commenting the workspace `members` entry.

Without an explicit criterion for re-activation, the parked state risks:
- Accidental re-inclusion by a contributor who doesn't realize the dependencies aren't ready.
- Endless drift as the parked code falls behind the workspace's PG-canonical baseline.
- Confusion during onboarding (new contributors don't know whether the crates are deprecated or pending).

This ADR captures the criterion per crate so the decision is explicit and reviewable.

---

## Decision

Each parked crate has its own activation criterion. The criterion is intentionally different per crate because each was parked for a different reason (see §Alternatives below).

### `cognicode-axiom`

**Parked on**: 2026-06-09 (`e4b232c`)
**Reason for parking**: Axiom's rule definitions assumed a SQLite-backed layer that was removed when Postgres became canonical. Re-wiring the 600+ rules to read from PG was out of scope for the v2 cleanup.

**Re-activation trigger**: Either (a) a dedicated quality-agent design cycle that produces an ADL for the rule layer + storage adapter, OR (b) explicit decision to retire the rule system and archive the crate.

**Archive-vs-delete criterion**: If trigger (b) is chosen, the crate is **archived** (sources kept under `docs/parked-crates/cognicode-axiom/`) — never deleted, because the historical rule catalog may be referenced by retrospective audits.

**Owner**: unassigned — re-activation requires a fresh design cycle, not a single owner.

---

### `cognicode-quality`

**Parked on**: 2026-06-09 (commit `e4b232c`)
**Reason for parking**: `cognicode-quality` depended on `cognicode-axiom` for its `Issue` type. Once axiom was parked, quality had no upstream. The `incremental.rs` module was preserved as a stub and now carries `#[deprecated]` annotations per the 2026-06-25 architecture review (candidate 3, Path B: cleanup futuro).

**Re-activation trigger**: Depends on **axiom's** re-activation path. If axiom lands trigger (a) (fresh ADL), quality can be revived alongside it. If axiom lands trigger (b) (archive), quality is also archived — there is no value in reviving one without the other.

**Archive-vs-delete criterion**: Same as axiom — **archived**, never deleted. The `incremental.rs` analysis-state machine documents a useful baseline-difference design that may inform future work even if the code is never rebuilt.

**Owner**: unassigned.

---

### `cognicode-rule-test-harness`

**Parked on**: 2026-06-09 (commit `e4b232c`)
**Reason for parking**: This was the test harness for axiom's rule engine. Once axiom was parked, the harness had no production rules to test.

**Re-activation trigger**: Depends on **axiom's** re-activation. Specifically, the harness can be revived only after axiom's rule ADL is decided, and only if the resulting rule set needs property-based or fuzz testing beyond what `cargo test` provides.

**Archive-vs-delete criterion**: **Delete** is acceptable. The harness has no useful state independent of axiom; if axiom is archived, this crate can be removed entirely without losing anything. (If axiom is reactivated, this crate is rebuilt from scratch.)

**Owner**: unassigned.

---

## Alternatives considered

1. **Reactivate all three together as a "rules subsystem" ADR**: rejected. The crates have independent value (axiom = rules, quality = analyzer, harness = tests). One decision doesn't fit all.
2. **Delete all three immediately**: rejected. The 2026-06-25 architecture review found at least 600+ rules of historical value in axiom and a useful baseline-diff design in quality. Both deserve preservation in archived form.
3. **Keep parked indefinitely without an ADR**: rejected. This is the status quo before this ADR. It caused confusion (the 2026-06-25 SQLite error in engram #2900 partially traces back to a contributor reading the parked-crate history and assuming the crates were active).

## Consequences

- **Positive**: New contributors can read this ADR and immediately understand the parked state without grep'ing `Cargo.toml`. The criterion is reviewable; if axiom's reactivation ever becomes desirable, this ADR is the starting point.
- **Positive**: The archive-vs-delete distinction prevents accidental deletion of historical code.
- **Negative**: Three parked crates continue to consume workspace-level mental overhead even though they're not in the build graph.
- **Mitigation**: The `sddk-archive` skill should always verify this ADR still matches reality when a new cycle touches the parked crates (the next cycle to do so is `quality-stack-pg-canonical` itself, which lands the change that motivated this ADR).

## References

- Commit that parked the crates: `e4b232c feat: move graph federation from explorer to core behind multimodal feature` (2026-06-09)
- Workspace `Cargo.toml` lines 13-15 (commented-out members with parking notes)
- 2026-06-25 architecture review §5 candidate 3 (Path B for `incremental.rs`)
- Engram jurisprudence: #2900 (root-cause audit), #2902 (deepening decisions)
- v1 archive report: `sddk/quality-stack-pg-canonical/archive-report.md` (REQ-009 NOT DONE)
- v2 spec: `sddk/quality-stack-pg-canonical-v2/spec.md` (REQ-V2-006)
- v2 verify report: `sddk/quality-stack-pg-canonical-v2/verify-report.md` (Issue #1, closed by this ADR)

## Status

ACCEPTED on 2026-06-25 as part of the `quality-stack-pg-canonical` cycle closure. Linked from `sddk/quality-stack-pg-canonical-v2/verify-report.md` Issue #1.
