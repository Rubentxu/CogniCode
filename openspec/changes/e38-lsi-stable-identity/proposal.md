# Proposal: e38 — Stable Entity Identity (LSI M3)

> **Delivery: auto-chain** — chained slices, stacked-to-main; no commits without user approval; NEW identity files + additive kernel/bridge edits.

## Intent

e37 id strings embed `line`: any line shift changes identity (fails UAT-U20); renames/moves lack continuity. M3 needs durable identity without invalidating e37 evidence (goldens, `PINNED_IDENTITY_DIGEST`, from_facts harness).

## Scope

### In Scope

- `StableEntityId` newtype + occurrence wiring (`OccurrenceId` unused in `ids.rs`).
- Matcher over `facts_in_snapshot(N)`/`(N+1)`: T0 exact-FQN → T1 path+name+kind → T2 git evidence → T3 fingerprint; statuses `Matched|New|Terminated|Ambiguous`; fails closed (ADR-038).
- Fingerprint v1 from facts (kind, name, callee/type-ref multisets) via `DeterministicAnalyzer` — no extractor/`ProducerKind` change.
- `RenameEvidencePort` + shell-out adapter (`git diff -M --find-renames`).
- Workspace isolation suite (collisions = 0) and benchmark harness (own pinned digest) with fixtures (rename, rename+edit, move, move+edit, colliding names, line shift, control); `just lsi-identity`.
- UAT-U20/U21/U22 coverage (deferred runs reported honestly — A5).

### Out of Scope

- Fact validity intervals; extractor changes; ADR-039 `SnapshotDescriptor` wiring.
- Cross-repo identity (M14); Ladybug persistence; consumer cutover.

## Capabilities

**New Capabilities:** None — umbrella `specs/stable-entity-identity/spec.md` authored; e38 implements it.

**Modified Capabilities:** None.

> **Flag for sdd-spec:** umbrella's 2 scenarios leave margin/tiers/statuses untestable; delta must pin terminology (spec "EntityId" = `StableEntityId`; occurrence = snapshot `EntityId` + `OccurrenceId`) + matcher contract — delta `stable-entity-identity`, not a new capability.

## Approach

Snapshot `EntityId` stays the occurrence key — facts, goldens, store schemas untouched. Continuity is derived in-memory: two snapshots + optional rename evidence → `(ws, snap, EntityId) → (StableEntityId, status, evidence, confidence)`; matcher sorts facts itself.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/evidence_kernel/` (`ids.rs`, `ports.rs`, new continuity module) | Modified/New | `StableEntityId`, `RenameEvidencePort`, matcher, fingerprint v1 |
| `crates/cognicode-core/src/infrastructure/git/` | New | Shell-out rename-evidence adapter |
| `crates/cognicode-core/src/application/fact_bridge/` | Modified | Additive wiring |
| `crates/cognicode-core/tests/`, `sandbox/fixtures/lsi-identity/<case>/`, `justfile` | New | Benchmark + isolation suites, fixtures, recipe |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Terminology drift: spec "same EntityId" vs code `StableEntityId` | High | Spec delta pins the mapping |
| Fixtures are not standalone git repos | High | Fixture-declared evidence; adapter tests use throwaway `git init` repos |
| e37 perf-gate sub-us noise resurfaces | Medium | Reuse e37 bench-compare pattern |
| `ProducerKind` variant shifts bincode indices | Medium | Reuse `DeterministicAnalyzer` + detail |

## Rollback Plan

Remove new continuity/port/adapter files + additive edits; drop `just lsi-identity`. Facts, goldens, `PINNED_IDENTITY_DIGEST`, store schemas untouched; snapshot `EntityId` remains occurrence key (ADR-038 legacy-key-as-metadata). No migration, no re-pin.

## Dependencies

- e37 goldens/harness stay green (additive-only); `git` CLI for adapter tests.

## Success Criteria

- [ ] Line-shift identity retention 100% (UAT-U20)
- [ ] File-move retention ≥99% (UAT-U21)
- [ ] Controlled rename benchmark ≥95% precision / ≥90% recall (UAT-U22)
- [ ] Workspace isolation: collisions = 0
- [ ] Ambiguous never force-matched, fail-closed (ADR-038)
- [ ] e37 `PINNED_IDENTITY_DIGEST` unchanged (no re-baseline)

## Proposal question round

Autonomous fallback assumptions:

1. Reintroduced symbol = new `StableEntityId` (no resurrection)?
2. `Ambiguous` terminal per snapshot; no backfill?
3. Thresholds pinned constants, re-pinned like e37?
4. No continuity read port beyond tests in M3?
