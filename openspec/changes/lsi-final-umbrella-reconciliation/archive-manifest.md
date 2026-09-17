# Archive Manifest — LSI Final Umbrella Reconciliation

> Cycle: lsi-final-umbrella-reconciliation | Phase: archive | Date: 2026-09-17
> Administrative reconciliation only. NO product implementation.

## Required final report

```text
LSI roadmap implementation          COMPLETE

M11                                CLOSED
M13                                CLOSED

M10 executable architecture         CLOSED
e78 Pack Ecosystem                  DEFERRED / 1 genuine consumer (checkpoint not met)

DEBT-SDDK-004                       RESOLVED
DEBT-SDDK-006                       OPEN / NON-BLOCKING

architecture drift                  ZERO
baseline                            STABLE

ADR-050                             EXECUTED — e79/e80a/e80b implement "authored without authority"
ADR-051                             EXECUTED — e81-e83 implement and verify held-out separation
ADR-048                             PROPOSED — e78 deferred, clause not satisfied

umbrella apply                      COMPLETE (implemented roadmap)
umbrella verify                     DONE (scoped closure; evidence below)
umbrella archive                    DONE (scoped closure; e78 recorded DEFERRED)

CP0 Backstage                       NOT STARTED
```

## WU0 — baseline verification

```text
HEAD == origin/main == a2f62810      confirmed
tracked tree                         clean apart from the deliberate local docs/adr edit
sddk ledger verify                   GREEN (123 events, hash unchanged)
```

`docs/adr/**` edits are local-only per AGENTS.md and are NOT committed.

## WU0 — umbrella lifecycle engine check

```text
sddk status --cycle cognicode-living-software-intelligence  → STORAGE_NOT_FOUND
sddk ledger events | grep living-software-intelligence      → 0 matches
```

The umbrella was never instantiated as an SDDK cycle record, so there is no
engine-level lifecycle to contradict. Its lifecycle bookkeeping lives in
`openspec/changes/cognicode-living-software-intelligence/state.yaml`, which is
the operational authority (as the maintainer confirmed: `tasks.md` checkboxes are
historical and immutable; `state.yaml` is authoritative).

`sddk lint` reports 93 pre-existing framework/doc errors (missing
`docs/generated/*`, `permissions.yaml`, `schemas/`, and dangling references in
old `sddk/e29-*` change dirs). None is caused by this cycle and none concerns the
umbrella's lifecycle. Characterized, not bypassed.

## WU3 — e78 consumer-count checkpoint re-evaluated

Real consumers of the **executable architecture capability**
(`ArchitectureRegistry` / `ArchitectureAdmissionService` / evaluator) at current
HEAD, excluding the module itself, internal unit tests, docs, manifests and
future plans:

| Candidate | Verdict |
|-----------|---------|
| `crates/cognicode-core/tests/architecture_self_host_e2e.rs` | the originally-counted verification surface (1) |
| `crates/cognicode-core/tests/architecture_e77_1_wu3_canonical_grounding_e2e.rs` | same capability's acceptance surface, not a distinct product surface |
| `crates/cognicode-core/tests/architecture_drift_e2e.rs` | same, acceptance surface |
| `application/ai/semantic_miner.rs` + `domain::ai` | a candidate **producer** for admission (uses `domain::architecture` types), not a consumer of the evaluator |
| MCP tool `check_architecture` | **NOT a consumer**: it is a legacy `CycleDetector` over the call graph and hard-codes `violations: vec![]`; it never calls the e77 module |

```text
genuine distinct operational/product consumers: 0  (the checkpoint's counted 1 remains a verification surface)
=> e78 = DEFERRED
```

Preserved triggers:

```text
(a) detector-pack re-architecture needs the manifest surface
(b) another extension pack (e.g. security packs) needs capability advertisement
(c) a third-party/plugin reaches the admission surface
```

e78 was NOT implemented to close the umbrella. Note for CP0: the Explorer/MCP
surface for executable architecture does not exist yet — the name collision above
is worth knowing before building on it.

## WU4 — DEBT-SDDK-006

```text
status      OPEN
severity    housekeeping
blocks CP0  NO
trigger     next bounded maintenance / feature-gating cleanup
```

