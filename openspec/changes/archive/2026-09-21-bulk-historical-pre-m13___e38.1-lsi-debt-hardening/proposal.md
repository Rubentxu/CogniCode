# Proposal: E38.1 LSI Debt Hardening

## Intent

The 2026-09-13 LSI debt pass (`docs/CogniCode_Living_Software_Intelligence/RETIREMENT-LEDGER.md` §3) classified e36–e38 findings ACCIDENTAL. e36–e38 are committed/archived, so shared-file refactors are safe. This cycle retires them before cutover: broken bench, untyped dual-line-base FQN grammar (6+ sites), split kind codec with silent fallback, cross-producer subject mismatch, stale identity digest, duplicated resolver, ~150 dead lines.

## Scope

### In Scope
- **DEAD-1 (HIGH)**: bench feature gating — dual-gated imports + generic bench require both features.
- **CP-1 (HIGH)**: typed `SymbolFqn` (domain) — explicit line-base constructors (fact-side 1-based / legacy 0-based), centralized assemble/parse, all sites migrated; grammar byte-identical.
- **CP-2/OE-9 (MED)**: single `SymbolKindDetail` codec + exhaustive round-trip test; loud fallback replaces silent `SymbolKind::Unknown`; kind-multiset harness assertions.
- **CP-3 (HIGH latent)**: canonical subject grammar documented; subject normalization in `lsp_facts` (1-based FQN when resolvable, else fallback); RED-first dual-producer join test.
- **CP-6 (MED)**: one conscious `PINNED_IDENTITY_DIGEST` re-pin (adds 1-based line rule, references `SymbolFqn`) + self-checks; matcher digest separate.
- **DUP-7 (MED)**: one shared callee resolver (exact match → lowercase + lexicographic-smallest-FQN tie-break).
- **Trims (LOW)**: dead SnapshotId methods, test-only EntityIdTable accessors, `RelationKind::name()`, import-keeper test, duplicate repeated-extraction test, unused OccurrenceId surface, facade re-exports; mark `facts_of`/`KernelError::Store` reserved.

### Out of Scope
CP-4 id-space guard; CP-5 tie-break test; DUP-2/3/6 test-support extraction; S2 harness extension.

## Capabilities

### New Capabilities
None — pure debt/refactor cycle; grammar byte-identical.

### Modified Capabilities
None. CP-6 re-pins within `identity-benchmark`'s existing requirement (spec pins no digest value).

## Approach

Bottom-up, five stacked units: identity modules → site migration + bench gate → CP-3 subjects (RED-first) → harness assertions + re-pin → trims. Change-scoped verification per `.agent/TESTING-STATE.md`; never the full suite.

## Affected Areas

- `cognicode-core/src/domain/` (evidence_kernel, aggregates/symbol) — Modified: new modules, trims, FQN sites
- `cognicode-core/src/infrastructure/` (graph projections, parser) — Modified: FQN/codec/resolver refactors
- `cognicode-core/src/application/` (fact_bridge, ingest) — Modified: codec site, subject normalization, trims
- `cognicode-core/tests/`, `benches/`, fact-bridge contract docs — Modified: assertions, re-pin, bench fix, grammar doc

## Risks

- Digest/golden drift (Med) → byte-identical grammar rule; 1.0/1.0 scores; 42/42 fixtures
- Loud fallback exposes unknown kinds (Low) → intended; harness kind-multiset checks fidelity
- Trim removes hidden consumer (Low) → consumer audit; scoped tests

## Rollback Plan

Five stacked PRs, each independently revertible (modules → sites → subjects → re-pin → trims). Digest re-pin is one commit — reverting restores e37 prose. No schema migration.

## Dependencies

e36–e38 committed/archived; feature flags as configured.

## Success Criteria

- [ ] Bench compiles both-featured; no dual-gated refs with `evidence-kernel` alone.
- [ ] Scores 1.0/1.0; fixtures 42/42; identity + isolation gates 1.0.
- [ ] One FQN parser, one codec, one resolver; join test RED→GREEN.
- [ ] Digest re-pinned once; matcher digest untouched; clippy/fmt clean.
