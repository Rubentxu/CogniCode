# Verification Report — cycle e55 — M6 contract hardening

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e55 — M6 contract hardening |
| Path | A-lite |
| Base HEAD | `b3831584` (post-e54) |
| Type | M6-local API hardening (no consumers yet) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Module tests

```
cargo test -p cognicode-core --lib domain::findings
```

Result: **46 passed; 0 failed** (namespaced 5 + detector IR 18 +
finding 13 + projection 10).

### V-2 — P0 is enforced

`candidate_detector_never_blocks_even_when_promoted_later` proves that a
finding produced by a `Candidate` detector cannot block **even with**
`EvidenceClass::A`, `RiskLevel::Critical`, evidence and a causal chain —
because `Finding::can_block` checks `authority_at_execution`.

### V-3 — Capability/cost separation

`capability_tiers_are_ordered_but_capabilities_are_a_set` proves
`LlmReasoning` does not imply `SymbolicFeasibility`, and
`unsupported_construct_fails_loud` proves a missing capability fails
admission.

### V-4 — Build / purity / format

- `cargo check -p cognicode-core` → Finished, 0 errors.
- `grep` for `sqlx|tokio|I/O` in `domain/findings/` → none.
- `cargo fmt -p cognicode-core --check` → clean.

### V-5 — No stale references

`grep -rn "AnalysisLevel\|DetectorRef\b"` → no matches (removed types are
fully gone).

## Follow-ups (e56)

- Kernel id extraction + `EvidenceRef = EvidenceId`.
- Structured `CausalStep`.
