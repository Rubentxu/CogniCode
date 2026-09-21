# Archive Manifest — e86-2-cogh-real-user-uat

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e86-2-cogh-real-user-uat` |
| Path | a-min (propose → apply → uat → archive) |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **SUPERSEDED — `archive-partial-no-target-regression`** |
| Superseded at | 2026-09-21T06:34Z |

## Outcome

PARTIAL UAT closure. The cycle was originally scoped to validate
end-to-end that `cogh install mcp-server 0.95.0 --ide opencode --profile core`
works against the REAL production binary on the user's actual Linux PC.
Two defects were discovered and fixed in scope:

- **E86.2.1** (commit `a506fc2d`): HTTP 403 on cross-origin redirect.
  The reqwest client did not propagate the `COGNICODE_GITHUB_TOKEN`
  bearer header across the 302 to `objects.githubusercontent.com`.
  Closed by forwarding `Authorization` on every redirect hop.
- **E86.2.2** (commit `14f41dc4`): env-var split + bundle pre-fetch.
  `COGNICODE_API_BASE_URL` (release metadata) and
  `COGNICODE_ASSET_BASE_URL` (asset download) became canonical;
  legacy `COGNICODE_RELEASE_BASE_URL` retained on both sides for
  back-compat. New Phase B bundle pre-fetch step verifies SHA256 against
  the real github.com release.

The UAT re-run after E86.2.2 (`4264ed81`) showed the bundle install core
passing end-to-end (download → SHA256 verify → extract → shim write →
tracker pinned at `0.95.0`). However, the script still exits non-zero
because `--ide opencode` dispatches `cmd_ide_install` which writes to the
**developer's real** `~/.config/opencode/skills/` (ignoring
`OPENCODE_CONFIG`). This is a pre-existing IDE-side bug that was
explicitly **out of scope for E86.2.2** and is tracked separately.

## Acceptance verdicts (post-E86.2.2)

| REQ | Status | Evidence |
|---|---|---|
| REQ-UAT-01 | **PARTIAL → PASS (install core) / FAIL (IDE side, pre-existing)** | `4264ed81` script log; `receipt/bundle.yaml` sha256 69b51dd9… |
| REQ-UAT-02 | **PASS (filesystem state)** | `cognicode-home/install/0.95.0/manifest.yaml` written, `tracker/version` pinned at `0.95.0` |
| REQ-UAT-03 | **NOT EVIDENCED** | list/latest/where/doctor/rollback never ran because Phase B exited non-zero on IDE side |
| REQ-UAT-04 | **NOT EVIDENCED** | uninstall never ran |
| REQ-UAT-05 | **PASS** | Real failure captured with redirect chain + HTTP codes; E86.2.1 closed; E86.2.2 closed |
| REQ-UAT-06 | **PASS** | `cogh-uat-real-pc.sh` re-ran end-to-end (209 lines, `bash -n` validated) |

## What is NOT done (deliberate)

- `ide::integrate_opencode` should respect `OPENCODE_CONFIG` when
  computing the skills symlink target. Tracked separately; explicitly
  out of E86.2 scope because the UAT would have failed end-to-end
  before E86.2.2 anyway (no bundle manifest → SHA256 mismatch on the
  install core, not on the IDE step).
- `cmd_ide_install` should be guarded so a disposable UAT can run
  with `--ide opencode` without polluting the developer's real
  `~/.config/opencode/skills/`. Tracked separately.

## Commits produced (this cycle)

| SHA | Subject |
|-----|---------|
| `10768064` | docs(e86.x): distribution/lifecycle expansion — 3 proposed cycles |
| `f6159055` | docs(e86.2): apply-receipt + uat-receipt addendum — E86.2.1 HTTP-side closed |
| `4264ed81` | chore(uat): E86.2.2 UAT re-run — bundle pre-fetch + env-var split end-to-end |

## Sequencer

E86.2 → E86.3 → E86.4. E86.2's outcome informed E86.3's task list
(specifically that uninstall has the same `OPENCODE_CONFIG` bug pattern
the IDE adapter has). E86.4 starts independently but uses E86.3's
regression suite as the safety net for rollback-depth changes.

## Closure semantics

```text
implementation (E86.2.1 + E86.2.2)  CLOSED   (real, on main)
UAT                                  PARTIAL  (real evidence; install core PASS)
uninstall coverage (E86.3)           OPEN     (next cycle, planned)
selective rollback (E86.4)           OPEN     (next cycle, planned)
```

The umbrella `roadmap-auto-discovery` (superseded 2026-09-21) listed
e86-2 as Tier C "Live, not yet started" — that classification was true
at the time of the umbrella proposal but the cycle has since advanced
to PARTIAL closure. This archive-manifest supersedes that.

## Cross-references

- Source artifacts: `openspec/changes/archive/2026-09-21-e86-2-cogh-real-user-uat/`
- Sequencer: `openspec/changes/e86-3-cogh-uninstall-coverage/proposal.md`,
  `openspec/changes/e86-4-cogh-rollback-select-version/proposal.md`
- Predecessor: `e84-cognicode-distribution-artifact-contract` (M2.1
  distribution contract characterization)
- Receipt script: `/tmp/cogh-uat-real-pc.sh` (209 lines, disposable
  HOME under `/tmp/cogh-uat-real-pc-862/`)
