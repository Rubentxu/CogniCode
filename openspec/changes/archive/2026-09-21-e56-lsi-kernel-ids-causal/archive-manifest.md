# Archive Manifest — e56 — kernel ids + structured causal

> Cycle: A-lite | Milestone: M6 (Findings & Detector IR) | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e56-lsi-kernel-ids-causal` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** — closes the items deferred from e55 |
| Path | A-lite |
| Phases completed | explore → design → tasks → apply → verify → archive |
| Final status | **ARCHIVED** |
| Base SHA | `8097fc7c` (post-e55) |
| Verify verdict | **PASS** |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e56-lsi-kernel-ids-causal/exploration-report.md` |
| Design | `openspec/changes/e56-lsi-kernel-ids-causal/design.md` |
| Tasks | `openspec/changes/e56-lsi-kernel-ids-causal/tasks.md` |
| Verification report | `openspec/changes/e56-lsi-kernel-ids-causal/verification-report.md` |
| Implementation | commit (this cycle) — `refactor(cognicode-core): lift kernel ids out of the feature gate + structured causal` |

## What was delivered

| Item | Change |
|------|--------|
| Kernel ids ungated | `domain::kernel_ids` (new) holds `EntityId`, `OccurrenceId`, `SnapshotId`, `FactId`, `EvidenceId`, `StableEntityId`, `ExecutionId`. `evidence_kernel::ids` is now a re-export shim, so the gated kernel keeps one source of truth and every existing path resolves. |
| `ExecutionId` added | `exec:N` display; links a finding to a concrete detector execution. |
| `EvidenceRef` removed | `Finding.evidence: Vec<EvidenceId>` — the M6 model now uses the real kernel id, so no placeholder hardens into an API. |
| Structured causal | `CausalStepKind` + `CausalStep { kind, subject, fact, evidence, detail }` with navigable refs and builders. |
| Projection update | QualityIssue projection emits `CausalStepKind::Location`. |

## Evidence

- `cargo test -p cognicode-core --lib domain::findings` → 47 passed.
- `cargo test -p cognicode-core --lib kernel_ids` → 12 passed (now default).
- `cargo test -p cognicode-core --features evidence-kernel --lib evidence_kernel`
  → 82 passed (shim intact).
- `cargo check --workspace` → 0 errors; fmt clean; domain pure.
- 41 pre-existing failures in unrelated areas confirmed by stash A/B
  (environment/cwd-dependent, out of scope).

## Milestone progress (M6)

| Umbrella task | Status |
|---------------|--------|
| 7.1 Finding/Risk/EvidenceClass lifecycle | ✅ base (e53) + hardened (e55) + kernel-grounded (e56) |
| 7.2 Detector IR schema/parser/validator | ✅ base (e52) + hardened (e55) |
| 7.3 AST detector backend | next |
| 7.4 graph-pattern backend | pending |
| 7.5 dataflow backend | pending |
| 7.6 QualityIssue compatibility projection | ✅ (e54) + updated (e55/e56) |
| 7.7 Axiom rule import tooling | pending |

**The M6 contract is now coherent end to end** (Detector IR → execution
authority → Findings → kernel-grounded evidence → navigable causal lineage →
gate), which was the pre-condition the review set before building backends.

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.

## Verification command

```
cargo test -p cognicode-core --lib domain::findings kernel_ids
```

Returns `47 passed` and `12 passed` (0 failures). ✅
