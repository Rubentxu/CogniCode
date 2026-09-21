# Merge Receipt — M5.1b — Statement-level extraction

> Cycle: `p-c1fac1fea05615c6/m5-1b-statement-extraction`
> Path: **A-lite**
> Phase: **release**
> Persisted at: 2026-09-14T21:36:30Z
> CWD: `/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode`

## Push Evidence

| Field | Value |
|---|---|
| Branch | `main` |
| Remote | `origin = git@github.com:Rubentxu/CogniCode.git` |
| Local HEAD (pre-push) | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| Local HEAD (post-push) | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| `origin/main` (pre-push) | `90edff1f68d8d3e1219a648d27b48a88b86ce90f` |
| `origin/main` (post-push) | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| Commits pushed | 7 (`28dce397`, `6b8bee7f`, `23d7839e`, `47b25c86`, `448d81bd`, `2f1f638a`, `6bd72e78`) |
| Push command | `git push origin main` |
| Push exit_code | 0 |
| Push output | `90edff1f..6bd72e78  main -> main` |
| Post-push equality | `test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"` → EQUAL (exit 0) |
| Push digest (sha256) | `25063040e5aac70665faef761605d51cbc9d0358c0b1402c5c982f66eb47030d` |

## Authority

- The full local `HEAD` SHA equals the full `origin/main` SHA after the direct push.
- No pre-push hooks reformatted any committed change.
- No force-update was performed; push was a fast-forward (`90edff1f..6bd72e78`).
- `no-pending-effects` gate evidence is bound to this receipt.

## Linked

- `release-report.md` (full report at `openspec/changes/m5-1b-statement-extraction/release-report.md`)
- `release-receipt.md` (annotated tag evidence at `openspec/changes/m5-1b-statement-extraction/release-receipt.md`)
