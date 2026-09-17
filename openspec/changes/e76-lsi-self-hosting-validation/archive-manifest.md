# Archive Manifest — cycle e76 — LSI Self-Hosting Validation

> Cycle: A-lite (degraded-but-governed) | Milestone: M14 — Distribution | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e76 |
| Milestone | M14 — Distribution (self-hosting validation: CogniCode analyzes itself) |
| Requirement | prove the M14 portable execution + M9 lineage + M8 verification stack by having the system analyze its own source |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `a89be4d3` (e75 clippy cleanup) |
| Spec delta | none (foundation cycle; spec lives in `proposal.md`) |

## Lifecycle note (honest)

**e76 was never instantiated as a formal SDDK cycle in the ledger.** Same pattern as e67-e70+e74+e75:
four work units authored with deterministic checkpoints (WU1-WU4); each WU landed in a separate
commit. Closure route: D3 defer + on-disk archive (this manifest).

This archive commit closes the on-disk artifact gap (proposal + tasks were already committed;
archive-manifest was missing).

## Commits

| Commit | WU | Summary |
|--------|----|---------|
| `097ae14a` | WU1 | `feat(lsi)` — deterministic self-model baseline |
| `c563604d` | WU2 | `feat(lsi)` — sealed prediction vs observation scoring |
| `00fd8108` | WU3 | `feat(lsi)` — controlled mutation corpus with pre-declared outcomes |
| `d86b082f` | WU4 | `feat(lsi)` — platform equivalence + historical replay bootstrap |
| `5c1a99a0` | closure | `feat(lsi)` — e76 closure gate — structural proof of end-to-end composition |
| `27fc9179` | fix | `chore(e75+e76)` — resolve clippy -D warnings on the e75+e76 seam |
| `2ad79ccd` | fix | `fix(cognicode-core)` — gate self_hosting behind evidence-kernel feature |
| `9ba7e4b9` | acceptance | `feat(lsi)` — e76 end-to-end acceptance tests for WU1, WU2, WU4 |
| `0810e53f` | acceptance | `feat(lsi)` — e76 WU3 mutation acceptance + baseline edge cases |
| `97cb6694` | acceptance | `test(lsi)` — platform equivalence edge cases (BOM, mixed CR/LF, partial divergence) |
| `6529d841` | acceptance | `test(lsi)` — closure replay shape — HistoricalReplay carries what WU2 needs |
| `1f9518ed` | fix | `fix(lsi)` — gate the e76 acceptance/edge-case test modules behind evidence-kernel |

## Delivered

### WU1 — Deterministic self-model baseline

- The system ingests its own source (cognicode-core + cognicode-runtime + cognicode-cli + cognicode-mcp).
- Produces a deterministic self-model: stable across rebuilds on the same source, byte-identical
  for the same input.
- Baseline SHA captured for comparison against mutations.

### WU2 — Sealed prediction vs observation scoring

- Before execution: the system predicts what the produced model SHOULD look like.
- After execution: the system observes the produced model.
- Comparison: prediction vs observation, score = `matches / total`.
- "Sealed": the prediction is committed before observation; the scoring function cannot see
  the observation until it runs.

### WU3 — Controlled mutation corpus with pre-declared outcomes

- 7+ deliberate mutations applied to the self-model:
  - add a function (expect: model gains 1 entity + N facts).
  - remove a function (expect: model loses 1 entity + N facts).
  - rename a function (expect: identity continuity rule kicks in).
  - reorder lines (expect: no change).
  - mixed CR/LF / BOM (expect: canonical normalization).
  - partial divergence (expect: well-defined partial score).
- Each mutation has a pre-declared outcome (the expected scoring band) BEFORE the mutation
  is applied — this is the WU3 acceptance.

### WU4 — Platform equivalence + historical replay bootstrap

- Platform equivalence: the same self-model run on Linux and macOS produces the same
  byte-identical model (modulo canonical path normalization).
- Historical replay: a previous baseline can be replayed; the replay MUST yield the same
  score against the same prediction set.

## Acceptance (per WU5/WU6 + edge-case sweep)

- `cargo test -p cognicode-core --features evidence-kernel self_hosting` PASS (66 tests).
- `cargo test -p cognicode-core --features evidence-kernel portable_execution` PASS (89 tests).
- Total e75+e76: 155 tests, 271 pass / 0 fail on the e75+e76 scoped regression.
- 0 clippy findings on touched paths.
- Release build OK; doc build OK.
- 41 pre-existing failures UNCHANGED from origin/main baseline.

## Honest limitations (declared in scope)

- macOS / Windows UAT waived (Linux-only runner locally).
- e76 closure is **structural + shape-verified**, not runtime-closure-by-replay:
  `compare_replay` is `todo!()` by design (a follow-up cycle will land the runtime closure).
- DefaultNormaliser is content-only; path/env/case delegated to `portable_execution::canonicalize`.

## Out of scope (deferred)

- Runtime closure-by-replay (a separate cycle; not implied by the structural closure).
- macOS / Windows acceptance UAT.
- Performance characterization under load.

## Related cycles

- **e74** (predecessor): portable runtime & distribution.
- **e75** (predecessor): portable execution contract + NativeProcessBackend + PodmanBackend.
- **M9** (BLOCKED): Fork / Trial / Promote — the e76 self-model is the substrate that
  M9's trial semantics will operate on.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — release route blocked; closure via direct push.
- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode.

## STRATEGIC STOP

After e76's closure, a STRATEGIC STOP was declared (per the e76 archive commit message
and the testing-state). M9 (Fork / Trial / Promote) requires explicit user authorization
before being opened.
