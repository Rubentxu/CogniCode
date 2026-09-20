# Proposal — roadmap-auto-discovery

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/roadmap-auto-discovery` |
| Path | a-min |
| Phase | specify |
| Date | 2026-09-20 |

## Product objective

Establish a **governed, auto-advancing regime** under SDDK that allows
the orchestrator to execute CogniCode's roadmap via bounded cycles
without per-phase human gate, while preserving all hard safety gates
(`git.push`, `git.tag`, `git.release`, `no-v1.0.0`, no `allow`/waivers,
no AI attribution trailers).

## User intent (verbatim)

> "crea una iniciativa para completar todo el roadmap sobre que estamos
> trabajando con workflows sddk en modo auto"

## Auto-advance rule

Within the scope of an authorized initiative (defined here), the
orchestrator advances **without inter-phase human gate** through:

1. `cycle start` → opens a bounded SDDK cycle for one roadmap item.
2. Within the cycle: `explore → design → tasks → apply → verify →
   archive` (the canonical A-lite path) or the path declared per cycle.
3. Each cycle produces **exactly two commits**: one impl + one archive.
4. Push is **deferred** to a batch boundary. Multiple cycles accumulate
   verified local commits; the user authorizes the batch push with
   one `git push` (still respecting `git.push=human_gate`).

The user authorizes at **three boundaries**, not per phase:

- **Boundary 1**: initiative definition (this proposal). Specifies
  which roadmap items are in scope, which are out, which require
  separate user decision.
- **Boundary 2**: each cycle's *intent* and *acceptance* (start gate).
  Equivalent to one human `sddk cycle start --name <X>` per cycle.
- **Boundary 3**: each batch push. After N cycles accumulate N×2
  commits verified locally, the user authorizes `git push origin main`
  (FF only, no force, no rebase).

This satisfies the user prefs:

- "bounded cycles (explore→design→tasks→apply→verify→archive)" ✅
- "one impl commit + one archive commit" ✅
- "Conventional Commits, NO AI attribution trailers" ✅
- "NO v1.0.0 tag (frozen)" ✅
- "OpenSpec artifacts per change" ✅
- "git.push=human_gate, git.tag=human_gate, git.release=human_gate"
  respected at Boundary 3 ✅

## In-scope roadmap items (candidates)

The exploration report (`openspec/changes/roadmap-auto-discovery/explore-report.md`)
catalogued active OpenSpec changes. Classified:

### Tier A — Concrete candidate cycles (have proposal.md + tasks.md)

| Change | Status | Cycle path |
|--------|--------|------------|
| `h44-mcp-continuation-chain-real-server` | specs + tasks present | b-direct (H4.4 already merged; archive the change) |
| `m5-program-analysis-core` | full cycle artifacts + release-receipt | reference (already closed) |

### Tier B — Administratively closed (D3-DEFER pattern, already in main)

These are *not* candidates for new cycles; their work is in main. Their
OpenSpec changes need **archive synchronization only**:

`e65`, `e66`, `e67`, `e68`, `e69`, `e70`, `e71`, `e72`, `e73`,
`e74`, `e75`, `e76`, `e77`, `e79`, `lsi-final-umbrella-reconciliation`.

Action: ensure each has `archive-manifest.md` reflecting the D3-DEFER
closure. Bounded cycle: `archive-sync-<name>` for each.

### Tier C — Live, not yet started (have folder, no work yet)

`e80a`, `e80b`, `e81`, `e82-1`, `e82-2`, `e82`, `e83`, `e84`,
`e86-2`, `e86-3`, `e86-4`, `cp0`, `cp1`,
`cognicode-living-software-intelligence` (umbrella).

Action: **defer to separate user decision**. Each requires its own
product objective before auto-advance can be authorised.

### Tier D — Reference / out of scope

`m5-1-ast-lifting`, `m5-1b-*`, `m5-mcp-wiring` — reference cycles
already archived; not candidates.

## Concrete candidate cycles (this initiative)

The following bounded cycles will be opened, in this order, with
auto-advance between phases but explicit user authorization at the
start of each:

### Cycle 1: `archive-sync-lsi-m6-m7` (path: b-direct)

- **Goal**: ensure every LSI M6/M7 closure change has a complete
  `archive-manifest.md` reflecting its D3-DEFER closure + receipt.
- **Scope**: `openspec/changes/e65, e66, e67, e68, e69, e70, e71, e72,
  e73, e74, e75, e76, e77, e79, lsi-final-umbrella-reconciliation`.
- **Out of scope**: code changes, decisions on DEBT-SDDK-002/003.
- **Acceptance**: every change folder has an `archive-manifest.md`
  whose contents match the D3-DEFER pattern (commit SHA, scope closure,
  debt ledger reference if applicable).
- **Required tests**: no code change → only file existence check via
  `find openspec/changes/ -name 'archive-manifest.md' | wc -l`.
- **Deliverables**: N file updates (one per change), one archive
  commit, one receipt.

### Cycle 2: `clippy-hygiene-181-bounded` (path: b-direct)

- **Goal**: reduce 181 cosmetic clippy warnings to 0 (or document why
  some remain) without using waivers, `allow` attributes, or threshold
  changes.
- **Scope**: `crates/cognicode-cli` (153 warnings) + `cognicode-core`
  (19 warnings) + others (~9).
- **Stop conditions**: any waiver introduced → immediate rollback.
- **Acceptance**: `cargo clippy --workspace --all-targets` exits 0
  with 0 warnings; no `#[allow(clippy::*)]` added in this cycle;
  no `clippy.toml`/`.clippy.toml` thresholds changed.
