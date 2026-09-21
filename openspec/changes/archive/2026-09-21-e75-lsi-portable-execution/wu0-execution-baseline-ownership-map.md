# e75 WU0 — Execution baseline ownership map

> e75 WU0 deliverable: an explicit ownership map of every
> sandbox/execution-touching surface BEFORE adding abstractions.
> No code; only what exists today and what could/should be reused.
>
> This is the strategic pre-condition for e75 WU1 (ExecutionSpec /
> ExecutionOutcome boundary). User directive: "No code until it is
> clear what should be reused."

## 1. The five "execution" surfaces in the codebase today

The e75 directive points at five candidate surfaces. Here is what
each one is and which file it lives in:

### 1.1 sandbox-orchestrator (cognicode-sandbox binary)

`crates/cognicode-sandbox/src/main.rs` — 6903 lines, hardened
Linux-only scenario runner.

- Loads scenario manifests, expands the language × tool × variant
  matrix, executes scenarios in **isolated Podman containers** via
  direct `std::process::Command::new("podman")` (line 2468).
- Exit codes: 0 = all pass/fail as expected, 1 = unexpected
  failure, 2 = infrastructure failure.
- Captures artifacts, classifies failures, emits structured JSON.
- **This is the user's "existing hardened Linux Quadlet sandbox" —
  release/validation implementation + oracle. NOT a portable domain
  contract.**

### 1.2 sandbox_core (shared types crate)

`crates/cognicode-core/src/sandbox_core/` — 8 submodules, types
shared between the binary and external callers.

- `artifacts.rs` — `PipelineStageResult`, `ResourceUsage`, `ScenarioResult`, `Summary`, `Timing`, `ValidationResult`.
- `failure.rs` — `FailureClass` taxonomy.
- `ground_truth.rs` — `GroundTruth` matching.
- `history.rs` — `RunEntry`, `TrendDirection`, etc.
- `manifest.rs` — `ExpandedScenario`, `Manifest`.
- `mcp_core.rs` — `CapturedCall`, `McpError`, `McpServer` lifecycle.
- `resource.rs` — snapshotting + delta computation.
- `scoring.rs` — quality scoring across 4 dimensions.

These are reusable for e75: the e75 `PodmanBackend` and
`NativeProcessBackend` will use the same `ScenarioResult`,
`FailureClass`, `ResourceUsage`, and timing model. They do NOT have
to be re-derived.

### 1.3 WorkExecutor (e70 trait, M9 worker)

`crates/cognicode-core/src/application/local_ci/mod.rs` —
`pub trait WorkExecutor: Send + Sync` with a single method
`fn execute(&self, work: &WorkId) -> Vec<ProducerOutput>`.

- **`InMemoryWorkExecutor`** is the current impl (BTreeMap of
  WorkId → Vec<ProducerOutput>; missing keys return empty).
- The trait comment (line 37) says: "The trait abstracts what e70
  actually does — in tests it returns a static list of [`ProducerOutput`];
  in production (e70.x) it may shell out to cargo/just/cogh."
- The dir also contains `LocalVerticalReport`, `PerWorkReport`,
  `LocalVerticalError`, the orchestration entry point
  `run_local_vertical(...)`, and the counting test executor.
- **e75 takes over this trait**: a `NativeProcessBackend` and a
  `PodmanBackend` both satisfy `WorkExecutor`. `TrialExecutor`
  composes; it does NOT replace this.

### 1.4 TrialExecutor (e72 WU3, pure/non-IO)

`crates/cognicode-core/src/application/change_proposal/executor.rs`
— `pub trait TrialExecutor` with a single method
`fn run_trial(&self, trial_id: TrialId, input: TrialInput) -> TrialEvidence`.

- **`DefaultTrialExecutor`** is pure / deterministic / no-IO. It
  composes `evaluate(...)` (e69 `policy_gate`) with
  `assemble_trial_evidence(...)` and produces the lineage-labelled
  envelope.
- It does NOT execute commands. It only evaluates pre-computed
  EvidenceBundle against a `PolicySpec`.
- **e75 keeps this trait untouched.** `TrialExecutor` may compose
  `WorkExecutor` inside `run_trial` but the two must remain
  separate. The header doc explicitly says: "do not pre-emptively
  merge the two".

