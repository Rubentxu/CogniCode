# Verification Report — cycle e62.1 — kernel snapshot correctness

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e62.1 — kernel snapshot correctness |
| Path | A-lite |
| Base HEAD | `91b8b54e` (post-e61) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Characterization (WU-0)

Before the fix the characterization test **could not compile**
(`EvidenceStore::add` took 2 arguments), which is the defect at the type level:
a snapshot-pinned write was inexpressible.

### V-2 — After the fix

```
cargo test -p cognicode-core --features evidence-kernel --lib in_memory
```

Result: **29 passed; 0 failed.**

| Property | Test |
|----------|------|
| `for_fact` is pinned to the requested snapshot | `evidence_read_is_pinned_to_the_requested_snapshot` |
| `get` is pinned on the id axis (same id in two snapshots) | `evidence_get_is_pinned_to_the_requested_snapshot` |
| Re-using an evidence id in one snapshot is rejected atomically | `evidence_id_collision_in_the_same_snapshot_is_rejected` |
| `FactStore::get` is pinned; unknown ids degrade to `None` | `fact_get_is_pinned_to_the_requested_snapshot` |

### V-3 — No regressions

- `cargo test -p cognicode-core --features evidence-kernel --lib` → the failing
  test set is **identical** to the 41-entry known-failure baseline (verified by
  diffing the sorted names).
- `cargo check -p cognicode-core --features evidence-kernel --all-targets` → 0
  errors.
- `cargo check --workspace --all-targets` → 0 errors; `cargo fmt --all --check`
  → clean.

## Conclusion

The kernel's evidence store now honours the snapshot on both axes and exposes
the point reads U42 needs. This is a **kernel** fix, not an M6 accommodation:
the adapter is not asked to compensate for a storage defect. PASS.
