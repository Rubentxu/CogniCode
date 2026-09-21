# Archive Manifest — cycle e75 — Portable Execution

> Cycle: A-lite (degraded-but-governed) | Milestone: M14 — Distribution | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e75 |
| Milestone | M14 — Distribution (portable execution contract + host-execution adapters) |
| Requirement | isolated project-code execution on native Linux/macOS/Windows without silent fallback |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `149f5ccb` (e74 closure authorization) |
| Spec delta | none (foundation cycle; spec lives in `proposal.md`) |

## Lifecycle note (honest)

**e75 was never instantiated as a formal SDDK cycle in the ledger.** Same pattern as e67-e70+e74:
six work units authored with deterministic checkpoints (WU0 doc + WU1-WU6 code); each WU landed
in a separate commit. Closure route: D3 defer + on-disk archive (this manifest).

This archive commit closes the on-disk artifact gap (proposal + tasks + wu0-execution-baseline
were already committed; archive-manifest was missing).

## Commits

| Commit | WU | Summary |
|--------|----|---------|
| `3982f7c8` | WU0 | `docs(e75)` — execution baseline ownership map (no code) |
| `b413dde3` | WU1 | `feat(lsi)` — portable execution contract (spec + outcome + backend seam) |
| `052d2445` | WU2 | `feat(lsi)` — NativeProcessBackend (host-execution adapter, no silent fallback) |
| `f995ad6d` | WU3 | `feat(lsi)` — PodmanBackend + host discovery (linux direct / macos machine / windows wsl) |
| `ff43c0e9` | WU4 | `feat(lsi)` — workspace/mount normalization (canonical knowledge preservation) |
| `e1b36cb2` | WU5 | `feat(portable-execution)` — wire ExecutionBackend into TrialExecutor via trial_runtime |
| `7df13db5` | WU6 | `feat(portable-execution)` — portable-execution UAT with Linux oracle + macOS/Windows waivers |
| `a89be4d3` | fix | `chore(portable-execution)` — remove unused imports flagged by clippy -D warnings |
| `27fc9179` | fix | `chore(e75+e76)` — resolve clippy -D warnings on the e75+e76 seam |

## Delivered

### WU0 — Execution baseline ownership map (no code)

- Documents which subsystem owns each execution concern (who decides the backend,
  who decides the workspace, who decides the lifecycle).
- Pure documentation; no behavior change.

### WU1 — Portable execution contract

- `ExecutionBackend` trait: `pub trait ExecutionBackend { fn execute(&self, spec: ExecutionSpec) -> ExecutionOutcome; }`.
- `ExecutionSpec` carries the contract (cwd, env, command, args, expected shape).
- `ExecutionOutcome` carries the outcome (exit code, stdout/stderr, duration, observation).
- Backend is selected by policy, NOT by silent fallback.

### WU2 — NativeProcessBackend

- Host-execution adapter. Runs commands on the host OS directly.
- **No silent fallback**: if the requested backend is unavailable, the request fails
  with a typed error (`BackendUnavailable`), not a swap to a different backend.
- Linux/macOS/Windows implementation in a single module; per-OS gating where unavoidable.

### WU3 — PodmanBackend + host discovery

- Podman container execution.
- Host discovery:
  - **Linux**: direct (Podman runs natively, no machine).
  - **macOS**: Podman machine (QEMU-backed).
  - **Windows**: WSL.
- Discovery is deterministic per platform; reported in `cogh doctor`.

### WU4 — Workspace/mount normalization

- Canonical knowledge preservation: workspace paths and mount points are normalized
  before execution so the produced facts are stable across host/container boundaries.
- Mount semantics: the kernel treats the workspace as the same logical entity whether
  it is a host directory or a container bind mount.

### WU5 — Wire ExecutionBackend into TrialExecutor

- `TrialExecutor` (the trial-evaluation seam from M9 follow-on work) uses `ExecutionBackend`.
- `trial_runtime` module wires the trait object through.
- No parallel verdict path: e69's `PolicyGate` remains the gate; e75 only adds the
  backend that produces the evidence the gate evaluates.

### WU6 — Portable-execution UAT

- Linux oracle: native execution + Podman native, both verified.
- macOS / Windows: explicit waivers (no native runner in CI yet); documented in the UAT plan.

## Acceptance (per verification in WU6)

- `cargo test -p cognicode-runtime portable_execution` PASS (Linux oracle).
- `cargo clippy -p cognicode-runtime --all-targets -- -D warnings` clean on touched paths.
- `cargo fmt --check -p cognicode-runtime` clean.
- macOS / Windows: waived (documented in `wu6-cross-platform-uat.md` pattern).

## Out of scope (deferred)

- Remote workers (later cycle).
- AI agents (out of M14).
- Replacing `cogh` lifecycle/profile/plugin/lock semantics.

## Related cycles

- **e74** (predecessor): portable runtime & distribution — establishes the supported
  target matrix and `cogh doctor` capability discovery.
- **e76** (successor): self-hosting validation — closes the loop by having CogniCode
  analyze itself through the portable execution path.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — release route blocked; closure via direct push.
- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode.
