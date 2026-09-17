# Archive Manifest — cycle e74 — LSI Portable Runtime & Distribution

> Cycle: A-lite (degraded-but-governed) | Milestone: M14 — Distribution | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e74 |
| Milestone | M14 — Distribution: native-first for Linux/macOS/Windows |
| Requirement | umbrella `portable-runtime-distribution` (PRT-001..006) |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `d8b161c5` (umbrella add) |
| Spec delta | none (foundation cycle; spec lives in `proposal.md` + `prt-requirement-to-check-traceability.md`) |

## Lifecycle note (honest)

**e74 was never instantiated as a formal SDDK cycle in the ledger.** Same pattern as e67+e68+e69+e70:
six work units authored with deterministic checkpoints (the WU files describe each scope);
each WU landed in a separate commit. Closure route: D3 defer + on-disk archive (this manifest).

This archive commit closes the on-disk artifact gap. The full audit chain (CLOSURE-AUTHORIZATION.md,
final-acceptance-ledger.md, prt-requirement-to-check-traceability.md) was already committed
together with the WU implementations.

## Commits

| Commit | WU | Summary |
|--------|----|---------|
| `7d301395` | WU1 | `feat(e74)` — PlatformAdapter seam — replace scattered `cfg(unix)`/`cfg(not(unix))` |
| `8ad77c12` | WU2 | `feat(e74)` — deterministic bundle resolution — no wrong-platform fallback |
| `28faeb2c` | WU3 | `feat(e74)` — native release matrix — 5 platform lanes with install smoke |
| `7807fb18` | WU4 | `feat(e74)` — `cogh doctor` capability discovery across 4 dimensions |
| `843bade2` | WU5 | `feat(e74)` — packaging/sign/notarization spike — reject cargo-dist |
| `a389ce15` | WU6 | `feat(e74)` — cross-platform acceptance UAT plan + evidence script |
| `d034d719` | WU4-followup | `fix(e74)` — extend Core health with tracker/version + repair integration tests |
| `9d4d8523` | WU-fix | `fix(e74)` — release.yml bug — drop nonexistent bin, extend install-smoke to 3 binaries |
| `09f83c97` | doc | `docs(e74)` — traceability doc — explicit mapping to umbrella spec PRT-001..006 |
| `149f5ccb` | closure | `docs(e74)` — closure authorization + visible debt ledger for e75/e76 |

## Delivered

### WU1 — PlatformAdapter seam

- Single source of truth for platform-specific behaviour (paths, shims, process, filesystem).
- Replaces scattered `cfg(unix)` / `cfg(not(unix))` blocks.
- Establishes supported target build checks WITHOUT changing authority semantics.

### WU2 — Deterministic bundle resolution

- `BundleManifest::Platform` is the canonical source of truth.
- No wrong-platform fallback — the manifest either matches or the bundle is refused.
- Coherence check `scripts/check-release-matrix.sh` prevents drift between the contract
  enum and the release workflow.

### WU3 — Native release matrix

- 5 lanes declared, all backed by native runners: linux x86_64, linux aarch64,
  macos x86_64, macos arm64, windows x86_64.
- Every `Platform` enum variant has a matching lane.
- `release.yml` extended; install-smoke covers 3 binaries (cogh, MCP, IDE adapter).
- No cross-compilation; no fake platform support.

### WU4 — `cogh doctor` capability discovery

- 4 dimensions: native (linux/macos/windows), optional (e.g. jdtls), external (e.g. cargo),
  self (cogh version, tracker, repair).
- Health reporting extended with `tracker/version` + integration tests for the repair path.

### WU5 — Packaging / signing / notarization spike

- Cargo-dist REJECTED (insufficient for the multi-platform native contract).
- Decision recorded in `wu5-packaging-spike.md`: native packaging will be done per-lane
  in e75+ (Podman machine / native process / WSL); not via a cross-compiler pipeline.

### WU6 — Cross-platform acceptance UAT plan + evidence script

- Plan + executable evidence script.
- Macos/Windows lanes run with explicit waivers (no native runner in CI yet);
  Linux lane runs as the oracle.

## Closure gate (per tasks.md § "Closure gate")

> No container runtime is needed for normal platform-neutral analysis, and the release
> matrix matches the contractual platform matrix.

- 5 lanes declared, all backed by native runners (Linux only locally exercised; macOS/Windows
  have explicit waivers recorded in the UAT plan).
- `scripts/check-release-matrix.sh` exit 0 at HEAD.
- `cogh doctor` reports capabilities on Linux native without container runtime.

## Out of scope (deferred)

- Isolated project-code execution (e75 follow-on).
- Podman Machine orchestration (e75 follow-on).
- Remote workers (later).
- Replacing `cogh` lifecycle/profile/plugin/lock semantics.
- AI agents.

## Related cycles

- **e75** (successor): portable execution contract + NativeProcessBackend + PodmanBackend.
- **e76** (successor): self-hosting validation.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — release route blocked; closure via direct push.
- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode.
