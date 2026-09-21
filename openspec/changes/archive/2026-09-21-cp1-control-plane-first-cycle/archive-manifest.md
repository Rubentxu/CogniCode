# Archive Manifest — cp1-control-plane-first-cycle

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c1/cp1-control-plane-first-cycle` |
| Path | a-min |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **SUPERSEDED — `archive-partial-runtime-pending-architecture`** |
| Superseded at | 2026-09-21T06:45Z |

## Outcome

IMPLEMENTED with fail-closed runtime wiring exercised live in the
product binary; architecture evaluation in runtime NOT demonstrated
(no product path to admit constraints yet).

| Status line | Status |
|---|---|
| CP1.0 application/HTTP implementation | GREEN |
| CP1.0 production runtime wiring (composition) | GREEN (WU5 commit `1a722d38` — `--with-architecture` flag) |
| CP1.0 final product acceptance — fail-closed | GREEN (live HTTP UAT, product binary, 6/6 probes PASS) |
| CP1.0 architecture evaluation in runtime | NOT DEMONSTRATED (no product path to admit constraints yet) |

The vertical lives in 3 files under HEAD:

- `crates/cognicode-core/src/application/architecture/control_query.rs` (378 LOC)
- `crates/cognicode-core/src/application/architecture/mod.rs` (30 LOC)
- `crates/cognicode-explorer/src/api.rs` (control_query wiring: lines 510, 544, 600-602, 635, 1212-1221)

Endpoint: `GET /control-plane/workspaces/:workspace_id/architecture`
(mounted only when `with_control_query(Some(...))` is called).

## Commits produced

| SHA | Subject |
|-----|---------|
| `6169d454` | WU2/WU3: introduce ControlQueryService + read-model DTOs |
| `fed57b95` | WU4: HTTP vertical GET /control-plane/... + C1-C5 tests |
| `674c3795` | discovery — with_control_query called only from tests |
| `e8727a24` | corrected abstract verdict to "IMPLEMENTED / RUNTIME ACCEPTANCE PENDING" |
| `1a722d38` | WU5: composition root opt-in via `--with-architecture` flag |
| `a32e7e9f` | C6 regression guard for T12 path traversal echo |
| `47214597` | declare toolchain-only musl limitation in §12 |
| `d1f771ba` | docs(cp1.0): live HTTP UAT executed on product binary |

## Cross-references

- Cycle artifacts: `closeout-receipt.md`, `e78-inventory.md`, `proposal.md`,
  `specs/`
- Predecessor: CP0 (FIT_WITH_CONSTRAINTS verdict, archived alongside this
  cycle at `openspec/changes/archive/2026-09-21-cp0-backstage-fit-spike/`)
- Forward blocker: a product path that admits `ArchitectureConstraint` for
  evaluation. Until that exists, CP1.0's read-model is wired but cannot
  be fed. This is recorded in the LSI umbrella as the e78
  consumer-count checkpoint re-evaluation trigger.

## Closure semantics

```text
implementation      CLOSED (real, on main; 3 files + C1-C6 tests)
runtime wiring     GREEN  (--with-architecture flag in composition root)
live HTTP UAT      GREEN  (6/6 probes on product binary)
architecture eval  NOT DEMONSTRATED — needs ArchitectureConstraint
                              admission path (separate cycle)
archive closure    DONE   (this manifest)
```
