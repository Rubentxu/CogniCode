# e74 — Final acceptance ledger (PUBLIC-INTERFACE-OBSERVED)

> Every public interface touched by e74 WU0–WU6+W4FU, with the
> observed behavior at the moment of writing. The unit tests prove
> the code; this ledger proves the **public artifact**.
>
> "Public interface" here means: (a) any binary that ships, (b) any
> YAML/shell entry that runs, (c) any Rust API exported to consumers
> downstream of e74, (d) any evidence file a future runner will
> produce.

## 1. Public binaries shipped

| Binary            | Interface                              | Observed                                          | Evidence |
|-------------------|----------------------------------------|---------------------------------------------------|----------|
| `cogh`            | `cogh --version`                       | `cogh 0.95.0`                                     | UAT-JSONL |
| `cogh`            | `cogh --help`                          | prints 10+ commands                               | smoke-extended |
| `cogh`            | `cogh doctor` (clean install)          | `PASS Core health / PASS Native / PASS Isolation` (when Podman on PATH) | uat-real + doctor-empty-env |
| `cogh`            | `cogh doctor` (no isolation)           | `UNAVAILABLE Isolation backend` (with no-auto-install remediation) | doctor-empty-env.txt |
| `cogh`            | `cogh install mcp-server --version 0.95.0 --profile core` | manifest.yaml written + tracker/version=0.95.0  | /tmp/real-install-test/ |
| `cogh`            | `cogh list` / `current` / `where`      | honor `COGNICODE_HOME` env var                    | smoke-extended (during install) |
| `cognicode-mcp`   | `cognicode-mcp --version`              | `cognicode-mcp 0.95.0`                            | install-smoke-extended-linux-x86-64.txt |
| `cognicode-mcp`   | `cognicode-mcp --help`                 | shows `-c, --cwd <CWD>` + `-h, --help` + `-V`     | smoke-extended |
| `explorer-api`    | `explorer-api --version`               | `explorer-api 0.95.0`                             | install-smoke-extended-linux-x86-64.txt |
| `bundles/v0.95.0/bundle.yaml` | Bundle manifest             | valid YAML (apiVersion, kind, version, platform, profiles) | /tmp/real-install-test/.cognicode/install/0.95.0/manifest.yaml |

## 2. Public release artifacts

| Artifact                                       | Sha256 / size          | Producer                  | Locally observed |
|------------------------------------------------|------------------------|---------------------------|------------------|
| `cognicode-cli-v0.95.0-linux-x86-64.tar.gz`    | sha256 7d41a3ba…, 2.9 MB | `tar -czf … cogh`         | extracted, `cogh --version` → 0.95.0 |
| `cognicode-v0.95.0-linux-x86-64.tar.gz`        | 23.3 MB                | `tar -czf … explorer-api cognicode-mcp` | extracted, both binaries print --version → 0.95.0 |
| `evidence/e74-acceptance/linux-x86-64.jsonl`   | 4 records              | `e74-acceptance-evidence.sh` | run 20260916T174327Z-3743353, 4/4 pass |

## 3. Public release pipeline (.github/workflows/release.yml)

| Trigger                     | Lane                    | Static-coherence | Live-execution |
|-----------------------------|-------------------------|------------------|----------------|
| `push: tags: 'v*'`          | build (5 lanes)         | YAML parses; matrix matches `BundleManifest::Platform` | locally simulated: bin-list corrected, install-smoke extended |
|                             |                         | scripts/check-release-matrix.sh → OK | |
| `push: tags: 'v*'`          | install-smoke (5 lanes) | tarball names match job expectations | locally simulated: cogh+mcp+explorer all print --version |
| `push: tags: 'v*'`          | release (publish)       | verifies a `cognocode-cli-*-<platform>.tar.gz` per lane | not exercised locally (requires GH token) |

**Bug surfaced + fixed during review (this turn):**
- WU3 release.yml BIN_LIST referenced `cognicode-runtime` as a binary
  that does not exist (the package produces `explorer-api` +
  `explorer-mcp`, not a binary of the same name). Fix: drop
  `cognicode-runtime` from BIN_LIST, keep only `explorer-api` +
  `cognicode-mcp`.
- WU3 install-smoke only asserted `cogh --version` + `cogh --help`.
  Fix: extend to assert `explorer-api --version` and
  `cognicode-mcp --version`, since the runtime surface has 3 binaries.

## 4. Public docs / scripts

| Artifact                                          | Audience            | Observed                              |
|---------------------------------------------------|---------------------|---------------------------------------|
| `scripts/check-release-matrix.sh`                 | maintainers / CI    | exits 0; prints YAML targets + Rust variants; OK |
| `scripts/e74-acceptance-evidence.sh`              | acceptance runners  | JSONL produced; 4/4 pass on linux-x86-64 |
| `openspec/changes/e74-.../proposal.md`            | team                | unchanged from M9 cherry-pick         |
| `openspec/changes/e74-.../tasks.md`               | team                | unchanged from M9 cherry-pick         |
| `openspec/changes/e74-.../wu5-packaging-spike.md` | team                | recorded WU5 evaluation + 5 typed waivers |
| `openspec/changes/e74-.../wu6-cross-platform-uat.md` | team             | 4-scenario acceptance contract + manifest of waivers |
| `openspec/changes/e74-.../prt-requirement-to-check-traceability.md` | team | 8 PRT-IDs, each with check + observed result |
| `docs/adr/ADR-052-reject-cargo-dist-e74.md`       | team (ephemeral)    | recorded in local working tree, NOT in remote |

