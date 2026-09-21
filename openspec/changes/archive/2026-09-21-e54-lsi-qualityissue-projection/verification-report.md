# Verification Report — cycle e54 — QualityIssue compatibility projection (M6.3)

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e54 — QualityIssue compatibility projection |
| Path | A-lite |
| Base HEAD | `513bf315` (post-e53) |
| Type | additive pure domain |
| Verify verdict | **PASS** |

## Verification

### V-1 — Module tests

```
cargo test -p cognicode-core --lib domain::findings
```

Result: **36 passed; 0 failed** (15 detector IR + 11 finding + 10 projection).

### V-2 — Build + purity + format

- `cargo check -p cognicode-core` → Finished, 0 errors.
- Domain purity: no `sqlx`/`tokio`/I/O imports in `domain/findings/`.
- `cargo fmt -p cognicode-core --check` → clean.

### V-3 — Policy is enforced, not just documented

`projected_finding_is_not_explainable_and_cannot_block` asserts that a
projected legacy issue (even `critical`) is not explainable and does not
clear a `FindingGate`. This is the key safety property of the projection.

## Follow-ups

- Wire the projection into the Explorer/MCP quality surface (adapter layer).
- Re-evidence pass to upgrade projected findings before they may block.

## Conclusion

Legacy quality rows can be represented in the new Finding model without
inheriting blocking authority. PASS.
