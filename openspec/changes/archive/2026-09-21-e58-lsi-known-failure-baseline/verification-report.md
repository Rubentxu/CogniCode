# Verification Report — cycle e58 — known-failure baseline

> Cycle: B-direct (housekeeping) | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e58 — known-failure baseline |
| Path | B-direct (housekeeping) |
| Base HEAD | `ebf6a40a` (post-e57) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Baseline matches the tree

```
python3 scripts/check_known_failures.py
```

Result: `OK: observed failure set matches baseline (41 entries)`, exit 0.

### V-2 — Drift is detected

Running against a baseline with one entry removed:

```
python3 scripts/check_known_failures.py --baseline /tmp/kf_drift.yaml
```

Result: `NEW REGRESSIONS (1)` listed, **exit 1**.

### V-3 — Unexpectedly-fixed is detected

The comparator also fails when a baseline test passes (it reports
`UNEXPECTEDLY FIXED`), so a fixed test cannot silently linger in the
baseline.

## Conclusion

A new regression can no longer hide behind the 41 known failures. PASS.
