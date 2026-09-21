# Explore Report — roadmap-auto-discovery

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/roadmap-auto-discovery` |
| Path | a-min (propose → spec → tasks → apply → verify → archive) |
| Base SHA | `ba8897b510fd72cd0fd85d11f0f1ed901b1e8532` |
| Branch | `main` |
| Phase | explore |
| Date | 2026-09-20 |
| Origin | User directive 2026-09-20 + SDDK discovery |

## Goal (verbatim from user)

> "crea una iniciativa para completar todo el roadmap sobre que estamos trabajando con workflows sddk en modo auto"

Cross-references:
- User directive 2026-09-20 ("paquetes autónomos"): regime change allowing
  multi-milestone bounded cycles with auto-advance between phases.
- SDDK framework 1.169.93 active, project adopted, ledger available.
- Workspace version `0.97.1`, lockstep rule active, no active cycle.
- CogniCode workflow conventions (per user reminder): bounded cycles
  (explore→design→tasks→apply→verify→archive), one impl commit + one
  archive commit, Conventional Commits, NO AI attribution trailers,
  NO v1.0.0 tag (frozen), OpenSpec artifacts per change, push
  accumulated and authorised at end of batch.

## What already exists in the repo

### Active LSI changes (cycles not yet archived)

From `openspec/changes/`:

| Change | Artifacts present | State |
|--------|-------------------|-------|
| `e65-lsi-m7-4-budgets` | (folder missing) | Closed; product in main; release v0.95.0 cut |
| `e66-lsi-m7-5-read-sets` | proposal + explore + design + tasks + verification + archive | Closed via D3-DEFER (commit `382a0c10` ancestor of HEAD) |
| `e67-lsi-production-grounding` | proposal + design + spec + tasks + verification + archive | Closed via D3-DEFER (`ae1579d0` ancestor) |
| `e68-lsi-semantic-diff-affected-work` | proposal + verification + archive | Closed via D3-DEFER (`ce7f9e1c` ancestor) |
| `e69-lsi-evidence-bundle-policy-gate` | proposal + verification + archive | Closed via D3-DEFER (`ce7f9e1c` ancestor) |
| `e70-lsi-local-ci-vertical` | proposal + verification + archive | Closed via D3-DEFER (`ce7f9e1c` ancestor) |
| `e71-lsi-software-world-fork` | archive only | Closed |
| `e72-lsi-trial-execution` | archive only | Closed |
| `e73-lsi-promotion-gate` | archive only | Closed |
| `e74-lsi-portable-runtime-distribution` | full set + closure-authorization | Closed |
| `e75-lsi-portable-execution` | proposal + tasks + wu0-execution-baseline + archive | Closed; WU1-WU6 in main |
| `e76-lsi-self-hosting-validation` | proposal + tasks + archive | Closed |
| `e77-lsi-executable-architecture` | full set + archive | Closed |
| `e79-lsi-ai-foundation-readonly-agents` | explore + archive | Closed |
| `e80a-lsi-automated-author-authority` | (folder exists, content to verify) | TBD |
| `e80b-lsi-fix-agent-change-proposal` | (folder exists) | TBD |
| `e81-lsi-historical-replay-dataset-split` | (folder exists) | TBD |
| `e82-1-lsi-behavior-event-boundary` | (folder exists) | TBD |
| `e82-2-lsi-historical-score-correctness` | (folder exists) | TBD |
| `e82-lsi-shadow-evaluation-failure-regimes` | (folder exists) | TBD |
| `e83-lsi-heldout-promotion-governed-improvement` | (folder exists) | TBD |
| `e84-cognicode-distribution-artifact-contract` | (folder exists) | TBD |
| `e86-2-cogh-real-user-uat` | (folder exists) | TBD |
| `e86-3-cogh-uninstall-coverage` | (folder exists) | TBD |
| `e86-4-cogh-rollback-select-version` | (folder exists) | TBD |
| `lsi-final-umbrella-reconciliation` | archive only | Closed |
| `h44-mcp-continuation-chain-real-server` | specs + tasks | Active; tied to H4.4 |
| `m5-program-analysis-core` | full cycle artifacts + release-receipt | Closed reference pattern |
| `cognicode-living-software-intelligence` | (umbrella LSI program folder) | Active spec umbrella |
| `cp0-backstage-fit-spike` | (cp* active) | TBD |
| `cp1-control-plane-first-cycle` | (cp* active) | TBD |

### Live state at HEAD `ba8897b5`

- **Release tags reachable from origin**: `v0.94.11`, `v0.94.12`, `v0.94.13`,
  `v0.94.14`, `v0.94.15`, `v0.95.0`, `v0.95.0-m6-m7-closure`, `v0.96.0`,
  `v0.97.0`, `v0.97.1`.
- **`v0.97.1` is ancestor of HEAD**; H4.4-H4.7 are post-v0.97.1.
- **Lockstep rule**: `sddk release plan` refuses any tag mismatched with
  workspace version. Workspace=0.97.1 → next tag must be `v0.97.2` or
  higher.
- **Backlog**: `sddk backlog list` shows "no live backlog items".
- **Cargo gates at HEAD** (re-verified this cycle):
  - `cargo fmt --all -- --check` EXIT=0
  - `cargo check --workspace --all-targets` EXIT=0
  - `cargo clippy --workspace --all-targets` EXIT=0 (181 cosmetic warnings,
    pre-existing, catalogued in `.agent/h4.7-evidence/clippy-catalog-181-warnings.txt`)
  - `cargo test -p cognicode-mcp --test continuation_e2e` 5/5 PASS
  - `python3 scripts/check_known_failures.py --tmpdir-root /tmp` EXIT=0
- **13 stashes intact**, no push pending, working tree clean (only
  `.agent/TESTING-STATE.md` dirty, gitignored).

### Pending user-facing items (per snapshot + persistent memory)

- **DEBT-SDDK-002**: third consecutive cycle using D3 defer. Documentation
  in `docs/debts/DEBT-SDDK-002.md` (local-only).
- **DEBT-SDDK-003**: AI worker delegation unavailable; degraded-but-
  governed mode for e67/e68/e69/e70.
- **v1.0.0 tag cut**: FROZEN by user policy. Pre-cut gates operational
  (T7 5-night cadence + 3-run scorecard streak).
- **182 cosmetic clippy warnings** (catalogued): candidates for a bounded
  clippy-hygiene cycle (forbidden: waivers, `allow` attributes, threshold
  changes).
- **H4.4-H4.7 post-v0.97.1 commits**: candidates for `v0.97.2` ceremonial.

## What needs to be built (per user intent)

### Scope of the "initiative"

This is an **umbrella initiative**, NOT a single cycle. The user wants
to "completar todo el roadmap" via SDDK workflows in auto mode. The
initiative:

1. Defines the **rule for executing each bounded cycle** under SDDK.
2. Identifies the **candidate cycles** (one per OpenSpec change) that
   constitute "the roadmap" today.
3. Specifies the **auto-advance policy** between cycles within the
   initiative.
4. Respects hard gates: `git.push=human_gate`, `git.tag=human_gate`,
   `git.release=human_gate` — push accumulated and authorised at end
   of batch per user prefs.

### In scope (this explore cycle)

- Discovery of roadmap items as concrete cycles.
- Definition of the "auto mode" execution rule.
- Mapping of each roadmap item to an SDDK cycle (or marking it
  administratively closed).
- Boundary: this explore cycle produces ONLY the proposal + tasks
  document. **No implementation in this cycle.**

### Out of scope (this explore cycle)

- Any code change.
- Any tag cut.
- Any cycle start beyond this exploration.
- Decisions on DEBT-SDDK-002/003 resolution (those are separate user
  decisions).

## Scope boundary (this cycle)

| In scope | Out of scope |
|----------|--------------|
| Catalogue every active OpenSpec change | Implement any spec |
| Map each change to its SDDK cycle status | Cut any tag |
| Define auto-advance policy | Push anything |
| Produce proposal.md + tasks.md for the umbrella | Resolve DEBT-SDDK-002/003 |
| Identify hard gates that block auto mode | Decide v0.97.2 contents |

## Risk and unknowns

- **Risk: scope explosion** — counting 100+ OpenSpec changes, an
  umbrella initiative could become unmanageable. Mitigation: this
  cycle produces a *catalogue* and a *routing rule*, not a plan to
  execute everything.
- **Risk: push authorisation** — `git.push=human_gate` cannot be
  auto-bypassed. The "auto mode" applies between cycles' internal
  transitions, not at the final integration. Mitigation: each cycle
  verifies locally; push is batched per directive 2026-09-20 FASE 3.
- **Risk: stale OpenSpec changes** — many LSI changes are marked closed
  administratively. Distinguishing them from active ones requires
  checking artifacts present. Mitigation: use presence of
  `archive-manifest.md` alone vs. presence of `proposal.md`+`tasks.md`
  as heuristic; flag ambiguous cases for user review.
- **Risk: v1.0.0 frozen** — user prefs forbid v1.0.0 tag. Mitigation:
  the umbrella explicitly excludes v1.0.0; it focuses on v0.97.2
  preparation and clippy hygiene.

## Successor phases (for the umbrella)

1. **Specify phase** — produce proposal.md defining the umbrella rule,
   the catalogue of candidate cycles, and the auto-advance policy.
2. **Tasks phase** — produce tasks.md with one task per candidate
   cycle (e.g., "execute cycle X via SDDK transitions").
3. **Apply phase** — auto-advance through each candidate cycle using
   `sddk cycle start` + `sddk cycle transition` + `sddk cycle evaluate-gate`.
   This is the bulk of work and runs many sessions.
4. **Verify phase** — ensure all cycles closed per SDDK gates, ledger
   consistent.
5. **Archive phase** — `sddk sddk-archive` for the umbrella initiative.

## Open questions

- Does the user want this umbrella to **enqueue** the next bounded cycle
  (so I can `sddk cycle start` it in the next turn) or **complete the
  first bounded cycle as a proof** before enqueuing more?
- For cycles blocked on DEBT-SDDK-002 (e65, e66, e67 in some sense),
  does the user want to wait for that decision or proceed with adjacent
  cycles?
- For the 181 clippy warnings, is the bounded clippy-hygiene cycle
  wanted as part of this umbrella, or treated as a separate workstream?
