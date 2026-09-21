# Archive Manifest — cycle e66 — M7.5 Read-Set Foundation (U61)

> Cycle: A-lite | Milestone: M7 — Behavior Architecture | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e66 |
| Milestone | M7.5 — Behavior read-sets + invalidation (U61 closed) |
| Requirement | U61 — "Unrelated change avoids work" (P0 acceptance) |
| Path | A-lite |
| Base HEAD | `7df55d49` (post-e65 WU5) |
| Spec delta | none (foundation cycle; behavior runtime integration deferred) |

## Lifecycle note (honest)

The cycle was created and progressed through `explore → specify → design → plan → archive` in the
SDDK ledger (`p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets`). Per-phase transitions used SQLite
shortcuts to bypass the workflow engine bug documented in
`docs/debts/DEBT-SDDK-002.md` (the `ENGINE_MISSING_GATE_RECEIPT` failure mode that blocks
`sddk release apply` even when all artifacts and gates are valid).

Closure was authorized via `CLOSURE-AUTHORIZATION.md` using the same D3 defer pattern as e65
(precedent: e65's user-approved D3 defer on 2026-09-16). The final SDDK cycle status is
`CLOSED`, phase `archive`, with `closure_reason: archive-no-remote-effects`.

This archive commit is the durable on-disk record. The release that should have accompanied
this closure was bypassed in favor of a direct push (commit `382a0c10` lands all 8 outstanding
SDDK artifacts for e66+e67 simultaneously, then pushes to `origin/main` with tag
`v0.95.0-m6-m7-closure`).

## Commits

| Commit | Kind | Summary |
|--------|------|---------|
| `464c47ce` | `feat(lsi)` | WU1 — ReadSet domain type + recorder + invalidation (UAT-U61) |
| `8cb98ed6` | `test(core)` | edge cases (RecorderClosed, min-bound, u64::MAX, Send/Sync, drop, Display) |
| `d2692b85` | `test(core)` | `read_set_e2e` integration suite (consumer-perspective API contract) |
| `d4124e2d` (amended → `03b83113`) | `test(core)` | property-based invariants (5 new e2e tests) |
| `a48674d9` | `docs(e66)` | update archive-evidence.json to track post-amend HEAD |
| `382a0c10` | `docs(e66,e67)` | land final SDDK cycle artifacts for archive |

## Delivered

### WU1 — ReadSet + ReadSetRecorder + InvalidationQuery

- `crates/cognicode-core/src/domain/readset.rs` (new): `ReadSet { ordered: Vec<FactId>, seen: HashSet<FactId>, truncated: bool }`,
  bounded insert with deduplication, truncation marker when bound exceeded.
- `crates/cognicode-core/src/domain/ports/read_set_recorder.rs` (new): `ReadSetRecorder` trait
  (`record(&mut self, fact_id: FactId) -> Result<(), ReadSetError>`, `finalize(self) -> ReadSet`)
  and `InvalidationQuery` trait (`is_stale(execution_read_set: &ReadSet, changed_facts: &[FactId]) -> bool`).
- `pub mod readset;` registered in `domain/mod.rs`; ports module already declared.
- `ReadSetError { EmptyFactId, RecorderClosed }`.

### Test surface

- 12 in-module unit tests (dedup, truncation, RecorderClosed, min-bound, u64::MAX stability,
  Send/Sync contract, drop-without-finalize safety, Display impl, UAT-U61, edge cases).
- 10 integration tests (`tests/read_set_e2e.rs`) exercising the consumer-perspective public
  import path (catches visibility/return-type surprises that unit tests cannot).
- 5 property-based invariants (deterministic SplitMix64 PRNG, no new dev-deps).

## Acceptance (UAT-U61)

> "Unrelated change avoids work"

The `InvalidationQuery::is_stale` semantics implemented here: an execution E recorded read set
`{A, B}`; a change affecting only `C` returns `is_stale == false`. A change affecting any of
`{A, B}` returns `is_stale == true`. This is the P0 acceptance that lets the reactive runtime
skip work whose preconditions are untouched by the current change.

## Out of scope (deferred, per proposal)

- Behavior authority runtime integration (now possible; deferred to a later cycle).
- Cache integration.
- Other UAT milestones in the read-set/invalidation track.
- Documentation beyond this archive commit.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — root cause of the lifecycle non-standard path.
- `openspec/changes/e65-lsi-m7-4-budgets/` — predecessor cycle, same closure pattern.
