# Roadmap Amendment — Portable Runtime, Portable Execution and Self-Hosting Validation

> Date: 2026-09-16
> Scope: Living Software Intelligence umbrella
> Status: planned roadmap amendment; implementation remains per-evolutivo

## Why this amendment exists

The M0–M8 work establishes canonical facts/evidence, grounded findings, semantic deltas, affected-work planning, evidence bundles and local knowledge-driven CI. M9 adds isolated SoftwareWorld/Trial/Promotion authority. Two capabilities are still missing before executable architecture, packs and AI agents become the safest next step:

1. **Portable execution of CogniCode itself** across Linux, macOS and Windows without making containers a prerequisite for normal analysis.
2. **Self-hosted validation** where CogniCode evaluates CogniCode and its predictions are checked against independent oracles rather than accepted on self-report.

These capabilities are inserted **after M9 fork/trial/promotion and before the Packs/Architecture and AI-agent milestones**.

## Amended sequence

```text
M0–M7  Intelligence foundation
  ↓
e67     production grounding: source → canonical Fact → grounded Finding
  ↓
e68     semantic diff + affected logical work
  ↓
e69     WorkResult → EvidenceBundle → PolicyGate
  ↓
e70     local knowledge-driven CI + why_scheduled/why_decided
  ↓
M9 / e71–e73
         SoftwareWorld → ChangeProposal → Trial → Promotion
  ↓
e74     Portable Runtime & Distribution
  ↓
e75     Portable Execution / Sandbox
  ↓
e76     Self-Hosting + Platform Equivalence Harness
  ↓
Strategic review
  ↓
M10     Executable Architecture before generalized Pack ecosystem
  ↓
M11     Read-only AI agents, then Fix Agent through ChangeProposal/Trial
  ↓
M13     Historical replay + held-out governed improvement
```

The original umbrella `tasks.md` remains immutable by its own contract. This amendment adds operational sequencing without rewriting the historical decomposition.

## e74 — Portable Runtime & Distribution

**Goal:** normal CogniCode analysis is native-first and available without a container runtime.

Initial supported targets:

- Linux x86_64
- Linux aarch64
- macOS x86_64
- macOS arm64
- Windows x86_64

`cogh`, the daily CogniCode executable/MCP runtime and platform-neutral analysis must not require Podman, Docker, WSL or a VM. Container/VM support is an optional execution capability used only when isolated execution is requested.

Primary deliverables:

- release matrix aligned with the platform set already modelled by `BundleManifest::Platform`;
- deterministic platform bundle selection;
- platform adapters for paths/shims/process/filesystem semantics rather than scattered `cfg(target_os)` policy;
- `cogh doctor` capability discovery and actionable remediation;
- signing/notarization and installer strategy spike;
- evaluate `dist`/equivalent only as build/package/bootstrap machinery, never as a replacement for `cogh` bundle/version/profile semantics.

## e75 — Portable Execution / Sandbox

**Goal:** isolate execution behind capabilities instead of making Linux Quadlet the product-level contract.

Initial backend set:

- `NativeProcessBackend` for explicitly safe/native work;
- `PodmanBackend` for isolated execution.

Host strategy:

- Linux: rootless Podman directly;
- macOS: native CogniCode + Podman Machine for Linux containers;
- Windows: native CogniCode + Podman Machine/WSL2-compatible backend where available;
- future remote execution remains a seam, not an e75 implementation requirement.

The existing hardened Quadlet sandbox remains a Linux implementation/oracle, not the cross-platform domain contract.

## e76 — Self-Hosting + Platform Equivalence Harness

**Goal:** CogniCode must be able to analyse CogniCode, make a prediction before a controlled change, and have that prediction evaluated by independent observations.

Core loop:

```text
base CogniCode snapshot
  → CogniCode prediction (sealed)
  → isolated ChangeProposal/SoftwareWorld trial
  → cargo/rustc/clippy/tests/sandbox observations
  → prediction-vs-observation evaluation
  → EvidenceBundle + metrics
```

Self-hosting is not allowed to use CogniCode as its only oracle. At least three views are kept separate:

1. predicted impact/architecture/findings from CogniCode;
2. declared ground truth from mutation/scenario contracts where available;
3. external observation from compilers/tests/linters/sandbox.

The harness grows through levels:

- SH0: deterministic build/test/fmt/clippy/known-failure baseline;
- SH1: deterministic self-model;
- SH2: architecture/graph/findings on CogniCode itself;
- SH3: affected-work prediction vs observed impact;
- SH4: controlled mutation/fault corpus;
- SH5: SoftwareWorld/Trial integration;
- SH6: cross-platform semantic equivalence and historical replay bootstrap.

## Cross-platform equivalence invariant

Operating-system differences must not accidentally become knowledge differences. For a platform-neutral corpus, normalize legitimate platform metadata and compare at least:

- canonical Fact semantic digests;
- SemanticFactDelta;
- AffectedWorkPlan dispositions/reasons;
- Findings and grounding status;
- EvidenceBundle completeness;
- PolicyGate decision;
- `why_scheduled` and `why_decided` semantics.

Special attention: path separators, drive letters, CRLF/LF, case sensitivity, symlinks, Unicode boundaries, process exit semantics and platform-specific filesystem identity.

## Roadmap control rules

### Continue automatically inside an approved cycle when

- the previous WU checkpoint is GREEN;
- no new authority path is introduced;
- no new storage/source-of-truth model is introduced;
- no fail-closed invariant is weakened;
- the next WU remains inside the approved capability budget.

### Stop for strategic review when

- a new crate/major dependency is required for architecture rather than mechanics;
- a backend changes the authority model;
- portability would require semantic divergence between platforms;
- self-hosting starts using its own output as the only oracle;
- UNKNOWN/incomplete evidence would be interpreted as safe;
- e76 completes (mandatory review before M10/M11).

## Deferred on purpose

Not part of e74–e76 unless evidence forces reconsideration:

- remote/distributed workers;
- a custom CogniCode VM/appliance;
- Docker backend parity purely for feature count;
- automatic WSL/Hyper-V/Podman installation;
- generalized query-dependency tracking (DEBT-X trigger remains workload-driven);
- automatic AI promotion;
- replacing the existing external sandbox corpus.
