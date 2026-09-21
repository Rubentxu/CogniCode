# Tasks — e43 — cogh CLI conformance coverage

> Change: `e43-lsi-cogh-cli-conformance-coverage` | Phase: tasks | Date: 2026-09-15

## Work units

### WU-1 — Create `crates/cognicode-cli/tests/cogh_cli.rs`

RED-first test file covering the four scenarios documented in `spec.md`:

- `cogh_version_reports_semver_token` (REQ: `cogh --version` reports the binary version).
- `cogh_list_on_uninitialised_home_reports_not_initialized` (REQ: `cogh list` renders the plugin table, scenario "uninitialised home").
- `cogh_list_on_initialised_home_renders_table` (REQ: `cogh list` renders the plugin table, scenario "initialised home").
- `cogh_current_with_no_pinned_version_reports_unset` (REQ: `cogh current` reads the tracker, scenario "no pinned version").
- `cogh_current_reads_pinned_version_from_tracker` (REQ: `cogh current` reads the tracker, scenario "reads tracker").
- `cogh_doctor_on_uninitialised_home_reports_not_initialized` (REQ: `cogh doctor` validates installation, scenario "uninitialised home").
- `cogh_doctor_on_initialised_home_reports_healthy` (REQ: `cogh doctor` validates installation, scenario "initialised home").

Implementation notes:

- Spawn `cogh` via `std::process::Command::new(env!("CARGO_BIN_EXE_cogh"))` (Cargo injects the path at build time; this is the supported pattern for testing an owned binary).
- Pass `--home <temp>` via `tempfile::tempdir()` so each test runs against an isolated fixture.
- For "initialised home" scenarios, call `cogh init --home <temp>` first to populate the layout, then assert against the populated state.
- Use `assert_cmd` or plain `Command`+`output()` — given the crate's current minimal dev-deps, plain `Command` is fine; no new dev-dependency needed.
- For `cmd_current` "reads tracker" scenario, write the tracker file directly via `std::fs::write(tracker_path, "0.92.0\n")` after `cogh init` (this matches the spec's "plain text (one version string per line)" contract).

### WU-2 — Verify locally

```
cargo test -p cognicode-cli --test cogh_cli
```

Expected: 7 passed; 0 failed.

### WU-3 — Update conformance evidence_map

Update `sandbox/reports/evidence_map.yaml` so that the entries for the 4 promoted specs (`cognicode-cli/1`, `cognicode-cli/6`, `cognicode-cli/7`, `cognicode-cli/11`) carry:

- `status: verified`
- `evidence: crates/cognicode-cli/tests/cogh_cli.rs`

Re-run `python3 sandbox/scripts/openspec_conformance.py --evidence-map sandbox/reports/evidence_map.yaml --specs-dir openspec/specs` to confirm `pct_verified` lifts from `91.4%` to ≥ `92.2%`.

## Sequencing

Apply WU-1, verify WU-2, then apply WU-3. WU-3 is a doc-only change to the conformance evidence map (not source code); it belongs to the same commit but is logically distinct.

## Acceptance gate

- `cargo test -p cognicode-cli --test cogh_cli` passes 7/7.
- `openspec_conformance.py` reports `pct_verified ≥ 92.2%`.
- No production code change (test-only cycle).
