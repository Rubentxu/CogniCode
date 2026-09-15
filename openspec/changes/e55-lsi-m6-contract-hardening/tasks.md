# Tasks — cycle e55 — M6 contract hardening

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Work units

### WU-1 — Shared namespaced identifiers
- New `findings::namespaced` (`NamespacedName`, `NamespacedError`,
  `validate`) with the canonical `namespace.name` grammar.
- `SubjectPattern`, `FindingKind`, `DetectorId` all delegate to it;
  `DetectorId` becomes namespaced.

### WU-2 — Capability vs cost
- `AnalysisCapability` (8 variants, set semantics) + `EscalationTier`
  (T0–T5), replacing `AnalysisLevel`.
- `DetectorIr.requires: BTreeSet<AnalysisCapability>`; `escalation_tier()`
  = max tier of `requires`.
- Validator V7 becomes `MissingCapability{capability,index}`; FLOW/EXCLUDE
  ⇒ `GraphQuery`, `VERIFY feasible_path` ⇒ `SymbolicFeasibility`.

### WU-3 — Execution authority (P0)
- `ExecutionId`, `DetectorDigest`, `DetectorExecutionRef`
  (`authority_at_execution`, `detector_digest`, `execution_id`).
- `DetectorExecutionRef::from_definition(&DetectorIr, version, exec_id)`.
- `Finding.detector: DetectorExecutionRef`; `Finding::can_block()`
  requires `detector.can_block()` (authority at execution).

### WU-4 — Status + origin
- `FindingStatus` += `RiskAccepted`, `Suppressed`; `is_active()` unchanged
  in spirit (only Open/Accepted active).
- `FindingOrigin::{Detector, LegacyQuality{issue_id, rule_id}}` +
  `Finding.origin`.

### WU-5 — QualityIssue projection update
- Populate `origin`, record `Candidate` authority at execution, map
  statuses distinctly (`wontfix` ⇒ RiskAccepted, `suppressed` ⇒ Suppressed,
  `false_positive` ⇒ FalsePositive).

### WU-6 — Doc + governance
- Fix the `satisfies(A)` → `satisfies(D)` doc bug.
- Umbrella: state.yaml is authoritative; tasks.md 7.1 label →
  `EvidenceClass`; `finding-evidence-model` wording clarified.

## Acceptance gate
- `cargo test -p cognicode-core --lib domain::findings` green.
- `cargo check -p cognicode-core` green; fmt clean; domain pure.
- New test `candidate_detector_never_blocks_even_when_promoted_later`.
- Conventional commit, no AI trailers.
