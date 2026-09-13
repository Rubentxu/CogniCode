# Proposal: E36 LSI Evidence Kernel Foundation

## Intent

LSI's first implementation slice (umbrella `cognicode-living-software-intelligence`, groups 1–2). CallGraph/GenericGraph remain the de-facto truth source, blocking SAST/runtime/reactive features. M0 freezes behavior via golden fixtures + benchmark baseline; M1 adds the canonical evidence kernel, zero consumer-visible change.

## Scope

### In Scope
- **M0**: graph consumer inventory (MCP/CLI/Explorer); golden multi-language fixtures capturing graph/symbol/impact outputs; benchmark harness + baseline; ADR-037..051 review record.
- **M1**: ids `EntityId`/`OccurrenceId`/`SnapshotId`/`FactId`/`EvidenceId`; typed `FactValue`; namespaced `RelationKind`; `ProvenanceRecord` (extends legacy `Provenance`); `Evidence` + `EvidenceGrade`; `SnapshotDescriptor`; ports `FactStore`/`EvidenceStore`/`SnapshotStore`/`SchemaRegistry`; round-trip/property tests.

### Out of Scope
- Umbrella groups 3–13: projections, identity, providers, findings, reactive, CI, forks.
- Store migration; legacy graph removal.
- LLM/agent features beyond the type-level provenance contract.

## Capabilities

> `evidence-kernel` spec delta authored in umbrella `cognicode-living-software-intelligence`; this change IMPLEMENTS it.

### New Capabilities
- None.

### Modified Capabilities
- None.

## Approach

Additive-first, hexagonal: types in `cognicode-core` domain (no I/O imports), stores as port traits, Ladybug adapters in infrastructure, feature-gated runtime wiring. `SnapshotDescriptor` maps onto the existing RevisionId/revision-store model (ADR-039), not a parallel system; M0 baseline lands first.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/value_objects/` | New | ids, FactValue, RelationKind, ProvenanceRecord, Evidence(+Grade), SnapshotDescriptor |
| `crates/cognicode-core/src/domain/ports/` | New | FactStore, EvidenceStore (kernel-namespaced), SnapshotStore, SchemaRegistry |
| `crates/cognicode-core/src/infrastructure/`, `crates/cognicode-ladybug` | Modified | adapters for new ports |
| `crates/cognicode-runtime` | Modified | feature-gated wiring |
| `sandbox/` | Modified | harness + baseline |
| `openspec/changes/e36-lsi-evidence-kernel-foundation/` | New | specs/tasks/verify artifacts |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `EvidenceStore` collides with investigation port | High | kernel-namespaced trait |
| SnapshotDescriptor vs revision model divergence | Med | RevisionId↔SnapshotId mapping (ADR-039) |
| M0 behavior drift | Low | fixtures gate before M1 code |
| Domain imports I/O crate | Low | `just lint` + review |

## Rollback Plan

Fully additive: feature-flag off, delete new modules; revert restores behavior. No format migration yet, so no data rollback. Legacy paths untouched (strangler approach).

## Dependencies

- Umbrella artifacts approved; sandbox scorecard infra for the baseline.

## Success Criteria

- [ ] M0: ≥95% critical surfaces under contract/golden fixtures; benchmark in CI; fixtures byte-stable. (NOT met: weighted coverage 33.3% vs ≥95% target — verify-report WARNING 1; fixtures byte-stable and benchmark baseline evidenced)
- [x] M1: lossless round-trip; snapshot-pinned reads never mix snapshots; type contract blocks LLM deterministic provenance. (verify-report: 9/9 scenarios compliant — round-trip, pinned-read, LlmAgent-rejection tests)
- [ ] UAT-U01..U06 pass (see question round). (deferred, A5)
- [x] ADR-037..051 review recorded. (adr-review.md carries ADR-037..051 verdicts)

## Proposal question round

Autonomous mode; review at next gate. Assumptions:

1. Bundle M0+M1, or split M0 out?
2. Reuse sandbox `release_scorecard.py` as harness?
3. `SnapshotDescriptor` maps onto existing revision model — confirm?
4. `domain/ports/evidence_store.rs` collides — assume kernel-namespaced naming?
5. Gate cites U01..U06, only U01–U04 catalogued — author U05/U06 or re-baseline?
