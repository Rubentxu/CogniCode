# Release Receipt — M5.1b — statement-extraction tag

> Cycle: `p-c1fac1fea05615c6/m5-1b-statement-extraction`
> Path: **A-lite**
> Phase: **release**
> Persisted at: 2026-09-14T21:36:30Z
> CWD: `/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode`

## Annotated Tag

| Field | Value |
|---|---|
| Tag name | `m5-1b-statement-extraction` |
| Tag type | `tag` (annotated) |
| Tag peel SHA | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| `git rev-parse HEAD` | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` (= peel SHA) |
| `git rev-parse origin/main` | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` (= peel SHA) |
| Tag message | `Release M5.1b — statement-level extraction (A-lite cycle, 7 commits, PASS_WITH_WARNINGS)` |
| Tag create command | `git tag -a m5-1b-statement-extraction 6bd72e789a04922e22e3bc282884c9f7d2af36a0 -m "..."` |
| Tag push command | `git push origin refs/tags/m5-1b-statement-extraction` |
| Remote verification | `git ls-remote origin 'refs/tags/m5-1b-statement-extraction^{}'` → `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |

## Tag Operations

| Step | Command | Exit | Result |
|---|---|---:|---|
| Create local tag | `git tag -a m5-1b-statement-extraction 6bd72e78... -m "..."` | 0 | tag created |
| Verify annotated type | `git cat-file -t refs/tags/m5-1b-statement-extraction` | 0 | `tag` |
| Verify peel to HEAD | `git rev-parse 'refs/tags/m5-1b-statement-extraction^{}'` | 0 | `6bd72e78...` (= HEAD) |
| Push tag | `git push origin refs/tags/m5-1b-statement-extraction` | 0 | `* [new tag] m5-1b-statement-extraction -> m5-1b-statement-extraction` |
| Verify remote peel | `git ls-remote origin 'refs/tags/m5-1b-statement-extraction^{}'` | 0 | `6bd72e78...` (= HEAD) |

## Authority

- The remote annotated tag peels to the verified `main` SHA.
- The tag is annotated (not lightweight).
- The tag was pushed exactly once (idempotency satisfied — no second version created).
- `no-pending-effects` gate evidence is bound to this receipt.

## Linked

- `merge-receipt.md` (push evidence at `openspec/changes/m5-1b-statement-extraction/merge-receipt.md`)
- `release-report.md` (full report at `openspec/changes/m5-1b-statement-extraction/release-report.md`)