Two `interproc_summary` feature-gating failures in the maintained baseline: known,
reproducible, classified, not caused by e79-e83, and unrelated to any
authority/governance proof (the governed path's evidence is e83's own suite).
`known_failures.yaml` was NOT edited and the feature gate was NOT fixed here.

## WU5 — ADR status audit (LOCAL ONLY)

Convention: `EXECUTED` = applied and verified in code; `ACCEPTED` = ratified but
not yet executed.

| ADR | Before | Clause | Evidence | Correct status |
|-----|--------|--------|----------|----------------|
| 043 Intelligence Event Log | ACCEPTED | UAT + consumers/rollback | event log applied; all `BehaviorRuntime` events flow through the domain port; `intelligence_event_log_e2e` + e82.1 shape test | EXECUTED |
| 044 Behavior Classes | ACCEPTED | UAT + consumers/rollback | sealed permit + governed runtime; e64/e65 suites; `behavior_authority_e2e`/`behavior_budget_e2e` | EXECUTED |
| 045 Read Sets | ACCEPTED | UAT + consumers/rollback | `ReadSet` + recorder + invalidation consumed by AI frames and e81/e82 lineage | EXECUTED |
| 046 Software World Fork/Trial/Promote | PROPOSED | UAT + consumers/rollback | `SoftwareWorld`/`fork`/`TrialEvidence`/`evaluate_promotion`/`PromotionPermit`; e81-e83 + 42 authority tests | EXECUTED |
| 047 Evidence Bundle | PROPOSED | UAT + consumers/rollback | bundle + e69 `PolicyGate` + `DefaultTrialExecutor`; policy suites + e83 trial leg | EXECUTED |
| 048 Packs | PROPOSED | UAT + consumers/rollback | e78 deferred; consumer checkpoint unmet | **PROPOSED (unchanged)** |
| 049 Executable Architecture | PROPOSED | UAT + consumers/rollback | e77 implemented; e77.1 corrected grounding; e82.1 closed the last drift; self-host 0 drift, no longer ignored | EXECUTED |
| 050 Code Authorship Without Authority | PROPOSED | UAT + consumers/rollback | e79 read-only AI, e80a authority boundary, e80b FixAgent proposal-only; e83 requires held-out pass | EXECUTED |
| 051 Historical Held-out Promotion | ACCEPTED | UAT + consumers/rollback | e81 separation, e82 measurement, e82.2 truth table, e83 block + governed E2E | EXECUTED |

Each promoted ADR received an **Execution evidence** section documenting the
implementing cycles AND the affected consumers/rollback path, so the promotion
satisfies the ADR's own clause rather than bypassing it. ADR-048 was deliberately
left PROPOSED.

All ADR edits are local (`docs/adr/**` is gitignored and must never be pushed).

## WU6 — umbrella lifecycle semantics

There is no SDDK engine state for the umbrella, so closure is recorded in
`state.yaml`:

```text
apply    COMPLETE  — the implemented roadmap (M0-M9, M11, M13, M10 executable architecture)
verify   DONE      — scoped closure; evidence: e79-e83 archives + this reconciliation
archive  DONE      — scoped closure; e78 explicitly DEFERRED WITH TRIGGER
```

**Governance note (stated plainly).** This is a **scoped** closure: one milestone
(M10 Packs / e78) is closed as *deferred by design* rather than satisfied, because
its consumer-count checkpoint is still unmet. Nothing is claimed as done that is
not. If the maintainers require an umbrella to close only with every milestone
satisfied, this closure should be read as "implemented roadmap complete; umbrella
lifecycle remains open solely for deferred e78" instead. e78 was NOT implemented,
and implementing it to close a status field would be the wrong reason to build it.

## WU7 — final verification

```text
sddk ledger verify                  GREEN (123 events)
architecture self-host              0 drift (runs as normal verification)
known-failure checker               exact (41 entries)
cargo test --lib                    baseline-exact
evidence-kernel suite               only the two DEBT-SDDK-006 failures
cargo build --workspace             GREEN
```

No focused e79-e83 suite was re-run: this is reconciliation, not another
acceptance campaign, and their archived evidence stands.

## STOP

```text
CP0 Backstage Fit Spike             NOT STARTED
```

The next decision is whether Backstage is an adequate host for the headless
architecture plus the Explorer — not another brain component.