## 5. Public Rust APIs

| Module                                  | Symbol                                  | Visibility    | Used by                     |
|-----------------------------------------|-----------------------------------------|---------------|-----------------------------|
| `crates/cognicode-cli/src/cmd/platform_adapter.rs` | `pub trait PlatformAdapter`  | crate-private | installer_transaction, ide  |
| `crates/cognicode-cli/src/cmd/platform_adapter.rs` | `pub enum PlatformFamily`    | crate-private | doctor::probe_optional_isolation |
| `crates/cognicode-cli/src/cmd/doctor.rs`        | `pub struct DoctorReport`     | crate-private | layout::cmd_doctor        |
| `crates/cognicode-cli/src/cmd/doctor.rs`        | `pub fn run_doctor(&Path)`    | crate-private | layout::cmd_doctor        |
| `crates/cognicode-cli/src/cmd/bundle_manifest.rs` | `pub fn assert_host_platform` | crate-private | installer_transaction     |
| `crates/cognicode-cli/src/cmd/installer_transaction.rs` | `pub fn run(profile)` | crate-private | install::run_install       |

(The crate has no `lib.rs`; these APIs are not exported to downstream
crates. They are public-within-the-crate.)

## 6. Acceptance contract summary (per platform)

| Lane             | Acceptance scenarios (A/B/C/D)            | Evidence file                          |
|------------------|-------------------------------------------|----------------------------------------|
| linux-x86-64     | ✅ A=pass B=pass C=pass D=pass             | `evidence/e74-acceptance/linux-x86-64.jsonl` |
| linux-aarch64    | ⚠️ runner-only install-smoke; UAT pending  | `e74-uat-pending-linux-aarch64`         |
| mac-os-x86-64    | ⚠️ runner-only install-smoke; UAT pending  | `e74-uat-pending-macos-x86-64`          |
| mac-os-aarch64   | ⚠️ runner-only install-smoke; UAT pending  | `e74-uat-pending-macos-aarch64`         |
| windows-x86-64   | ⚠️ runner-only install-smoke; UAT pending  | `e74-uat-pending-windows-x86-64`        |

Install-smoke now covers 3 binaries per lane (cogh + cognicode-mcp +
explorer-api). Locally simulated on linux-x86-64, all three respond
to `--version` correctly on clean HOME.

## 7. Edge cases observed (this turn)

| Edge case                                    | Surface                             | Result          |
|----------------------------------------------|-------------------------------------|-----------------|
| `cogh install --home PATH`                   | layout::install_manifest_path        | silently ignores `--home`; uses COGNICODE_HOME instead. Pre-existing bug; e74 recorded it |
| `cogh doctor` with PATH=                     | doctor::probe_optional_isolation     | UNAVAILABLE (not Fail); remediation mentions "manually only if you need… (e75)" |
| `cogh doctor` with PATH containing podman    | doctor::probe_optional_isolation     | PASS podman detected (correct host-extras layer) |
| `cogh list` / `current` / `where`            | layout::cognicode_home              | honors COGNICODE_HOME correctly |
| `cogh install` with `--home PATH` (adversarial) | installer_transaction             | documented as known limitation; not silently fixed |
| Build with empty bundle.v0.95.0/bundle.yaml  | cogh build (was WU0)                | fixed pre-WU0 — bundle present now |
| Wrong-platform bundle in test                | assert_host_platform                | rejects loudly; integration test pinned |

## 8. Out of e74 scope (transparently recorded)

- GPG-signed tarballs (waivered: `release-not-signed-yet`)
- Apple notarization (waivered: `release-no-notarization-yet`)
- macOS `.pkg` (waivered: `release-no-pkg-yet`)
- Windows `.msi` (waivered: `release-no-msi-yet`)
- Windows code signing (waivered: `release-no-codesign-yet`)
- Linux `.deb` / `.rpm` / AppImage (out of roadmap)
- Self-hosted Podman orchestration (e75)
- Remote workers (out of scope until e77+)
- Lifting the `cogh install --home` silent-ignore bug (out of e74;
  requires refactor of `InstallerTransaction::run`)

## 9. Single-source-of-truth for going further

| Question you have                                        | Read this file                                                                          |
|----------------------------------------------------------|------------------------------------------------------------------------------------------|
| What does each WU produce?                                | commits `f935042e`..`d034d719` on `feat/e74-lsi-portable-runtime-distribution`            |
| Why did we reject cargo-dist?                            | `docs/adr/ADR-052-reject-cargo-dist-e74.md` (ephemeral local) + `wu5-packaging-spike.md` |
| Where is the PRT requirement → check traceability?       | `openspec/changes/e74-.../prt-requirement-to-check-traceability.md`                       |
| How do I lift a runner-only waiver?                      | `openspec/changes/e74-.../wu6-cross-platform-uat.md` §5                                   |
| Where is the CI/static coherence check?                  | `scripts/check-release-matrix.sh`                                                        |
| Where is the acceptance evidence?                        | `evidence/e74-acceptance/`                                                               |
