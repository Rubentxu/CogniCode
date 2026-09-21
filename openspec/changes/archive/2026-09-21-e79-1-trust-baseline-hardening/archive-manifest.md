# Archive Manifest — e79.1 Trust Baseline Hardening

> Cycle: e79.1 (LSI trust-baseline-hardening) | Phase: archive | Date: 2026-09-17
> Closure pattern: **administrative archive** (git-level). e79.1 was never
> instantiated as a formal SDDK cycle record; the SDDK workflow engine itself
> was validated end-to-end by the canary cycle
> `e79-1-canary-characterize-debt-002` (see below), which WAS driven through
> the full normal lifecycle.

## Cycle summary

| Phase | Status | Artifact |
|-------|--------|----------|
| characterize | DONE | `openspec/changes/e79-1-trust-baseline-hardening/characterization.md` (commit `c77d0229`) |
| apply (P0-A ledger migration) | DONE | `ledger-migration/` (MIGRATION.md, ACTIVATION-FAILURE.md, src/) |
| apply (P0-B test isolation) | DONE | `crates/cognicode-core/src/application/services/refactor_service.rs` |
| apply (P1.1/P1.2/P1.3 drifts) | DONE | 6 files across domain/application/infrastructure |
| verify | DONE | known-failures baseline exact match; workspace + subsystem suites GREEN |
| archive | DONE | this document + `state.yaml` umbrella update |

## What e79.1 repairs

Bounded stabilization of three governance debts, chosen because automated
authorship (e80a) must not start on a broken governance substrate.

### P0-A — DEBT-SDDK-002 (ledger schema drift): RESOLVED

Two `cycle.supersede` events in stream
`cycle:p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets` (sequences 4 and 5) carried
three drifts from the current `EventEnvelopeV1`:

| Field | Legacy shape | Canonical shape |
|-------|--------------|-----------------|
| subject discriminator | `subjects.kind` | `subjects.type` |
| spurious field | `subjects.role` present | absent |
| evidence refs | `Vec<Map>` (`{"path":...}`) | `Vec<String>` |
| content hash | truncated to 32 hex | full 64-hex SHA-256 |

Migration v2 normalizes all three and recomputes the content hash with the
framework's own `EventEnvelopeV1::compute_content_hash` (framework commit
`d032939`, v1.169.64) for bit-perfect parity with the strict parser.
Preconditions are hard-pinned to the two known `event_id`s and the legacy
shape; the run is deterministic (run 1 == run 2).

The live ledger was swapped atomically only after a **real**
`sddk ledger verify` returned GREEN on an isolated copy; the post-swap verify
is GREEN on the live ledger.

The first migration attempt (v1) normalized only `subjects.kind` and was
rolled back when the framework strict parser still rejected `evidence_refs`.
That failure is preserved verbatim in `ACTIVATION-FAILURE.md` rather than
hidden: it is the evidence that the custom in-repo verifier is diagnostic
only, never the acceptance oracle.

### Canary — SDDK lifecycle validation

Cycle `e79-1-canary-characterize-debt-002` was driven through the full normal
lifecycle with supported commands only (no manual SQL, no ledger edits, no
fabricated receipts):

```text
phase.build.complete.b-direct   -> phase=verify
phase.verify.complete.b-direct  -> status=RELEASE_PENDING, phase=release
approval granted (surface.cycle_state#cycle_supersede)
supersede --reason external-obsolete -> status=CLOSED
```

Final ledger: framework `sddk ledger verify` GREEN. The canary artifacts
(`implementation-receipt.md`, `verification-report.md`) are archived beside
this manifest.

### P0-B — DEBT-SDDK-005 (`cargo test --lib` stall): RESOLVED

`test_rename_symbol_generates_preview` (and the two sibling rename tests)
placed their fixture directly under `TMPDIR` with `NamedTempFile`.
`RefactorService::rename_symbol` infers the project root from
`Path::new(&file_path).parent()` and `build_minimal_graph` then walks that
directory recursively with `walkdir`. With the fixture directly under a large
temp root, the walk parsed the whole tree.

Fix: a `TempDir` with a dedicated `project/` subdirectory holding exactly the
fixture file. The real `rename_symbol` call is preserved (not swapped to
`generate_rename_edits`), and a regression guard asserts the fixture directory
contains exactly one file.

**Measured:** 19.36s -> 0.00s with `TMPDIR=/tmp` (9397 files under it).

### P1 — DEBT-SDDK-004 (architecture drifts): 3/4 repaired, 1 explicit

