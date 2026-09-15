# Verification Report — cycle e63 — Intelligence Event Log foundation (M7.1, U50)

> Cycle: A-lite | Milestone: M7 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e63 — Intelligence Event Log foundation |
| Path | A-lite |
| Base HEAD | `95f5761e` (M6 closed) |
| Commit | `de57775d` |
| Verify verdict | **PASS** |
| U50 | **CLOSED** |

## Verification

### V-1 — Suite counts

| Suite | Result |
|-------|--------|
| `domain::findings` | 114 passed |
| `domain::naming` | 6 passed |
| `domain::intelligence_log` + `infrastructure` + `application` | 31 passed |
| `findings_canonical_grounding_e2e` (feature) | 10 passed |
| **`intelligence_event_log_e2e`** (feature, acceptance) | **4 passed** |
| `cargo check --workspace --all-targets` | 0 errors |
| `cargo check -p cognicode-core --features evidence-kernel --all-targets` | 0 errors |
| `cargo fmt --all --check` | clean |
| `scripts/check_known_failures.py` | exit 0 — 41 entries, unchanged |

`domain::findings` moved 119 → 114 only because the 5 namespaced-name tests
moved with the grammar to `domain::naming` (which now holds 6). No test was
lost; the total went up.

### V-2 — The U50 exit gate

`tests/intelligence_event_log_e2e.rs`, over the **real** slice (facts committed
to the real kernel, the real M6 executor and canonical bridge):

| Assertion | Evidence |
|-----------|----------|
| `causal_chain(X) == [SourceDelta, FactBatchCommitted, AnalysisCompleted, FindingProduced]` | kinds compared in order, **and** each `caused_by` edge checked, so the order cannot be a reversed coincidence |
| the chain ends at a finding that is real | the same finding is loaded through `KernelEvidenceReadModel` and passes `can_block` |
| replay produces the same ordered immutable events | two `replay` calls compare equal; an event's `by_id` value is unchanged by later appends; incremental replay returns the correct suffix |
| a large batch is one bounded event referencing a digest | 500 facts → 4 events total; the batch event is `Artifact`, and its digest equals `ContentDigest::of(committed bytes)` |
| the slice is one correlated operation across actors | `by_correlation` returns all 4; the batch is attributed to the kernel and the finding to the detector |

### V-3 — Store semantics (the specification as tests)

| Property | Test |
|----------|------|
| ids are store-allocated, one global sequence | `append_allocates_sequential_ids_from_one_global_sequence` |
| append is all-or-nothing | `an_unknown_cause_is_refused_and_nothing_is_written`, `append_is_all_or_nothing_across_a_whole_batch` |
| a cause must exist | `an_unknown_cause_is_refused_and_nothing_is_written` |
| tenants cannot reach each other's history | `a_cause_from_another_workspace_is_refused` (append refused **and** the foreign event is invisible to a chain) |
| nothing is ever mutated | `an_event_is_immutable_once_appended` |
| replay is deterministic and incremental | `replay_is_deterministic_and_incremental` |
| correlation groups one operation in order | `correlation_groups_one_operation_in_append_order` |
| payloads stay bounded / digest-referenced | `inline_payloads_are_bounded`, `a_bounded_payload_and_an_artifact_reference_both_round_trip` |
| the recorder never invents a cause | `record_next_without_a_previous_event_fails_loud` |

### V-4 — Design decisions that the tests made load-bearing

- The choice of **one global id sequence** was not cosmetic. With per-workspace
  numbering, "this cause is another tenant's event" is undetectable, and the
  first version of that test asserted the wrong error precisely because of it.
- The first version of the append logic only accepted causes already *stored*,
  which made a natural batch (`SourceDelta → FactBatch` in one append)
  impossible. Intra-batch causes were added deliberately, and the atomicity test
  still holds.
- The fixture originally committed `FactId(7)` twice (once from the generated
  range, once as a hand-written duplicate). It was removed rather than tolerated:
  a fixture that commits the same fact twice is a lie about the batch.

## Deliberately not executed

- Full-workspace test run beyond the known-failure baseline: the change adds two
  ungated modules and one gated integration test, and the baseline covers the
  rest of the workspace by name.
- Durable-adapter tests: no durable adapter exists yet.

## Unknown impact

- **`replay` is proved only in memory.** The oracle defines the semantics, but
  nothing yet exercises a restart, a partial write or a re-open. That is the
  first thing a durable adapter must be tested against, and it is the honest
  boundary of this cut.
- The behavior/policy event kinds are reserved but unreachable: nothing emits
  them, and nothing may yet, since no behavior runtime exists. Their presence is
  vocabulary, not capability.

## Conclusion

The system can now remember what happened, causally and in order, at a bounded
cost per operation. ADR-043 has the concrete evidence the spike was supposed to
produce: a real slice, a real chain, a real replay and a real content-addressed
batch. **PASS — U50 closed; M7.1 complete.**