### 1.5 M9 abstract execution-identity layer

`crates/cognicode-core/src/domain/execution/` — 4 submodules,
pure identity vocabulary (M7.2, e64):

- `actor.rs` — `ActorKind`, `ActorRef`, `ActorError` ("who ran it").
- `correlation.rs` — `CorrelationId`, `CorrelationError` ("which
  logical operation it belongs to").
- `scope.rs` — `AnalysisScope`, `AnalysisScopeError` ("which
  workspace + snapshot it is pinned to").
- `context.rs` — `ExecutionContext`, `ExecutionContextError` (the
  combination of the three + triggering event).

The module doc is explicit: **"What is deliberately not here:
authority."** Authority comes from admission; it is never derived
from context.

**e75 reuses these as the input/output envelope of
`ExecutionSpec`/`ExecutionOutcome`.** They do not need to grow new
fields for portable execution — the existing identity model already
captures everything needed.

## 2. The ad-hoc process-execution seams

These are scattered `std::process::Command::new(...)` /
`tokio::process::Command::new(...)` call sites in cognicode-core.
None of them go through a backend; all of them are point-of-use
spawns.

| File                                                              | Process | Capability needed  | Is the call isolated? |
|-------------------------------------------------------------------|---------|--------------------|-----------------------|
| `application/ingest/blame.rs` lines 19, 93, 140, 154, 162       | git     | read-only history  | NO — uses real HOME   |
| `application/services/file_operations.rs` line 1510, 1557        | rustc   | reads fs           | NO                    |
| `infrastructure/verification/rust_verifier.rs` lines 66, 109, 181| rustc   | validate a file    | NO                    |

These are the **invocations a future `WorkExecutor` impl might
abstract**. Each call site has different cleanliness requirements:

- `git blame` against a workspace: read-only, no isolation, but
  current implementation hits the real HOME git config.
- `rustc --emit metadata` to parse a file: can be safe-mode
  (`--crate-type=lib --emit=metadata` does not run link), but
  rustc still does FS mutation in `--out-dir`.
- rust_verifier validates output: same concerns.

**No call today goes through a `NativeProcessBackend` or
`PodmanBackend`.** Today the only "backend" is `InMemoryWorkExecutor`
in tests. The e75 WU1 contract will need to either:

- (path A) Add a process-execution seam behind `WorkExecutor` that
  all of these call sites can route through; OR
- (path B) Leave them as-is for e75 and only do the right thing
  for new isolated-execution consumers (e.g., `TrialExecutor` →
  `WorkExecutor`).

The user's directive says: "Backend-specific details may be
diagnostics, not semantic authority. Do not duplicate
WorkResult/EvidenceBundle semantics." That leans path B for e75:
introduce the seam, route new work through it, leave existing
point-of-use spawns alone until they need isolated-execution
semantics (which is when path A becomes necessary).

## 3. Podman / Quadlet assumptions

| Assumption                                       | Source                                  | Severity for e75 |
|--------------------------------------------------|-----------------------------------------|------------------|
| `podman` binary is on `$PATH`                    | sandbox-orchestrator direct invocation  | This is Linux-only and ignores macOS/Windows Podman Machine |
| Quadlet / systemd is the unit lifecycle           | implicit (not in code I found)          | Reuse model: out of scope for e75 (e75 is about a `PodmanBackend` adapter; Quadlet stays for the Linux-only oracle) |
| `podman_image(...)` is a Linux container image    | `sandbox_core/manifest.rs`              | e75 WU2 must support direct Linux Podman AND VM-backed Podman Machine on macOS/Windows, without rewriting the manifest |
| Mount semantics: read-only, rw, overlay          | implicit in the run command             | e75 WU4 (workspace/mount normalization) must handle Windows drive paths + Unix mount semantics uniformly |
| Network: `--network=host` is acceptable          | maybe — not yet checked                 | e75 should preserve the **isolated** property: no host network without explicit config |

## 4. Command and result DTOs (today)

| DTO                            | Location                                                 |
|--------------------------------|----------------------------------------------------------|
| `ProducerOutput`               | `application/evidence_bundle/producer.rs`                |
| `ProducerSlot` / `ProducerSource` / `BundleEntry` / `EvidenceBundle` | `application/evidence_bundle/mod.rs` |
| `ValidationResult` + `ResourceUsage` + `ScenarioResult` + `PipelineStageResult` + `Summary` + `Timing` | `sandbox_core/artifacts.rs` |
| `FailureClass`                 | `sandbox_core/failure.rs`                                |
| `PerWorkReport` + `LocalVerticalReport` + `LocalVerticalError` | `application/local_ci/mod.rs` |
| `TrialEvidence` / `TrialInput` / `TrialId`     | `application/change_proposal/trial.rs`        |
| `AnalysisScope` / `ActorRef` / `CorrelationId` / `ExecutionContext` | `domain/execution/`       |

**e75 will NOT add a new `WorkResult`.** Instead, it will reuse
`ProducerOutput` (the smallest unit) for a single executed step,
and `EvidenceBundle` for the entire trial. The "execution result
DTO" of e75 WU1 will be a thin adapter on top of `ProducerOutput` —
not a new type that re-derives exit-code semantics.

## 5. Path / mount handling (today)

- `crates/cognicode-core/src/application/services/file_operations.rs`
  uses `PathBuf` everywhere. POSIX path semantics assumed.
- `crates/cognicode-sandbox/src/main.rs` constructs Podman `-v`
  bind mounts as `host:container` strings. Path conversion is
  local; no `path-clean` helper.
- `crates/cognicode-core/src/sandbox_core/manifest.rs` does NOT
  store canonical mount paths; the scenario YAML embeds them.

For e75 WU4 this is the area of highest portability hazard:

- **Windows drive paths:** `C:\workspace` vs `C:/workspace` vs
  `\\?\C:\workspace` — Podman Machine maps to VM-side paths.
- **`\` vs `/`:** the existing code uses `Path::join` which on
  Windows produces `\`. If the manifest stores `/`, that breaks.
- **Case sensitivity:** Windows paths are case-insensitive; the
  Podman backend receiving a `/workspace` lookup against a
  `/Workspace` actual will fail to find it.
- **Read-only mounts:** Podman `--mount type=bind,...:ro` is the
  right answer; `--volume :ro` is the legacy spelling. Both
  currently used in places.
- **Workspace mapping:** the user's directive calls for "explicit
  workspace mapping" — the `ChrootSpec` semantics will need to
  encode at minimum: source path, target path inside container,
  read-only flag, propagation (private, shared, slave).

## 6. e75 WU0 ownership map (table form)

The user wanted "an explicit ownership map." Concretely:

| Concern                                       | Module/Type                                            | Owner after WU1 |
|-----------------------------------------------|--------------------------------------------------------|-----------------|
| Capability discovery (host OS, podman state)   | `crates/cognicode-cli/src/cmd/doctor.rs` (e74 reuse)   | unchanged; e75 reuses |
| Execution identity (scope, actor, context, correlation) | `crates/cognicode-core/src/domain/execution/` | unchanged |
| Trial envelope (assembly + policy gate)        | `crates/cognicode-core/src/application/change_proposal/{trial,executor}.rs` | unchanged; `TrialExecutor` composes `WorkExecutor` |
| Work orchestration (which work + how)           | `crates/cognicode-core/src/application/local_ci/mod.rs` (`WorkExecutor`) | **`WorkExecutor` becomes the seam** |
| Backend: in-memory (tests)                    | `InMemoryWorkExecutor`                                   | unchanged |
| Backend: native process (e75 WU2)             | new `NativeProcessBackend: WorkExecutor`                | **e75 WU2 introduces** |
| Backend: podman isolation (e75 WU3)            | new `PodmanBackend: WorkExecutor`                       | **e75 WU3 introduces** |
| Validation/output data shape                    | `EvidenceBundle` + `ProducerOutput` + `sandbox_core::artifacts` | unchanged; reused |
| Sandbox-orchestrator binary                    | `crates/cognicode-sandbox/src/main.rs`                  | **stays the Linux oracle**, NOT the portable contract |
| Process spawn point-of-use (currently bare)    | `blame.rs` / `file_operations.rs` / `rust_verifier.rs`  | **out of e75 scope**; revisit when any of them needs isolated execution |

## 7. Decisions deferred (none)

The "before adding abstractions" directive asks for clarity on what
gets reused. Reuse decisions taken at WU0:

- **Reuse `WorkExecutor` for the backend seam.** No new
  `ExecutionBackend` parallel trait; no new `BackendSpec`
  envelope. The trait is in `application/local_ci/mod.rs`.
- **Reuse `TrialExecutor`** as the trial envelope. Compose
  `WorkExecutor` underneath; do not merge them.
- **Reuse `EvidenceBundle` + `ProducerOutput`** for the result
  shape. e75 produces `ProducerOutput` from `WorkExecutor::execute`;
  `LocalVerticalReport` aggregates them as today.
- **Reuse e74 doctor** for capability discovery. e75 will NOT
  add a second probe subsystem. New probes land in `doctor.rs`.
- **Reuse `domain/execution/`** for identity. The new
  `ExecutionSpec`/`ExecutionOutcome` types carry
  `AnalysisScope`/`ActorRef`/`CorrelationId`/`ExecutionContext`
  — they do NOT introduce new identity fields.
- **Sandbox-orchestrator stays as-is** for the Linux oracle lane.
  It is not the portable contract; we only consume its types via
  the shared `sandbox_core` crate.

Decisions NOT taken at WU0 (deferred):

- Concrete `ExecutionSpec`/`ExecutionOutcome` shape (WU1).
- `PodmanBackend` filesystem/host-network semantics (WU3, after WU4).
- Whether to backport path-normalization to `file_operations.rs` /
  `blame.rs` (only if path A in §2 is chosen).
- Whether `git`/`rustc` are "safe enough" for `NativeProcessBackend`
  (depends on WU2 semantics).

## 8. Critical invariants reaffirmed

These came from the user's directive. They will be re-tested at
WU5/WU6.

1. **If work requires isolation AND no isolation backend is
   available: explicit Unavailable / InsufficientCapability.
   Never silent native fallback.**

2. **Platform semantics stay in adapters.** No `podman`, `systemd`,
   `quadlet`, `wsl`, `hyper-v` symbols cross into
   `domain/` / `application/` types.

3. **ExecutionBackend surface is small on day one.**
   `NativeProcessBackend` + `PodmanBackend`. No `DockerBackend` or
   `RemoteBackend` until a concrete e75 requirement proves the
   initial pair insufficient.

4. **TrialExecutor ≠ WorkExecutor.** The two compose; they do not
   merge. The header doc has been carrying this contract since
   e72 WU3; we keep it.

5. **Execution success never grants promotion authority.**
   `ExitCode::Zero` → `ProducerOutput` → `EvidenceBundle` →
   `PolicyGate` → `PromotionEvaluation` → `PromotionPermit`. Never
   `ExitCode::Zero` → promotion. The pipeline is unchanged.

## 9. STOP-class conditions for e75 (re-stated)

The user enumerated seven STOP conditions. The first round of
self-review is whether any of them is already violated by what we
plan to do. **None are.** Specifically:

- `cargo-dist` was rejected in e74 WU5. No new major dep is
  planned for e75.
- M9 lineage is unchanged; no canonical-truth shift.
- `TrialExecutor` / `WorkExecutor` ownership is preserved by
  sticking to the existing traits.
- No fail-open isolation fallback will be written — the WU1
  contract explicitly demands `Unavailable` on isolation-required
  + unavailable-backend.

If any of these conditions flips during e75, that is the STOP class.

## 10. Open handoff

e75 WU0 complete: ownership map produced, no code written yet.

e75 WU1 next: introduce a minimal `ExecutionSpec`/`ExecutionOutcome`
boundary that:
- expresses the `Success | CommandFailure | InfrastructureFailure |
  UnavailableCapability | Timeout/ResourceLimit` taxonomy the user
  specified;
- threads `AnalysisScope` + `ActorRef` + `CorrelationId` from
  `domain/execution/`;
- produces a `ProducerOutput` shape compatible with
  `EvidenceBundle`;
- does NOT introduce a parallel `WorkResult` or duplicate
  `EvidenceBundle` semantics;
- has explicit contract tests before any backend is implemented.

No code in this commit.
