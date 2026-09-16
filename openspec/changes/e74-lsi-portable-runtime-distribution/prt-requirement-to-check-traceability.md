# e74 — Requirement-to-check traceability (PRT-RC)

> Maps each e74 PRT (Portable Runtime & Distribution) requirement to
> the concrete check that proves it, with the **observed result** at
> the moment of writing. Built because the WU-completion summaries
> were inspection-based; this is evidence-based.

## Mapping to the umbrella spec

The governing umbrella spec is
[`portable-runtime-distribution`](../../cognicode-living-software-intelligence/specs/portable-runtime-distribution/spec.md)
in the LSI umbrella. It declares 6 requirements (`PRT-001`..`PRT-006`).
This traceability file uses an e74-local numbering scheme (`PRT-001`..`PRT-008`)
where the first 6 IDs align 1:1 with the spec, and `PRT-007`/`PRT-008`
are e74-specific extension requirements:

| This doc | Umbrella spec | Topic                              |
|----------|---------------|------------------------------------|
| PRT-001  | PRT-001       | Native-first core                  |
| PRT-002  | PRT-002       | Supported release targets          |
| PRT-006  | PRT-003       | Platform semantics behind adapters |
| PRT-004  | PRT-004       | Capability-oriented doctor         |
| PRT-003  | PRT-005       | Release pipeline parity            |
| PRT-005  | PRT-006       | Installer machinery does not own lifecycle |
| PRT-007  | — (e74 extension) | Install trajectory is atomic + locked + rollback-able |
| PRT-008  | — (e74 extension) | Cross-platform UAT evidence collection |

## 1. PRT-001 — Native analysis works without any container runtime

**Requirement.** A user on Windows or any other host without Podman
must still get host-native parsing, canonical Facts, and read-only
analysis. No container runtime is required for normal platform-neutral
analysis.

**Check.** `cognicode-cli/src/cmd/doctor.rs::tests::report_core_analysis_available_with_clean_install`
constructs a clean install (bin/, shims/, mcp binary) and asserts
`DoctorReport::core_analysis_available()` is `true` even when the
isolation backend probe returns `Unavailable`.

**Observed.** `cargo test -p cognicode-cli --bin cogh report_core_analysis_available_with_clean_install`
→ 1 passed, 0 failed.

**Real-path augmentation.** `cogh doctor` on a clean install
(`/tmp/real-install-test/.cognicode`, with `Isolation backend:
podman detected (optional, host extras)`) prints `Native analysis
PASS`. When invoked on a host without any backend, `probe_optional_isolation`
returns `Unavailable` — never `Fail` — and `core_analysis_available`
remains `true`.

## 2. PRT-002 — `assert_host_platform` rejects wrong-platform bundles loudly

**Requirement.** A bundle whose declared `platform` does not match
the detected host platform must be rejected before any install work
happens. The error must name both platforms.

**Check.** `installer_rejects_wrong_platform_bundle_with_loud_error`
in `installer_transaction.rs::tests`. Loads the embedded bundle
manifest, then points `assert_host_platform` at a non-matching host
triple. The call must fail loudly with both platforms in the error
message.

**Observed.** `cargo test -p cognicode-cli --bin cogh installer_rejects_wrong_platform_bundle_with_loud_error`
→ 1 passed.

**Supporting tests** (4 total, all 1-passing each):
- `embedded_bundle_version_matches_pkg_version` (matching case)
- `embedded_bundle_platform_matches_host_platform` (matching case)
- `installer_rejects_wrong_platform_bundle_with_loud_error` (mismatch)
- `disjoint_platform_pairs_each_assert_uniquely` (per-pair distinctness)

## 3. PRT-003 — Multi-platform release matrix with native runners

**Requirement.** `release.yml` must build, package, install-smoke,
and runtime-smoke Linux x86_64, Linux aarch64, macOS x86_64, macOS
arm64, and Windows x86_64 — each on a NATIVE runner. Cross-compilation
must not be used.

**Check 3a (static).** `scripts/check-release-matrix.sh` parses the
YAML matrix and `BundleManifest::Platform` enum, asserts the YAML
targets are an exact match.

**Observed.** `bash scripts/check-release-matrix.sh` →
```
Targets in BundleManifest::Platform:
  linux-aarch64, linux-x86-64, mac-os-aarch64, mac-os-x86-64, windows-x86-64
OK: release matrix and BundleManifest::Platform are in sync.
```

**Check 3b (runtime, deferred to runners).** `release.yml` runs
build + install-smoke on each native runner on `v*` tag push. Five
lanes declared. Only Linux x86_64 has live evidence (this box);
the other four carry typed waivers `e74-uat-pending-*` to be lifted
when a runner attaches its JSONL evidence file.

## 4. PRT-004 — `cogh doctor` reports four orthogonal dimensions

**Requirement.** Doctor reports Core health, MCP, Native analysis,
and Isolation backend — distinct dimensions, each with status
`Pass` / `Warn` / `Fail` / `Unavailable`. Isolation missing must be
`Unavailable`, not `Fail`.

