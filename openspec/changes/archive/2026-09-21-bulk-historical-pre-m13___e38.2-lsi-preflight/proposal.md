# Proposal: e38.2 LSI Preflight — Four Gates Before M4

## Intent

LSI M0–M3 archived (HEAD 6977d816). M4 adds multiple fact producers committing into shared snapshots; four gates must close:

1. **CP-4**: `InMemoryFactStore::commit` silently double-assigns FactIds when a second batch reuses an id-space in the same `(WorkspaceId, SnapshotId)` (collision = existing fact id ≤ batch max; spaces start at 1).
2. **PERF**: M2 gate flagged sub-µs bench `search` (+124% vs 79ns over 2 runs; ms-scale ±4%). Only default-path LSI delta: `CallGraphProjection::unresolved_edges`.
3. **ENGINE-DET**: legacy nondeterminism in `build_project_graph` (`analysis_service.rs:349`): unsorted walk, lowercase name-keyed first-file-wins collapse. Quarantined (multi-lang-types 0.5969/0.0714).
4. **CLIPPY**: `just lint` red on 5 pre-existing lints since E30.1.

## Scope

### In Scope
- **CP-4**: collision guard + RED-first contract test; new `KernelError` variant (`SnapshotMismatch`-style caller violation; `Store(String)` stays I/O-reserved).
- **PERF**: 2× fresh `compare --fail-above 10`; if >10% reproduces: stash-revert `unresolved_edges` (never committed), rebuild, re-run, restore, re-run. Persists → NOISE-ATTRIBUTED (baseline untouched); flips → feature-gate.
- **ENGINE-DET**: sorted walk + deterministic duplicate-name rule; align to shared `resolve_callee_identity` ONLY if python-hello/rust-hello goldens stay byte-identical, else determinism-only. Report multi-lang-types (un-quarantine only if ≥0.99, not expected); double-run proof.
- **CLIPPY**: fix 5 lints; `just lint` exit 0 is the criterion.

### Out of Scope
S2 harness, CP-5, DUP-2/3/6, formal UAT, extractor use-import gap, M0 coverage; commits deferred to orchestrator.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- None. No capability spec covers `FactStore::commit`; kernel semantics live in umbrella `cognicode-living-software-intelligence` (`evidence-kernel` delta) — fold the guard requirement there at archive.

## Approach

Four independent work units, one stacked slice (auto-chain, no commits): CP-4 RED-first; ENGINE-DET golden-gated; PERF evidence-only; CLIPPY mechanical.

## Affected Areas

- `evidence_kernel/ports.rs` + `in_memory.rs` — new `KernelError` variant; guard + contract test.
- `application/services/analysis_service.rs` — deterministic walk + tie-break.
- `call_graph_projection.rs` — field gated only if A/B implicates it.
- `cognicode-macros` ×2; `domain/aggregates/generic_graph.rs` — 5 clippy fixes.

## Risks

- Resolver alignment changes goldens (Medium) → byte-check gate; determinism-only fallback.
- A/B edit leaks into cycle (Low) → never committed; goldens re-verify restore.
- ENGINE-DET exceeds minimal fix (Medium) → STOP: record blocker, ship 1/2/4.

## Rollback Plan

Each unit is an independent revert boundary; the A/B revert is stash-style, never committed. `just lsi-fixtures check` re-proves the default path after each unit.

## Dependencies

HEAD 6977d816, clean tree. Quiet bench window (sub-µs benches noise-prone).

## Success Criteria

- [ ] CP-4: RED→GREEN; duplicate commit rejected
- [ ] PERF: A/B verdict recorded; baseline untouched if noise
- [ ] ENGINE-DET: goldens 42/42 or justified `--accept`; scored fixtures 1.0/1.0; multi-lang run-stable
- [ ] `just lint` exit 0 (first since E30.1); fmt clean; clippy delta zero
- [ ] Identity gates 1.0000; core/runtime checks exit 0
