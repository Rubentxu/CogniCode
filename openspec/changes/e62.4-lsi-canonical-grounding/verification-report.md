# Verification Report — cycle e62.4 — canonical grounding, coherence verification (U42, part 2b)

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e62.4 — canonical grounding, coherence verification |
| Path | A-lite |
| Base HEAD | `b6e97c88` (post-e62.3) |
| Commits | `5f9bb6cf` (WU2+WU3), `f86cee55` (WU4+WU5+WU6) |
| Verify verdict | **PASS** |
| U42 | **CLOSED** |

## Verification

### V-1 — Suite counts

| Suite | Result |
|-------|--------|
| `domain::findings` (ungated) | 119 passed |
| `domain::findings` (feature `evidence-kernel`) | 119 passed |
| `application::findings` (feature `evidence-kernel`) | 21 passed |
| `evidence_kernel` (feature) | 88 passed |
| `findings_ast_e2e` / `findings_graph_e2e` | 8 / 4 passed |
| `findings_dataflow_e2e` / `findings_axiom_import_e2e` | 7 / 3 passed |
| `findings_canonical_grounding_e2e` (feature, **acceptance**) | 10 passed |

`domain::findings` went 106 → 119 across the cycle.

### V-2 — The three adversarial acceptances (U42's closing condition)

`tests/findings_canonical_grounding_e2e.rs`, run against the **real** kernel
stores and the plain sync verifier:

| Case | Construction | Result |
|------|--------------|--------|
| **A** | finding produced in snapshot A; read model loaded from snapshot B where a real evidence atom and a real fact carry the *same* numeric ids (`model.len() == 1` proves B is complete) | `Err(ScopeMismatch)` — raised before any id is resolved, and `can_block == false` |
| **B** | correct evidence id, evidence grades fact 7, causal step claims fact 8 (both facts exist and resolve) | `Err(FactMismatch { step: 0, step_fact: 8, evidence_fact: 7 })`, `can_block == false` |
| **C** | correct id, correct fact, `grade = Refutes` | `Err(RefutingEvidence { grade: Refutes })`, `can_block == false` |

### V-3 — The three positives

AST, Graph and Dataflow each run `prepare → CanonicalEvidenceWriter::persist →
finalize`, are loaded back through `KernelEvidenceReadModel::load`, and reach
`can_block == true` through `FindingGate`. Graph is checked atom by atom (4
atoms, 4 causal steps, 4 distinct evidence ids); Dataflow likewise (3 and 3).

### V-4 — The bridge does not fabricate

| Property | Test |
|----------|------|
| Fact missing in scope | reported `Ungrounded { MissingFact }`, nothing written, route still explained, cannot gate |
| Entity hint contradicts the canonical subject | reported `Ungrounded { EntityFactMismatch }`, cannot gate |
| Store failure | `Err(KernelError::Store)` — never silently "ungrounded" |
| `prepare` alone | writes nothing to the kernel |

V-4's second row was load-bearing rather than decorative: the first version of
the graph/dataflow acceptance fixtures hinted an entity per node, which the
bridge correctly refused. The fixtures were fixed, not the rule.

### V-5 — Verifier refusals (unit level)

`UngroundedCausalStep`, `RefutingEvidence`, `FactMismatch`, `SubjectMismatch`,
`DanglingFact`, `SnapshotMismatch` each have a focused test, in addition to the
pre-existing scope and referential tests.

### V-6 — Build / format / regressions

- `cargo check --workspace --all-targets` → 0 errors.
- `cargo check -p cognicode-core --features evidence-kernel --all-targets` → 0 errors.
- `cargo fmt --all --check` → clean.
- `scripts/check_known_failures.py` → exit 0, baseline 41 entries unchanged.

## Deliberately not executed

- Full-workspace test run beyond the known-failure baseline: the change is
  contained in `domain::findings`, `application::findings` and one gated
  acceptance test, and the baseline covers the rest of the workspace by name.
- Real-producer grounding: no producer emits canonical facts yet, so there is
  nothing to test. See "Unknown impact".

## Unknown impact

- **Real runs cannot gate yet.** M5, the graph/DAG lift and the AST lift do not
  emit canonical facts, so in production every evidence atom is `Ungrounded` and
  every finding is explainable but non-blocking. This is the intended
  `correct incomplete` state and it is *more* conservative than before this
  cycle: previously an ungrounded route could block, now it cannot. The first
  producer to emit facts will make real findings gateable without touching the
  verifier.
- The in-memory findings double attests the snapshot it was hydrated for, so it
  cannot exercise `SnapshotMismatch`; the kernel read model is where that check
  has teeth, and the acceptance test uses it.

## Conclusion

`EvidenceId` no longer means "a number that exists in a store". It means
`(workspace, snapshot) + canonical evidence → canonical fact`, with provenance
retained in the kernel; a finding may block only when its evidence, its facts
and its causal chain all agree with that meaning. **PASS — U42 closed, M6 done.**