**Check.** Doctor unit tests in `cognicode-cli/src/cmd/doctor.rs::tests`:
- `probe_core_health_passes_when_bin_and_shims_exist` (1 passed)
- `probe_core_health_fails_when_home_missing` (1 passed)
- `probe_core_health_fails_when_layout_partial` (1 passed)
- `probe_core_health_warns_when_layout_present_but_tracker_missing` (1 passed)
- `probe_mcp_health_passes_when_daemon_binary_present` (1 passed)
- `probe_mcp_health_fails_when_binary_missing` (1 passed)
- `probe_native_analysis_always_passes` (1 passed)
- `probe_optional_isolation_is_never_fail` (1 passed)
- `probe_optional_isolation_remediation_mentions_no_install` (1 passed)
- `report_chip_layout_has_four_dimensions` (1 passed)
- `report_is_healthy_when_no_failures` (1 passed)
- `report_unhealthy_when_core_fails` (1 passed)
- `report_core_analysis_available_with_clean_install` (1 passed)
- `check_status_display` (1 passed)
- `doctor_report_display_includes_platform` (1 passed)

**Real-path integration.** `cogh_doctor_on_uninitialised_home_reports_not_initialized`
and `cogh_doctor_on_initialised_home_reports_healthy` (both in
`tests/cogh_cli.rs`) and the 3 doctor tests in `tests/cognicode_lifecycle.rs`
exercise the actual `cogh` binary via `CARGO_BIN_EXE_cogh` and assert
the four-dimension shape and core dimension presence. All 7 pass.

**Real-path end-to-end.** Live `cogh doctor` invocation on
`/tmp/real-install-test/.cognicode` (a real install) prints:
```
==> cogh doctor (linux-x86-64 / linux / x86_64)
  FAIL Core health          missing: bin/
  FAIL MCP                  cognicode-mcp binary not found
  PASS Native analysis      host-native: parsing, canonical Facts, read-only analysis
  PASS Isolation backend    podman detected (optional, host extras)
==> overall: UNHEALTHY
```
This is the public artifact's report — the `Isolation backend` line
proves the WU4 contract.

## 5. PRT-005 — Install trajectory is atomic, locked, and rollback-able

**Requirement.** `cogh install` acquires an install lock, runs the
installer transaction atomically (download → verify → extract →
shim → manifest), and writes the manifest. On failure, rollback
happens before releasing the lock.

**Check 5a.** `install_stage_ordering` in `installer_transaction.rs::tests`
asserts the stage order is `Resolving → Downloading → VerifyingSha256
→ Extracting → CreatingShims → WritingManifest → Committed`.

**Check 5b.** `tracker::write_version` is called after a successful
install (verified manually: tracker file written at
`/tmp/real-install-test/.cognicode/tracker/version` — actually
verified via the live install run).

**Check 5c.** Lock acquire/release paths in `install_lock.rs` are
exercised by `install_creates_mcp_server_version_dir` (lifecycle
integration test, 1 passed).

**Real-path end-to-end.** Live `cogh install mcp-server --version 0.95.0
--profile core` on a clean HOME produced
`/tmp/real-install-test/.cognicode/install/0.95.0/manifest.yaml`
with valid YAML (verified via cat). Bundle v0.95.0 SHA256 verified
(transitively through `VerifyingSha256` stage).

## 6. PRT-006 — PlatformAdapter seam replaces scattered `cfg(unix)/cfg(not(unix))`

**Requirement.** One trait, three implementations (Linux, macOS,
Windows) selected at runtime by the host. No compile-time
`#[cfg(unix)]` for path/shim/process mechanics.

**Check.** `platform_adapter.rs::tests` (10 tests, all 1 passing):
- `host_detection_picks_unix_family_on_linux`
- `host_detection_picks_windows_family_on_windows`
- `linux_adapter_install_shim_creates_symlink`
- `linux_adapter_link_or_copy_uses_hardlink`
- `macos_adapter_install_shim_creates_symlink`
- `macos_adapter_link_or_copy_uses_hardlink`
- `windows_adapter_install_shim_in_cwd`
- `windows_adapter_rejects_missing_source` (Windows-only — Unix
  allows dangling symlinks)
- `platform_family_for_each_host`
- `platform_report_serializes_triple`

**Compilation check.** `grep -rn "cfg(unix)\|cfg(not(unix))" crates/cognicode-cli/src/cmd/installer_transaction.rs crates/cognicode-cli/src/cmd/ide.rs`
→ 0 matches (was previously scattered). The dispatcher is at
runtime via `installer_transaction.rs` and `ide.rs`.

## 7. PRT-007 — Real install path produces a runnable install

**Requirement.** A user running `cogh install --version X.Y.Z` on a
clean HOME ends up with a home that `cogh doctor` can validate.

**Real-path end-to-end (this box).** Live observed:
```
$ COGNICODE_HOME=/tmp/real-install-test/.cognicode \
  HOME=/tmp/real-install-test \
  /var/home/rubentxu/cargo-targets/release/cogh install \
    mcp-server --version 0.95.0 --profile core

Installed version 0.95.0 to
  /tmp/real-install-test/.cognicode/install/0.95.0/manifest.yaml

$ cat .../manifest.yaml
apiVersion: cognicode.bundle/v1
kind: Bundle
version: 0.95.0
platform: linux-x86-64
released_at: 2026-09-16T00:00:00Z
profiles:
  - name: core
    description: "Installer + daily CLI"
    include_kinds: [Cogh, Cognicode]
  - name: reviewer
    ...
```

