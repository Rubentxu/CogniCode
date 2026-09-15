# Archive Manifest — e58 — known-failure baseline

> Cycle: B-direct (housekeeping) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e58-lsi-known-failure-baseline` |
| Path | B-direct (housekeeping) |
| Final status | **ARCHIVED** |
| Base SHA | `ebf6a40a` (post-e57) |
| Verify verdict | **PASS** |

## What was delivered

- `scripts/known_failures.yaml` — machine-readable baseline of the
  41 environment-dependent `cognicode-core` lib failures, each with a
  category and reason.
- `scripts/check_known_failures.py` — runs the target and compares the
  FAILED set; exits 1 on a NEW failure **or** on an unexpectedly-fixed test;
  `--update` regenerates the baseline.
- `just check-known-failures` / `just update-known-failures`.

## Evidence

- Clean run: exit 0, `41 entries` matched.
- Drift run (entry removed): exit 1, `NEW REGRESSIONS (1)`.

## Why no tag

Housekeeping; no production change.

## Verification command

```
python3 scripts/check_known_failures.py
```

Exits 0 when the failure set matches the baseline. ✅
