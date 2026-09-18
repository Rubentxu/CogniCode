# e88 FINAL — Fresh-Machine Public UAT v0.97.1

Date: 2026-09-18. Baseline: **v0.97.1** (published 2026-09-18T18:47:53Z,
non-draft, 12 assets, run 35380725902: 3/3 jobs success). Contains F1
(rollback tracker restore) and F2 (state-aware doctor) fixes.

Environment: disposable HOME (`/tmp/e88-final/home`), all bits downloaded
from the public release. NO checkout, NO target/debug, NO localhost,
NO staging, NO `COGNICODE_*` seams. Real `~/.cognicode` verified untouched.

## Vertical (all OBSERVED)

| # | Step | Result |
|---|---|---|
| U0 | SHA256SUMS + public install.sh fetched | checksums OK |
| U1 | install.sh bootstrap → `cogh --version` | `cogh 0.97.1` |
| U2 | `install mcp-server --version 0.97.1 --profile reviewer` | resolved tag v0.97.1, installed, doctor healthy |
| U3 | `update --channel stable` | idempotent, healthy |
| U4 | `rollback` | **pin restored to 0.97.1** (F1 fix live: no orphan pin); reversal removed tree/shims. See F3 below. |
| U5 | reinstall | healthy |
| U6 | `uninstall --ide opencode` | tree + journal + pin cleared |
| U7 | uninstall again | graceful idempotence; doctor `overall: healthy` with `WARN no pinned version` + `UNAVAILABLE no active runtime` (F2 fix live) |
| U8 | mise declarative backend `github:Rubentxu/CogniCode[matching=cogh-]@0.97.1` | `cogh 0.97.1` — same public artifact, Layer-1 untouched |

## Doctor semantics verification (F2)

- Clean uninstall (U7): `overall: healthy` — absent-by-design is not broken.
- Active install: MCP PASS via shim (U2, U5).
- Pin-without-tree (after U4): MCP correctly `UNAVAILABLE` (not evaluated);
  Core owns the FAIL. Honest attribution.

## New finding F3 (non-blocking, recorded)

`update` is same-version idempotent and overwrites the journal for that
version. `rollback` then reverses the update transition, restoring the pin
to the previous value (same version) while removing the tree the *original*
install created. Result: pin 0.97.1 with no tree → doctor correctly reports
UNHEALTHY (Core FAIL: missing shims/). This is the DEBT-4 one-shot journal
semantics colliding with idempotent updates. Recovery: `cogh install` (U5
proves clean recovery). Candidate for a future lifecycle cycle; NOT an e88
gate: the pre-existing-version case (0.96.0 → 0.97.x → rollback → 0.96.0)
was proven in e86 and the UAT regression suite.

## Verdict

**e88 — Fresh Linux Lifecycle UAT: CLOSED. PROVEN.**

- public bootstrap: PROVEN (install.sh + mise, same sha256 artifact)
- public install / update / rollback / uninstall / idempotence: PROVEN
- doctor lifecycle semantics: PROVEN (F2 live in a public binary)
- rollback tracker restore: PROVEN (F1 live in a public binary)
- zero test seams: PROVEN
