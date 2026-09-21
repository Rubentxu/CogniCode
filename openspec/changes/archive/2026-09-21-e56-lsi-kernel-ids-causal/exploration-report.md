# Exploration Report — cycle e56 — kernel ids + structured causal

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

e55 hardened the M6 contract but deliberately deferred two items that
require touching the feature-gated `evidence_kernel`:

1. **Reconcile `EvidenceRef` with the kernel `EvidenceId`** and lift the
   fundamental ids out of the `evidence-kernel` gate (they are domain
   vocabulary, not infrastructure). Otherwise three detector backends
   would bake a placeholder `EvidenceRef(u64)` into a de-facto stable API.
2. **Make `CausalStep` navigable** (kind + EntityId/FactId/EvidenceId
   refs), so Explorer can walk finding → entity → fact → evidence rather
   than render opaque strings.

## Landscape

- `crates/cognicode-core/src/domain/evidence_kernel/ids.rs` (367 LOC) —
  `EntityId`, `OccurrenceId`, `SnapshotId`, `FactId`, `EvidenceId`,
  `StableEntityId`, all gated behind `evidence-kernel`. Referenced by
  ~18 files via `evidence_kernel::ids::*` / `super::ids::*`.
- `crates/cognicode-core/src/domain/findings/finding.rs` — `EvidenceRef(u64)`
  placeholder + flat `CausalStep { label, detail }`.

## Strategy

**Extraction (item 1):**
- Move `ids.rs` → `domain/kernel_ids.rs` (ungated); add `ExecutionId`.
- Replace `evidence_kernel/ids.rs` with a `pub use crate::domain::kernel_ids::*;`
  shim, so every existing `evidence_kernel::ids::*` path keeps resolving
  (single source of truth, no duplicated definitions).
- Register `pub mod kernel_ids;` in `domain/mod.rs`.
- Findings use `EvidenceId` directly (drop `EvidenceRef`); `ExecutionId`
  comes from `kernel_ids`.

**Structured causal (item 4):**
- `CausalStepKind { Source, Flow, Call, Sanitizer, Guard, Sink,
  RuntimeObservation, Verification, Location }`.
- `CausalStep { kind, subject: Option<EntityId>, fact: Option<FactId>,
  evidence: Option<EvidenceId>, detail }` with `with_subject/with_fact/
  with_evidence` builders.
- QualityIssue projection emits `CausalStepKind::Location`.

## Impact model

| Artifact | Impact | Confidence |
|----------|--------|-----------|
| `domain/kernel_ids.rs` (new, ungated) | ids become available on default builds (additive) | KNOWN |
| `evidence_kernel/ids.rs` | shim; `evidence_kernel::ids::*` unchanged | KNOWN |
| `domain/findings/*` | M6-local; no consumers yet | KNOWN |
| gated kernel | verified green (`--features evidence-kernel`) | KNOWN |

## Note on pre-existing failures

`cargo test -p cognicode-core --lib` (default) reports 41 pre-existing
failures in unrelated areas (file_operations, mcp handlers,
workspace_session, program_analysis acceptance). Confirmed pre-existing by
stashing e56 and re-running one (`test_workspace_boundary_enforcement`
still failed). They are environment/cwd-dependent and out of scope.
