# e69 — Verification Report

## Status

```text
e69 implementation   CLOSED
e69 verification     PASS
e69 SDDK lifecycle   not formally instantiated
```

e69 was implemented as a bounded A-lite slice without a formal SDDK
cycle (proposal → spec → tasks → verify → archive). This document
records the honest lifecycle state without fabricating artifacts that
were never produced.

## Scope delivered

Three work units, all GREEN in a single commit. Modules are not
independently compilable (WU2 and WU3 depend on WU1 types), so they
are committed together.

| WU | Module |
|----|--------|
| WU1 | `crates/cognicode-core/src/application/evidence_bundle/mod.rs` |
| WU2 | `crates/cognicode-core/src/application/evidence_bundle/producer.rs` |
| WU3 | `crates/cognicode-core/src/application/policy_gate/mod.rs` |

## Verification evidence

### WU1 — EvidenceBundle + BundleEntry

7/7 tests in `application::evidence_bundle::tests`:

- empty_bundle_is_well_formed
- four_kinds_coexist_without_collapsing
- ready_for_gate_requires_no_failed_missing_unknown
- ready_for_gate_false_when_any_non_evidence_present
- ordering_is_stable_across_input_reordering
- bundle_ids_are_unique_within_a_process
- slot_lookup_is_consistent_across_variants

### WU2 — EvidenceProducer trait + aggregation

7/7 tests in `application::evidence_bundle::producer::tests`:

- ok_outcome_yields_evidence_entry
- err_outcome_yields_failed_entry
- unreachable_yields_missing_entry
- garbled_yields_unknown_entry
- aggregation_across_n_producers_yields_n_or_fewer_entries
- aggregation_is_deterministic_under_producer_reordering
- one_producer_failure_does_not_abort_others

### WU3 — PolicyGate adversarial + invariants

14/14 tests in `application::policy_gate::tests`:

- empty_bundle_is_insufficient_evidence
- required_slot_satisfied_with_evidence_passes
- required_slot_missing_is_insufficient_evidence_not_pass
- required_slot_producer_missing_is_insufficient_evidence_not_pass
- required_slot_producer_unknown_is_insufficient_evidence_not_pass
- required_slot_producer_failed_is_block
- optional_slot_failed_does_not_block_only_warns
- required_slot_block_beats_optional_slot_warn
- below_grade_required_slot_is_insufficient_evidence
- satisfies_when_min_grade_is_met
- refutes_minimum_is_satisfied_only_by_refutes
- absence_of_required_evidence_never_yields_pass
- decision_is_deterministic_under_spec_reordering
- decision_carries_a_reason_for_every_rule

### Total: 28/28 PASS

## Regression (all green)

| Suite | Result |
|-------|--------|
| e69 WU1+WU2+WU3 (lib tests) | 28/28 |
| e68 WU1+WU2+WU3 | 25/25 |
| e67 WU1+WU2+WU3 | 20/20 |
| e66 readset | 2/2 |
| `just lsi-equivalence` | 7/7 (same scores; multi-lang-types still QUARANTINED) |
| `findings_ast_e2e` | 8/8 |
| `findings_graph_e2e` | 4/4 |
| `findings_dataflow_e2e` | 7/7 |
| `findings_canonical_grounding_e2e` | 10/10 |
| `intelligence_event_log_e2e` | 4/4 |

## Static gates (clean for e69 files)

- `rg 'Handle::current\(\)\.block_on' <file>` → no match.
- `cargo fmt --check -p cognicode-core` → clean.
- `cargo clippy -p cognicode-core --all-targets --features evidence-kernel -- -D warnings` →
  no warnings on e69 files.

## Architecture invariants preserved

- **Facts remain canonical**: `EvidenceBundle` is a derived
  operational aggregation. Nothing in e69 writes to the Evidence
  Kernel.
- **Evidence carries proof, never authority by itself**: the
  `PolicyGate` decides **based on** evidence entries, it does not
  mint authority.
- **Derived operational decisions are not Facts**: `PolicyDecision`
  is derived. It is not stored as a canonical Fact anywhere.
- **AI/plugins propose; they do not mint authority**: producers
  (`cargo test`, `just`, `cogh`, `analysis`) translate raw output
  into `BundleEntry` values. The gate is the only decision
  authority.
- **`Unknown/incomplete never becomes "safe"`**: enforced by tests:
  - `required_slot_missing_is_insufficient_evidence_not_pass`
  - `required_slot_producer_unknown_is_insufficient_evidence_not_pass`
  - `absence_of_required_evidence_never_yields_pass`
  - `below_grade_required_slot_is_insufficient_evidence`

### Hard distinctions

- `ProducerFailed` ≠ `ProducerMissing` ≠ `ProducerUnknown` ≠
  `Evidence`. Each variant is preserved through aggregation; none
  collapses into another. The bundle's `count_failed` /
  `count_missing` / `count_unknown` / `count_evidence` accessors
  are explicit diagnostics for the gate and for the
  e70 `why_decided` renderer.
- `PolicyOutcome::Pass | Warn | Block | InsufficientEvidence` are
  disjoint. The gate's outcome derivation order encodes the
  load-bearing rule: `Block > InsufficientEvidence > Warn > Pass`.

### Determinism

- Bundle entries sorted by `(source, slot_id)` on construction.
- `reasons` sorted by `(rule, slot)` before returning.
- Producer reordering does not change the final bundle.
- Spec reordering does not change the final decision (when
  outcomes are equal).

## Deviations vs. the proposal

None.

## Lifecycle honesty

e69 did not instantiate the formal SDDK lifecycle. The proposal,
verification, and test evidence above were captured locally under
`openspec/changes/e69-lsi-evidence-bundle-policy-gate/`. Future
cycles should not reference e69 as having gone through
`archive.complete` or any release-receipt chain — those artifacts
were never produced.

## Handoff to e70

e70 may now consume:

- `application::evidence_bundle::{EvidenceBundle, BundleEntry,
  ProducerSource, ProducerSlot, EvidenceBundleId, next_bundle_id}`
- `application::evidence_bundle::producer::{EvidenceProducer,
  Outcome, ProducerOutput, StaticProducer, aggregate,
  aggregate_fresh}`
- `application::policy_gate::{PolicyOutcome, PolicySpec, GateRule,
  ReasonVerdict, PolicyReason, PolicyDecision, evaluate}`

e70 may introduce:
- Local work execution orchestration (no GitHub Actions, no
  Jenkins).
- Local adapters for `EvidenceProducer` that shell out to cargo /
  just / cogh.
- Structured `why_scheduled` and `why_decided` explanations.