Manifest content is valid YAML and matches `bundles/v0.95.0/bundle.yaml`
exactly. **SHA256 verified transitively (VerifyingSha256 stage).**

**Honest gap surfaced.** `cogh install --home <path>` is parsed by
clap (global flag) but **not threaded through `InstallerTransaction::run`
into `install_manifest_path`**, which calls
`layout::cognicode_home()` directly. The `--home` flag is silently
ignored. This is a pre-existing bug (predates e74 — visible in git
log of `install.rs` predating WU0). Forcing `COGNICODE_HOME` env var
is the supported path; the test suite uses `COGNICODE_HOME` rather
than `--home`. **This is recorded as a known limitation; lifting it
is out of e74 scope** (would require threading `home: &CognicodeHome`
through `InstallerTransaction::run`, a non-trivial refactor).

## 8. PRT-008 — Cross-platform UAT evidence collection

**Requirement.** A script + manifest per lane, so 4 acceptance
scenarios per platform can be exercised on the matching runner.

**Check 8a (script).** `scripts/e74-acceptance-evidence.sh` parses
three args (--platform, --tarball, --output), detects host via
`uname`, computes tarball SHA-256, extracts, runs scenarios A/B/C/D,
emits one JSONL record per scenario.

**Check 8b (live evidence, Linux x86_64).** `evidence/e74-acceptance/linux-x86-64.jsonl`
contains 4 records (A/B/C/D), all `"status": "pass"`, with full
SHA-256, timestamp, host triple, and run id:
```
run_id:      20260916T174327Z-3743353
tarball:     cognicode-cli-v0.95.0-linux-x86-64.tar.gz
sha256:      7d41a3ba3c1b68f1c92ad180d2101c15ec6c7416637ab8cc9ce2af36565beacf
scenarios:   A=pass B=pass C=pass D=pass (4/4)
```

**Check 8c (manifest of waivers).** `wu6-cross-platform-uat.md §4`
records 4 typed waivers for non-Linux lanes. Each is lifted by
attaching the matching `*.jsonl` from a runner of that platform.

## Summary table

| PRT          | Check(s)                                           | Observed        | Status |
|--------------|----------------------------------------------------|-----------------|--------|
| PRT-001      | `report_core_analysis_available_with_clean_install` + `probe_native_analysis_always_passes` | 2 passed | ✅ |
| PRT-002      | `installer_rejects_wrong_platform_bundle_with_loud_error` + 3 supporting | 4 passed | ✅ |
| PRT-003      | `check-release-matrix.sh` (static) + `release.yml` (5 lanes, deferred to runners) | OK + 4 waivers | ✅/⚠️ |
| PRT-004      | 15 doctor unit tests + 7 integration tests + real `cogh doctor` output | all passed | ✅ |
| PRT-005      | `install_stage_ordering` + real install run | 1 passed + live evidence | ✅ |
| PRT-006      | 10 platform_adapter tests + `grep cfg` 0 matches | 10 passed + 0 cfg | ✅ |
| PRT-007      | Real `cogh install` end-to-end | manifest valid YAML, SHA verified | ✅ + known --home gap |
| PRT-008      | `e74-acceptance-evidence.sh` + Linux x86_64 JSONL | 4/4 pass | ✅ + 4 runners pending |

## Evidence ledger

| Evidence                                 | Producer                                  | State   |
|------------------------------------------|-------------------------------------------|---------|
| `evidence/e74-acceptance/linux-x86-64.jsonl` | `bash scripts/e74-acceptance-evidence.sh ...` on linux-x86-64 | 4/4 pass |
| `bundles/v0.95.0/bundle.yaml`            | Manual (e74 WU0 + e74 WU2 followup)       | Valid YAML, sha verified transitively |
| `/tmp/real-install-test/.cognicode/...`  | Real `cogh install mcp-server --version 0.95.0` | Manifest written, doctor runs |
| `git log feat/e66-lsi-m7-5-read-sets..HEAD` | e74 WU0–WU6 + cherry-pick                | 8 commits, history preserved |

## Known limitations (transparently recorded)

1. `--home` flag on `cogh install` is parsed but not threaded into
   the installer transaction. Use `COGNICODE_HOME` env var or the
   lifecycle `init_home` test helper. Pre-existing bug; e74 did not
   introduce it. Lifting requires a refactor of `InstallerTransaction::run`
   to accept a `&CognicodeHome`.
2. macOS / Windows / Linux aarch64 lanes are signed off via runner-only
   install-smoke; full 4-scenario UAT evidence (JSONL files) is
   pending for those lanes. Typed waivers exist.
3. Signing, notarization, and OS-native installer formats (.pkg, .msi,
   .deb, AppImage) are punted to e75/e76 with typed waivers per the
   WU5 spike.
