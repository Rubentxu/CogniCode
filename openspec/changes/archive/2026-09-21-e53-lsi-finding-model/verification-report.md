# Verification Report — cycle e53 — Finding & evidence-class model (M6.2)

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e53 — Finding & evidence-class model |
| Path | A-lite |
| Base HEAD | `811b9389` (post-e52) |
| Type | additive pure domain |
| Verify verdict | **PASS** |

## Verification

### V-1 — Module tests

```
cargo test -p cognicode-core --lib domain::findings
```

Result: **26 passed; 0 failed** (15 detector IR + 11 finding).

Umbrella scenario coverage:

| Umbrella scenario | Test |
|-------------------|------|
| Finding evidence chain / "Blocking finding is explainable" | `blocking_finding_is_explainable`, `explainable_requires_evidence_and_causal_chain` |
| Evidence grades / "Hypothesis cannot satisfy strong gate" | `hypothesis_cannot_satisfy_strong_gate`, `evidence_class_ordering_and_satisfies` |
| lifecycle correctness | `status_active_only_for_open_and_accepted`, `inactive_or_unvalidated_findings_cannot_block` |
| serde contract | `finding_round_trip` |

### V-2 — Build + purity + format

- `cargo check -p cognicode-core` → Finished, 0 errors.
- Domain purity: no `sqlx`/`tokio`/I/O imports in `domain/findings/`.
- `cargo fmt -p cognicode-core --check` → clean.

## Follow-ups

- Detector backends (AST/graph/dataflow) that emit `Finding`s.
- `EvidenceRef` ↔ kernel `EvidenceId` reconciliation in the wiring cycle.
- QualityIssue compatibility projection (task 7.6).

## Conclusion

The Finding/evidence-class model is in place, tested against both umbrella
`finding-evidence-model` scenarios, and pure. PASS.