| # | Drift | Status |
|---|-------|--------|
| 1 | `domain::evidence_kernel::bootstrap` -> `infrastructure` | REPAIRED (test-only `TestSchemaRegistry` implementing the existing port) |
| 2 | `domain::traits::code_verifier` -> `application::error` | REPAIRED (port-owned `CodeVerifierError`; `AppResult` conversion at the boundary) |
| 3 | `domain::behaviors::runtime` -> `application::behaviors::Clock` | REPAIRED (`Clock` moved to `domain::behaviors::ports`) |
| 4 | `domain::behaviors::runtime` -> `application::intelligence_log::CausalRecorder` | NOT REPAIRED (architectural trigger) |

`ArchitectureClock` was intentionally NOT merged into `Clock`: it carries
different semantics (an opaque audit token vs a monotonic millisecond
counter), so collapsing them would be wrong.

Drift #4 needs a real semantic decision (a domain-owned causal-recording port,
or reclassifying the recording as an application concern), not a mechanical
move. It stays visible: the e77 self-host test remains `#[ignore]` with a
reason string that names P1.4.

The e77 self-host evaluator now reports **exactly 1 drift, P1.4**, down from 4.
No new drift was introduced by the three repairs.

## Verification

| Check | Result |
|-------|--------|
| `cargo test -p cognicode-core --lib` | terminates; failure set == `scripts/known_failures.yaml` exactly (41 entries, no new regressions) |
| same, with `TMPDIR=/tmp` | 2039 passed / 0 failed / 27 ignored |
| `cargo build --workspace` | GREEN |
| architecture self-host (`--ignored`) | exactly 1 drift (P1.4), down from 4 |
| `architecture_drift_e2e` + `architecture_e77_1_wu0/wu3` | 24 passed / 0 failed |
| `findings_ast/canonical_grounding/graph` (`--features evidence-kernel`) | 22 passed / 0 failed |
| `intelligence_event_log_e2e` (`--features evidence-kernel`) | 4 passed / 0 failed |
| `behavior_authority_e2e` + `behavior_budget_e2e` (`--features evidence-kernel`) | 12 passed / 0 failed |
| clippy on touched files | no new warnings |
| `sddk ledger verify` (live) | GREEN |

**Note on the 41 baseline entries.** The in-session `TMPDIR` points under a
symlinked `/home` (`/home -> var/home`), so the file-ops symlink guard rejects
temp paths for 39 of the 41; the remaining 2 are feature-gating entries. All
41 are pre-existing and tracked in the project's own baseline. This cycle adds
none and removes none.

**Diagnosed but deliberately NOT changed (out of scope).** The two
feature-gating entries are a genuine, if small, mis-gating: the
`interproc_summary` fixture/tool is gated behind the `program-analysis-server`
feature, while the two asserting tests are not. Adding the matching `#[cfg]`
to those two tests makes them pass under the feature (verified: 2064 passed / 0
failed) and turns `cargo test --lib` into 0 observed failures with a clean
`TMPDIR`. Because that touches the maintained known-failures baseline, it is
recorded here as a finding for a future cycle rather than folded into e79.1.

## Debt status

```text
DEBT-SDDK-002  RESOLVED
DEBT-SDDK-005  RESOLVED
DEBT-SDDK-004  PARTIAL: 3/4 repaired, 1 explicit
P1.4            DEFERRED WITH ARCHITECTURAL TRIGGER
```

## Commits

* **Characterization:** `c77d0229` — `docs(e79.1): characterize DEBT-002/004/005`.
* **Implementation:** `0840ed20` — `fix(e79.1): repair DEBT-SDDK-002/005 and
  3/4 DEBT-SDDK-004 drifts` (11 modified + 1 added source file, plus the
  cycle artifacts).
* **Archive:** this commit — this manifest + `state.yaml` umbrella update.

The DEBT records (`docs/debts/DEBT-SDDK-002.md`, `DEBT-SDDK-004.md`) live
under the gitignored `docs/` tree and remain local-only working documents.

The pre-activation SQLite backups remain on disk under `ledger-migration/`
but are intentionally not committed: they are binary evidence, and
`MIGRATION.md` records the SHAs while the migration is reproducible from the
committed source.

## State transition

```text
e79.1 CLOSED
SDDK workflow engine GREEN (proven by canary lifecycle)
cargo test --lib GREEN (baseline-exact)
architecture self-host: exactly 1 known drift (P1.4)

NEXT: e80a (automated authorship) may proceed
```

## STOP

Architectural stop after e79.1. `e80a` is NOT started by this cycle. The
remaining P1.4 drift is the only known architectural item carried forward; it
blocks nothing today but MUST be resolved before the e77 evaluator is wired
into a gate (e83).
