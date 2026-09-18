# e88 — Fresh Linux Lifecycle UAT (public-consumer baseline)

Date: 2026-09-18. Baseline: **v0.97.0 public release** (non-draft, `627b2efc`).
Environment: disposable HOME (`/tmp/e88-vertical/home`), no checkout, no
`target/debug`, no localhost, no staging, no `COGNICODE_*` seams. Host: x86_64 Linux.

## Vertical executed (all OBSERVED)

| # | Step | Result |
|---|---|---|
| 1 | `curl` public `install.sh` → bootstrap Layer 0 | OK (`cogh 0.97.0`) |
| 2 | `cogh install mcp-server --version 0.97.0 --profile reviewer` | resolved 0.97.0 (tag v0.97.0), installed to `versions/0.97.0`, tracker 0.97.0 |
| 3 | `cogh doctor` | `overall: healthy` (canonical shim probe) |
| 4 | `cogh update --channel stable` | re-resolved 0.97.0, idempotent, tracker 0.97.0 |
| 5 | `cogh rollback` | **FINDING F1** (below) |
| 6 | reinstall + doctor | healthy again |
| 7 | `cogh uninstall mcp-server --version 0.97.0 --ide opencode` | removed tree, journal, tracker pin; clean |
| 8 | second uninstall | graceful idempotence: "not installed; nothing to do" (exit ok) |

## Findings

### F1 — `rollback` leaves a stale tracker pin (real bug, e86 journal gap)

In `installer_transaction.rs::commit()` the on-disk journal is persisted
(`lifecycle_journal::write`) **before** `journal.record(WroteTracker { .. })` is
added to the in-memory journal. The persisted envelope therefore contains
`CreatedDir`/`WroteManifest` but **no `WroteTracker` effect**, so:

1. `cogh rollback` correctly reverses the install (removes `versions/0.97.0`,
   shims) and consumes the journal;
2. but the tracker file still pins `0.97.0`;
3. `cogh doctor` then reports `overall: UNHEALTHY` (pin without a tree) —
   an inconsistent state a fresh user can reach with one command.

Contrast: the **uninstall** path handles this correctly (`✓ cleared tracker pin`).
Note the code comment claims the tracker record is "for in-process rollback
only", which contradicts REQ-LJ-02 (restore previous tracker on rollback) for
the persisted-journal case.

**Proposed fix (e88.1):** move `journal.record(WroteTracker { .. })` BEFORE
`lifecycle_journal::write(...)` so the persisted envelope includes the tracker
restoration, mirroring the planted-envelope shape used in tests
(`plant_journal` records WroteTracker; production does not — the tests pass
while the product path is broken).

### F2 — doctor post-uninstall reports UNHEALTHY (by-design observation)

After a clean uninstall (tracker pin intentionally cleared), `cogh doctor`
says `WARN Core health: tracker/version missing (no pinned version)` →
`overall: UNHEALTHY`. For a deliberately uninstalled machine "healthy with
nothing installed" may be the right verdict. Flag as a doctor-semantics
decision, not a bug.

## Conclusion

The public-consumer vertical (install → doctor → update → uninstall →
idempotence) is **GREEN**. The rollback leg exposed **F1**, a real
tracker-restore gap in the persisted journal. e88 status: **PASS WITH
FINDINGS** — F1 should be fixed in a follow-up slice (e88.1) with a regression
test asserting the *persisted* envelope contains `WroteTracker`.
