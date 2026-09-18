# E86.2.3 — Receipt (UAT re-run)

## REQ-by-REQ acceptance status

| REQ | Status | Notes |
|---|---|---|
| REQ-UAT-01 | **PARTIAL → PASS (install core + IDE integration)** | install bundle + OpenCode skills symlink land in disposable target; no HOME pollution. Phase B `--ide opencode` flag still triggers a pre-existing layout bug (`install/{ver}` vs `versions/{ver}`) — out of E86.2.3 scope. |
| REQ-UAT-02 | **PASS** | `cognicode-home/install/0.95.0/manifest.yaml` written; tracker pinned to `0.95.0`. Snapshots captured. |
| REQ-UAT-03 | **PASS** | list/latest/where/doctor all ran (Phase C, all `ok`). |
| REQ-UAT-04 | **PASS** | uninstall ran (Phase D, `ok`). |
| REQ-UAT-05 | **PASS** | Real failure recorded with redirect chain + HTTP codes; re-run proves both E86.2.1 and E86.2.2 land correctly. |
| REQ-UAT-06 | **PASS** | Script re-ran end-to-end; pre/post HOME snapshot proves zero pollution. |

## Pre/post HOME snapshot (WU4)

```
$ cat /tmp/cogh-uat-real-home-pre.snapshot
/home/rubentxu/.cognicode/install/0.95.0/mcp-server/skills|501c92bb75cf600c79d7ed56c1722281ee204b70dc9a31e1b40cc07c7134f737|1789722950

$ # after UAT (post = pre)
$ sha256sum /home/rubentxu/.config/opencode/opencode.json
501c92bb75cf600c79d7ed56c1722281ee204b70dc9a31e1b40cc07c7134f737  /home/rubentxu/.config/opencode/opencode.json
$ stat -c '%Y' /home/rubentxu/.config/opencode/opencode.json
1789722950
$ readlink /home/rubentxu/.config/opencode/skills/cognicode-0.95.0
/home/rubentxu/.cognicode/install/0.95.0/mcp-server/skills
```

Real HOME config sha256 + mtime + symlink target: identical before and
after. **Zero pollution.**

## Disposable-side footprint

```
$ ls -la /tmp/cogh-uat-real-pc-8623/opencode-config/skills/
cognicode-0.95.0 -> /tmp/cogh-uat-real-pc-8623/cognicode-home/versions/0.95.0/mcp-server/skills
```

The symlink lives entirely under the disposable HOME; the symlink target
also lives under the disposable HOME (after the rollback that re-installed
into `versions/`).

## Cycle verdict

PASS. All four WUs (WU0 characterization → WU5 gates) closed without
needing a STOP. Cycle is bounded: one file changed (`cmd/ide.rs`), 343
insertions, 34 deletions. No new dependencies, no new crate, no new
filesystem abstraction.

## Follow-ups filed separately (out of scope)

1. `install.rs::run_install` — skills source path uses
   `cognicode-home/install/{ver}/mcp-server/skills` but the real layout
   has it at `cognicode-home/versions/{ver}/mcp-server/skills`. Triggers
   `link_or_copy failed` in the Phase D reinstall path of the UAT.
2. `ide::detect_opencode` should use `Path::is_file()` not
   `Path::exists()` so a freshly-written `OPENCODE_CONFIG` target does
   not trigger a redundant integration on a subsequent install that did
   not pass `--ide opencode`.
