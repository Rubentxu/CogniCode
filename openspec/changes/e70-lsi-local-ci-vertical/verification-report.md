# e70 — Verification Report

## Status

```text
e70 implementation   CLOSED
e70 verification     PASS
e70 SDDK lifecycle   not formally instantiated
```

e70 was implemented as a bounded A-lite slice without a formal SDDK
cycle (proposal → spec → tasks → verify → archive). This document
records the honest lifecycle state without fabricating artifacts
that were never produced.

## Scope delivered

Single commit covering all three work units. The modules are tightly
coupled (WU2 and WU3 are types in the same file as WU1) and would
not compile in isolation, so they share one commit.

| WU | Scope |
|----|-------|
| WU1 | `WorkExecutor` trait + `InMemoryWorkExecutor` impl |
| WU2 | `why_scheduled` + `why_decided` (separate explanations) |
| WU3 | End-to-end adversarial + invariants |

Module: `crates/cognicode-core/src/application/local_ci/mod.rs`

## Verification evidence

### 15/15 tests

WU1 (orchestration):
- empty_dependencies_yield_empty_report
- unaffected_work_is_not_executed
- affected_work_is_executed_and_decided
- unknown_work_is_executed_and_may_yield_insufficient
- producer_failure_is_preserved_through_the_bundle_and_blocks_the_gate
- run_id_is_allocated_and_increases_per_call

WU2 (explanations):
- why_scheduled_reasons_are_preserved
- why_decided_reasons_are_preserved
- per_work_order_is_stable_under_dependency_reordering

WU3 (adversarial + invariants):
- product_demo_test_walks_a_real_vertical
- empty_delta_yields_empty_report
- all_unaffected_yields_no_executed_work
- mixed_affected_and_unknown_yield_decisions_per_work
- unknown_planner_disposition_with_missing_evidence_yields_insufficient
- vertical_is_deterministic_across_repeated_runs

## Regression (all green)

| Suite | Result |
|-------|--------|
| e70 WU1+WU2+WU3 (lib tests) | 15/15 |
| e69 WU1+WU2+WU3 | 28/28 |
| e68 WU1+WU2+WU3 | 25/25 |
| e67 WU1+WU2+WU3 | 20/20 |
| e66 readset | 2/2 |
| **Combined e67+e68+e69+e70 lib tests** | **88/88** |
| `just lsi-equivalence` | 7/7 |
| `findings_ast_e2e` | 8/8 |
| `findings_graph_e2e` | 4/4 |
| `findings_dataflow_e2e` | 7/7 |
| `findings_canonical_grounding_e2e` | 10/10 |
| `intelligence_event_log_e2e` | 4/4 |

## Static gates (clean for e70 files)

- `rg 'Handle::current\(\)\.block_on' <file>` → no match.
- `cargo fmt --check -p cognicode-core` → clean.
- `cargo clippy -p cognicode-core --all-targets --features evidence-kernel -- -D warnings` →
  no warnings on e70 files. Pre-existing warnings on other modules
  (`admission.rs`) are unchanged.

## Architecture invariants preserved

- **Facts are canonical**: nothing in e70 writes to the Evidence
  Kernel. The vertical consumes existing canonical types only.
- **Evidence carries proof, never authority by itself**: producers
  still propose; the gate still decides. The e70 orchestration does
  not change this contract.
- **Derived operational decisions are not Facts**:
  `LocalVerticalReport` is a derived in-process summary.
- **AI/plugins propose; they do not mint authority**: the
  `WorkExecutor` trait can be implemented by anything; the first
  impl is `InMemoryWorkExecutor` for tests. A real async shell-out
  executor (e70.x) would only translate external tool output into
  `ProducerOutput` — it would not change authority.
- **`Unknown/incomplete never becomes "safe"`**:
  - All-Unaffected work is NEVER executed (`all_unaffected_yields_no_executed_work`).
  - Planner Unknown IS executed (conservative) but a Missing slot
    on a required rule still yields `InsufficientEvidence`
    (`unknown_planner_disposition_with_missing_evidence_yields_insufficient`).
  - Producer failure on a required slot blocks the gate
    (`producer_failure_is_preserved_through_the_bundle_and_blocks_the_gate`).

### Two explanations, not one

- `why_scheduled` (per-work, multiple reasons possible): explains
  *selection* — pulled verbatim from `SchedulingReason` (e68).
  Kinds: `ReadFactRemoved`, `ReadFactChanged`, `UnrelatedChange`,
  `ConservativeAddition`, `ReadSetTruncated`, `UnknownDependency`.
- `why_decided` (per-work, one decision): explains *authority* —
  pulled verbatim from `PolicyDecision` (e69). Verdict kinds:
  `Satisfied`, `BelowGrade`, `Failed`, `Missing`, `Unknown`,
  `Absent`.
- These are deliberately different types. A single work item has
  exactly one `why_decided` entry but may carry one or more
  `why_scheduled` reasons (e.g. removed + changed + added → three
  reasons, one disposition).

### Determinism

- Per-work report sorted by `work_id`.
- Reasons preserved as-is from the upstream planners.
- Repeated runs produce the same per_work field
  (`vertical_is_deterministic_across_repeated_runs`).
- Dependency ordering does not affect the per-work order
  (`per_work_order_is_stable_under_dependency_reordering`).

## Deviations vs. the proposal

None. The `WorkExecutor` trait was the right choice; the first
impl is in-memory.

## Lifecycle honesty

e70 did not instantiate the formal SDDK lifecycle. The proposal,
verification, and test evidence above were captured locally under
`openspec/changes/e70-lsi-local-ci-vertical/`. Future cycles should
not reference e70 as having gone through `archive.complete` or any
release-receipt chain — those artifacts were never produced.

## Debt ledger

```text
DEBT-X (Query-level dependency tracking)
  severity: medium
  priority: revisit when conservative fallback exceeds X% in real
            workloads
  owner milestone: TBD (e70.x or later)
  reason for deferral: e68/e69/e70 are sufficient with Fact-level
                       read-sets; the work executor captures
                       per-slot outcomes that approximate query
                       behavior in practice
  trigger to revisit:
    - the addition-driven conservative fallback rate in real
      workloads exceeds the threshold agreed post-strategic-review
    - a real workload shows excessive reruns attributable to
      conservative-UNKNOWN placement
```

This is the first tracked debt entry under the new "controlled
debt" discipline. No TODOs without a revisit trigger.

## Mandatory stop after e70

Per the authorized envelope: **STOP after e70 for strategic
architecture review**. The next milestone (M9: Fork / Trial /
Promote) MUST NOT be opened automatically. The deliberate review
will examine:

- false-negative risk
- fallback frequency
- read-set usefulness
- FactDelta quality
- EvidenceBundle ergonomics
- performance
- API complexity
- duplicated abstractions
- technical debt

Decisions on M9 are deferred to that review.
