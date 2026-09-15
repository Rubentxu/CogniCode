# Archive Manifest — e55 — M6 contract hardening

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e55-lsi-m6-contract-hardening` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** — contract hardening before backends |
| Path | A-lite |
| Phases completed | explore → design → tasks → apply → verify → archive |
| Final status | **ARCHIVED** |
| Base SHA | `b3831584` (post-e54) |
| Verify verdict | **PASS** |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e55-lsi-m6-contract-hardening/exploration-report.md` |
| Design | `openspec/changes/e55-lsi-m6-contract-hardening/design.md` |
| Tasks | `openspec/changes/e55-lsi-m6-contract-hardening/tasks.md` |
| Verification report | `openspec/changes/e55-lsi-m6-contract-hardening/verification-report.md` |
| Implementation | commit (this cycle) — `refactor(cognicode-core): M6 contract hardening` |

## What was delivered

| Item | Change |
|------|--------|
| **P0 execution authority** | `DetectorExecutionRef{id,version,authority_at_execution,detector_digest,execution_id}`; `Finding::can_block` now requires `authority_at_execution.can_block()`. A Candidate run can never block, even if the detector is promoted later. |
| Capability vs cost | `AnalysisCapability` (set) + `EscalationTier` (cost) replace the linear `AnalysisLevel`; `DetectorIr.requires` + `escalation_tier()`; validator `MissingCapability`. |
| Namespaced ids | `findings::namespaced::NamespacedName` is the single `ns.name` validator behind `SubjectPattern`, `FindingKind` and now `DetectorId`. |
| Content digest | `DetectorIr::digest()` (fnv1a64, house convention) captured in the execution ref. |
| Distinct statuses | `FindingStatus` += `RiskAccepted`, `Suppressed`; `wontfix`/`suppressed`/`false_positive` no longer collapse. |
| Legacy origin | `FindingOrigin::LegacyQuality{issue_id, rule_id}` preserves the legacy link. |
| Doc fix | `satisfies(A)` → `satisfies(D)` comment corrected. |
| Governance | umbrella `tasks.md` now states `state.yaml` is authoritative; item 7.1 label → `EvidenceClass`; `finding-evidence-model` wording clarified (EvidenceClass ≠ kernel EvidenceGrade). |

## Evidence

- `cargo test -p cognicode-core --lib domain::findings` → **46 passed; 0 failed**.
- `cargo check -p cognicode-core` → 0 errors; fmt clean; domain pure.
- No stale references to the removed `AnalysisLevel` / `DetectorRef`.

## Deliberately deferred to e56

- Extract `EntityId/FactId/EvidenceId/SnapshotId/ExecutionId` into an
  ungated `domain::kernel_ids`; make `EvidenceRef = EvidenceId`.
- Structured `CausalStep` (kind + optional EntityId/FactId/EvidenceId).

Both touch the feature-gated `evidence_kernel`; isolating them keeps this
cycle a bounded, low-risk slice, as the review requested.

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| 7.1 Finding/Risk/EvidenceClass lifecycle | ✅ base (e53) + hardened (e55) |
| 7.2 Detector IR schema/parser/validator | ✅ base (e52) + hardened (e55) |
| 7.3 AST detector backend | pending (after e56) |
| 7.4 graph-pattern backend | pending |
| 7.5 dataflow backend | pending |
| 7.6 QualityIssue compatibility projection | ✅ (e54) + updated (e55) |
| 7.7 Axiom rule import tooling | pending |

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification command

```
cargo test -p cognicode-core --lib domain::findings
```

Returns `46 passed; 0 failed`. ✅
