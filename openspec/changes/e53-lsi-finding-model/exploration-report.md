# Exploration Report — cycle e53 — Finding & evidence-class model (M6.2)

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

e52 landed the Detector IR (umbrella task 7.2). The next M6 deliverable is
task **7.1 Define Finding/Risk/EvidenceGrade lifecycle** — the domain model
for evidence-backed conclusions that detectors produce.

## Design inputs

| Source | Content |
|--------|---------|
| umbrella `specs/finding-evidence-model/spec.md` | REQ "Finding evidence chain" (blocking finding links evidence + detector/version; explanation available); REQ "Evidence grades" (A/B/C/D; a grade-D hypothesis cannot satisfy a gate requiring B or stronger) |
| `ROADMAP.md` M6 | deliverables: Finding/Evidence grades; exit gate "finding causal explanation completa" |
| e52 `domain/findings/detector_ir.rs` | `DetectorId`, `FindingKind` reused as references |
| kernel `evidence_kernel::evidence` | existing `EvidenceGrade` (Supports/Refutes/Corroborates) — a **different** concept; the A–D scale is a strength class, named `EvidenceClass` to avoid collision |

## Landscape

- No `Finding` domain type exists in core (greenfield). The explorer has a
  legacy `dto::FindingSeverity { Info, Warning, Critical }` (different crate,
  no collision).
- The kernel `EvidenceId` is behind the `evidence-kernel` feature (off by
  default); findings are ungated, so evidence is referenced through a small
  opaque `EvidenceRef(u64)` with a documented reconciliation deferral.

## Strategy

Extend `domain/findings/` with `finding.rs`:

- `EvidenceClass` (A strongest … D hypothesis) with an order-based
  `satisfies(minimum)` predicate.
- `FindingSeverity`, `RiskLevel`, `FindingStatus`.
- `FindingId`, `EvidenceRef`, `DetectorRef`, `CausalStep`.
- `Finding` with `validate()`, `is_explainable()`, `can_block(gate)`.
- `FindingGate` (min evidence class + min risk) with `admits()`.

Both umbrella `finding-evidence-model` scenarios are encoded as tests.

## Impact model

| Artifact | Impact | Confidence |
|----------|--------|-----------|
| `domain/findings/finding.rs` (new) | additive pure domain | KNOWN |
| `domain/findings/mod.rs` | + re-exports | KNOWN |
| consumers | none yet | LIKELY |

## Out of scope

- Detector backends, QualityIssue projection, Axiom import tooling.
- Kernel evidence-store wiring (the `EvidenceRef` reconciliation).
