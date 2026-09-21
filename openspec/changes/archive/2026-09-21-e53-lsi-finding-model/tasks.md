# Tasks — cycle e53 — Finding & evidence-class model (M6.2)

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15
> Umbrella task: **7.1 Define Finding/Risk/EvidenceGrade lifecycle.**

## Work units

### WU-1 — Evidence class + scales
- `EvidenceClass { A, B, C, D }` with `satisfies(minimum)` and Display.
- `FindingSeverity { Info, Warning, Critical }` + `rank()`.
- `RiskLevel { Low, Medium, High, Critical }` + `rank()`.
- `FindingStatus { Open, Accepted, Fixed, FalsePositive }` + `is_active()`.

### WU-2 — References + causal chain
- `FindingId` (non-empty newtype), `EvidenceRef(u64)` (opaque),
  `DetectorRef { id, version }` (non-empty version), `CausalStep { label, detail }`.

### WU-3 — Finding
- `Finding { id, kind, severity, risk, evidence_class, evidence, detector,
  status, message, causal_chain }`.
- `validate()`, `is_explainable()`, `can_block(gate)`.

### WU-4 — Gate
- `FindingGate { min_evidence_class, min_risk }` + `admits()`.

### WU-5 — Tests for both umbrella scenarios
- "Hypothesis cannot satisfy strong gate" (grade D vs grade-B gate).
- "Blocking finding is explainable" (evidence + causal lineage present).
- plus ordering/Display/status/round-trip/constructor-rejection tests.

## Acceptance gate
- `cargo test -p cognicode-core --lib domain::findings` green (26 tests).
- `cargo check -p cognicode-core` green.
- Domain purity (no I/O imports); `cargo fmt --all --check` green.
- Conventional commit, no AI trailers.
