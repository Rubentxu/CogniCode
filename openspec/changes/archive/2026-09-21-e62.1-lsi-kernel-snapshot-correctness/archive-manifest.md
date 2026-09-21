# Archive Manifest — e62.1 — kernel snapshot correctness

> Cycle: A-lite | Milestone: M6 | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e62.1-lsi-kernel-snapshot-correctness` |
| Parent umbrella | `cognicode-living-software-intelligence` |
| Milestone | **M6** — U42 prerequisite (kernel defect found by U42 review) |
| Path | A-lite |
| Final status | **ARCHIVED** |
| Base SHA | `91b8b54e` (post-e61) |
| Verify verdict | **PASS** |

## What was delivered

| Id | Fix |
|----|-----|
| WU-0 | Characterization test proving evidence leaked across snapshots (written first) |
| WU-1a | `EvidenceStore::add(ws, snap, e)`, `EvidenceStore::get(ws, snap, id)`, `FactStore::get(ws, snap, id)` |
| WU-1b | `InMemoryEvidenceStore` re-keyed by `(workspace, snapshot, …)` on both axes |
| WU-1c | `KernelError::EvidenceIdCollision` — atomic rejection, never a silent merge |
| WU-1d | Corrected the doc comment that claimed pinning was "transitive through the fact" |

## Why this is not scope creep

The kernel itself declares that a pinned read must not mix snapshots, and
`findings/ports.rs` recorded since e57 that the async kernel contract was
pending renegotiation before productive wiring. U42 is that moment, and the
defect would otherwise have surfaced only when serious historical replay was
attempted.

## Evidence

- `in_memory`: 29 passed.
- Gated core lib failures identical to the 41-entry baseline (no regressions).
- `cargo check --workspace --all-targets` 0 errors; fmt clean.

## Remaining for U42 (e62.2)

`GroundingRef` on the analysis DTOs, atomic causal evidence (one evidence atom
per causal step rather than one id shared by a whole path),
`EvidenceDescriptor` read model + `KernelEvidenceReadModel::load(...).await`,
and `FindingVerifier` full grounding (causal step's evidence must resolve **and**
point at the step's own fact in the pinned snapshot; a `Refutes` must never
enable the gate).

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.
