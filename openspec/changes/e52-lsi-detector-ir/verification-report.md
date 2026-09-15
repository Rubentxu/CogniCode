# Verification Report — cycle e52 — Detector IR (M6.1)

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e52 — Detector IR (M6.1) |
| Path | A-lite |
| Base HEAD | `2fad1e1a` (post-e51) |
| Type | additive pure domain (no production consumers yet) |
| Verify verdict | **PASS** |

## Deliverable

Umbrella task **7.2 Define Detector IR schema/parser/validator.**

- `crates/cognicode-core/src/domain/findings/mod.rs` (new)
- `crates/cognicode-core/src/domain/findings/detector_ir.rs` (new)
- `crates/cognicode-core/src/domain/mod.rs` (+1 module registration)

## Verification

### V-1 — Module tests

```
cargo test -p cognicode-core --lib domain::findings
```

Result: **15 passed; 0 failed.** Covers:

| Test | Umbrella requirement exercised |
|------|-------------------------------|
| `analysis_level_rank_is_strictly_ascending`, `analysis_level_display` | escalation ladder order |
| `subject_pattern_accepts_namespaced`, `subject_pattern_rejects_malformed` | namespaced grammar |
| `valid_graph_flow_detector_admits` | happy path |
| `unsupported_construct_fails_loud` | REQ "unsupported construct fails loud" |
| `flow_without_match_rejected`, `exclude_before_flow_rejected`, `no_produce_rejected`, `produce_must_be_last`, `duplicate_produce_rejected`, `empty_identity_rejected` | REQ "validated before admission" |
| `candidate_detector_cannot_block` | REQ "AI detector authority" (Candidate has no GATE authority) |
| `detector_ir_round_trip`, `malformed_subject_fails_deserialization` | serde contract |

### V-2 — Default build compiles

```
cargo check -p cognicode-core
```

Result: `Finished` with the 2 pre-existing warnings only, 0 errors.

### V-3 — Domain purity

```
grep -nE "sqlx|tokio|std::fs|std::process|reqwest" domain/findings/*.rs
```

Result: only doc-comment mentions; **no I/O imports**. Domain purity rule satisfied.

### V-4 — Format + lint

- `cargo fmt -p cognicode-core --check` → clean.
- `cargo clippy -p cognicode-core` → no diagnostics referencing `findings`.

## Regression / no-regression

- No existing type or module modified except the one-line `pub mod
  findings;` registration.
- No consumer yet; the surface is additive.

## Follow-ups (next M6 cycles)

- Findings / evidence-class (A–D) model + gate predicate (umbrella
  `finding-evidence-model`).
- Detector backends (AST/graph/dataflow) that consume this IR.
- QualityIssue compatibility projection + Axiom import tooling.

## Conclusion

The Detector IR domain contract is in place, validated, tested and
pure. Later M6 work compiles against it. PASS.
