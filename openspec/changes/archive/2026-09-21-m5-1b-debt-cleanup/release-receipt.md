# Release Receipt — m5-1b-debt-cleanup tag

> Cycle: `p-c1fac1fea05615c6/m5-1b-debt-cleanup`
> Path: **A-min**
> Phase: **release**
> Persisted at: 2026-09-14T22:05Z
> CWD: `/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode`

## Annotated Tag

| Field | Value |
|---|---|
| Tag name | `m5-1b-debt-cleanup` |
| Tag type | `tag` (annotated) |
| Tag peel SHA | `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` |
| `git rev-parse HEAD` | `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` (= peel SHA) |
| `git rev-parse origin/main` | `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` (= peel SHA) |
| Tag message | `m5-1b-debt-cleanup: close 2 follow-up items from m5-1b verify report` |
| Tag create command | `git tag -a m5-1b-debt-cleanup aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d -m "..."` |
| Tag push command | `git push origin refs/tags/m5-1b-debt-cleanup` |
| Remote verification | `git ls-remote origin 'refs/tags/m5-1b-debt-cleanup^{}'` → `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` |

## Tag Operations

| Step | Command | Exit | Result |
|---|---|---:|---|
| Create local tag | `git tag -a m5-1b-debt-cleanup aaa821c4 -m "..."` | 0 | tag created |
| Verify annotated type | `git cat-file -t refs/tags/m5-1b-debt-cleanup` | 0 | `tag` |
| Verify peel to HEAD | `git rev-parse 'refs/tags/m5-1b-debt-cleanup^{}'` | 0 | `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` (= HEAD) |
| Push tag | `git push origin refs/tags/m5-1b-debt-cleanup` | 0 | `* [new tag] m5-1b-debt-cleanup -> m5-1b-debt-cleanup` |
| Verify remote peel | `git ls-remote origin 'refs/tags/m5-1b-debt-cleanup^{}'` | 0 | `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` (= HEAD) |

## Authority

- The remote annotated tag peels to the verified `main` SHA.
- The tag is annotated (not lightweight).
- The tag was pushed exactly once (idempotency satisfied — no second version created).
- `no-pending-effects` gate evidence is bound to this receipt.

## Linked

- `merge-receipt.md` (push evidence at `openspec/changes/m5-1b-debt-cleanup/merge-receipt.md`)
- `verify-report.md` (verification at `openspec/changes/m5-1b-debt-cleanup/verify-report.md`)