- **Required tests**: `cargo test --workspace` EXIT=0;
  `python3 scripts/check_known_failures.py --tmpdir-root /tmp` EXIT=0;
  H4 regression suite (continuation_e2e 5/5).
- **Sub-batches**:
  - 2a: 84 mechanical-only (unused_imports, collapsible_if,
    unused_variables, needless_borrow, doc_overindented, misc)
  - 2b: 19 in cognicode-core (likely test code; safe)
  - 2c: 72 dead_code → REQUIRES USER CLASSIFICATION per warning
    (delete vs. allow vs. preserve as surface). STOP until user
    classifies.

### Cycle 3: `release-v0.97.2-prep` (path: a-min, NO tag cut)

- **Goal**: prepare the conditions for the next ceremonial release
  (v0.97.2 or whatever lockstep rule allows), without cutting the tag.
- **Scope**: CHANGELOG.md entry draft; release-receipt template;
  scorecard pre-tag evidence; tag cut is human-gated and out of scope.
- **Acceptance**: changelog entry + scorecard receipts + release
  receipt template ready for user `sddk release apply` invocation.
- **Stop conditions**: tag cut attempted inside this cycle → STOP.

## Out of scope (initiative-wide)

- Any tag cut, including v0.97.2.
- DEBT-SDDK-002 / DEBT-SDDK-003 resolution.
- Anything requiring v1.0.0 tag (frozen).
- E78, TRACK B (deferred by user prefs).
- New features not in existing OpenSpec changes.

## Acceptance criteria (initiative-level)

1. All three cycles above reach `archived` state with verified
   receipts.
2. Working tree at HEAD matches SDDK-archived state.
3. `cargo fmt --check`, `cargo check`, `cargo clippy` (workspace,
   all-targets) EXIT=0.
4. `cargo test --workspace` and known_failures baseline EXIT=0.
5. No waivers, no `allow` attributes, no threshold changes, no
   AI attribution trailers introduced.
6. Conventional Commits only.
7. Each bounded cycle produced exactly two commits (impl + archive).

## Stop conditions (initiative-wide)

- Any violation of the no-waiver/no-allow policy → STOP and rollback.
- Any push attempted without user `git push` authorization → STOP.
- Any tag cut attempted → STOP and require human gate.
- Any change that violates "bounded cycles, one impl + one archive"
  → STOP and reconsider.
- Any conflict between this initiative's auto-advance and user
  preferences → STOP and ask.

## Risk and unknowns

- **Risk**: cycle 2c (dead_code classification) requires per-warning
  user input. Mitigation: STOP at the cycle boundary if user input
  not provided; do not invent classification.
- **Risk**: changes between HEAD and tag v0.97.1 need to be enumerated
  for the changelog. Mitigation: `git log v0.97.1..HEAD --oneline` will
  produce the list; we do not invent content.
- **Risk**: the LSI M6/M7 closure changes may have dependencies on
  each other that the archive-sync must respect. Mitigation: archive
  sync is non-destructive (only writes/updates `archive-manifest.md`,
  does not delete anything).

## Successor phases

- **Tasks phase**: produce `openspec/changes/roadmap-auto-discovery/tasks.md`
  with one task per cycle above.
- **Apply phase**: open and run cycles 1, 2 (a+b only), 3 in order.
- **Verify phase**: SDDK verify receipts + human-readable status.
- **Archive phase**: `sddk sddk-archive` for this initiative cycle.

## Open questions

- Does the user want cycle 2c (dead_code) included now, or deferred?
- Does the user want cycle 3 (v0.97.2 prep) included, or deferred to
  after cycles 1+2?
- For cycle 1, does the user want one archive-manifest.md per change,
  or one umbrella manifest listing all?
