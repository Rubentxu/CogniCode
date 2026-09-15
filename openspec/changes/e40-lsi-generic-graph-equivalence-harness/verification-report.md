# Verification Report — e40 — GenericGraph equivalence harness

> Change: `e40-lsi-generic-graph-equivalence-harness` | Phase: verify | Date: 2026-09-15

## Scope verification

| REQ | Title | Verdict | Evidence |
|-----|-------|---------|----------|
| REQ-EQGG-01 | Harness covers GenericGraphProjection | COMPLIANT | `crates/cognicode-core/tests/equivalence_harness/generic_graph.rs` exists, 298 lines. |
| REQ-EQGG-02 | Rebuild equivalence | COMPLIANT | `generic_graph::rebuild_byte_identical` PASS. |
| REQ-EQGG-03 | Dangling-free contract | COMPLIANT | `generic_graph::dangling_edge_skipped` PASS (3 edges, missing_callee dropped). |
| REQ-EQGG-04 | Self-loop-free | N/A (covered by `GraphEdge` unit tests, out-of-scope per spec) | — |
| REQ-EQGG-05 | Kind-multiset equivalence | COMPLIANT | `generic_graph::kind_multiset_matches` PASS. |
| REQ-EQGG-06 | Empty ⇒ empty projection | COMPLIANT | `generic_graph::empty_facts_yields_empty_projection` PASS. |
| REQ-EQGG-07 | Pinned digest gate | COMPLIANT | `generic_graph::pinned_digest_matches` PASS (digest = `sha256:6d22cf734f1094a07df63aea4698e512ae5046f4fa732a322ea43b31678a1cc5`). |
| REQ-EQGG-08 | Feature gating mirrors e37 | COMPLIANT | `#![cfg(all(feature = "evidence-kernel", feature = "multimodal"))]` at file head. |
| REQ-EQGG-09 | Wired into parent harness | COMPLIANT | `crates/cognicode-core/tests/equivalence_harness.rs` includes the new module. |

**Verdict: COMPLIANT (8/8 in-scope REQs, REQ-EQGG-04 explicitly N/A).**

## Verification commands run

```
$ cargo test -p cognicode-core --test equivalence_harness \
    --features evidence-kernel,multimodal
...
test generic_graph::helper_kinds_are_canonical ... ok
test generic_graph::empty_facts_yields_empty_projection ... ok
test generic_graph::kind_multiset_matches ... ok
test generic_graph::dangling_edge_skipped ... ok
test generic_graph::pinned_digest_matches ... ok
test generic_graph::rebuild_byte_identical ... ok
[...8 existing e36 scenarios...]
test result: ok. 13 passed; 0 failed; 0 ignored
```

## Diff summary

| Stat | Value |
|------|-------|
| Files | 2 (`equivalence_harness.rs`, `equivalence_harness/generic_graph.rs`) |
| LOC | +368 / -0 |
| Commits | 1 (`7791c2e2`) |
| Head SHA | `7791c2e2` |
| Origin SHA | `7791c2e2` (verified via `git ls-remote origin main`) |

## Verdict

**PASS** — cycle is ready for archive.
