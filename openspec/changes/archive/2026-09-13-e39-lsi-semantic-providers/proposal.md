# Proposal: e39 — Semantic Provider Pipeline (LSI M4)

> Delivery: `auto-chain` — chained slices, stacked to main, commit per verified cycle.

## Intent

`CompositeProvider` (`crates/cognicode-core/src/infrastructure/lsp/providers/composite.rs:41`) falls back LSP→tree-sitter with `warn!` only, dead `FallbackResult`, tier invisible to `fact_bridge`. M4 gates: explicit precision, unresolved-over-fabricated.

## Scope

### In Scope

- Domain `PrecisionTier` + `ProviderDiagnostic` in `domain/traits/code_intelligence.rs`; 6 trait ops intact.
- Policy-gated tier pipeline in `CompositeProvider`; results declare tier+diagnostics; realize/remove `FallbackResult`.
- Observability: diagnostics, composite counters (MCP/CLI), `tracing`; Event Log = M7.
- Tier→Provenance in `fact_bridge/lsp_facts.rs` (S2→Extracted, S1→Inferred, S0→Ambiguous; `docs_confidence_rules.rs:60`) into `ProvenanceRecord.detail`; unresolved → uncertainty, never fabricated; no new `ProducerKind` (bincode).
- Fixtures (`sandbox/fixtures` pattern): Rust/TS full; Java (jdtls) gated; precision targets per tier.
- Riders: CP-5; DUP-2/3/6 if mocks duplicate; SCIP spike criteria only (M5+).

### Out of Scope

SCIP/LSIF adapters; Event Log + read-set tracing (M7); new MCP/CLI surfaces; Ladybug adapter; GAP S2 (quarantined); formal UAT (A5).

## Capabilities

### New Capabilities

None — umbrella `semantic-provider-pipeline` spec already authored; e39 implements it.

### Modified Capabilities

None. Flag: fixture/precision-target conformance unspecified by umbrella — candidate `provider-pipeline-conformance` delta.

## Approach

Exploration option 1 + thin slice of 3: tier types → composite pipeline → lsp_facts mapping, reusing `ProvenanceRecord` (`fact.rs:76`); no new enum variants.

## Affected Areas

(under `cognicode-core/src/`)

| Area | Impact |
|---|---|
| `domain/traits/code_intelligence.rs` | tier/diagnostic types |
| `infrastructure/lsp/providers/{composite,fallback,lsp}.rs` | pipeline, stamps |
| `application/fact_bridge/{lsp_facts,batch_builder}.rs` | tier→provenance, detail |
| `application/workspace_session.rs`, `interface/mcp/handlers/lsp_handlers.rs`, `cognicode-cli`, `cognicode-core-mock` | surfacing, ripple |
| `sandbox/fixtures/`, `crates/cognicode-core/tests/` | fixtures, CP-5 |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Trait ripple to MCP/CLI/mocks | Medium | additive types; keep 6 ops |
| jdtls availability → flaky Java | High | availability-gated |
| Heuristic mis-join classed Extracted | Medium | never Extracted |
| Bincode: new enum variants | Low | `detail` reuse |
| Sub-µs bench noise (e38.2) | Medium | noise-attribution |

## Rollback Plan

Revert offending slice commits (one per cycle). Trait extension additive — restore two-tier composite from history; new types stay dormant. Fixtures additive; predicates untouched.

## Dependencies

e38.1/e38.2 evidence landed (SubjectIndex/CP-4); jdtls optional; no new crates.

## Success Criteria

- [x] Fallback observable: diagnostics+counters+tracing (gate 1). (fresh composite tests: S2 `Unavailable` diagnostic naming provider+tier+outcome, per-tier counters 1/0/1; rust/ts/java runtime conformance runs confirm diagnostics on the default path. W3 hierarchy readiness latency noted and fixed in e39.1 `d2358bcf` with the bounded fallback readiness)
- [x] No fabricated resolutions; uncertainty recorded (gate 2). (gated lsp_facts tests: S0 stays Ambiguous `assert_ne!(Extracted)`; exhausted queries record site + exhausted tiers and emit zero facts. W1 contradiction-check mechanism differs from D4's wording — check runs at observation admission (`add_tiered_observation`), not in `finish()`; design D4 corrected at archive time, spec sentence "fail batch construction" held)
- [x] Rust/TS fixtures hit precision targets; Java gated (gate 3). (declaration-only targets: rust/ts matching declarations pass serverless as S0×4+S1 with no numeric threshold; Java jdtls PATH-probe — absent → declared-unavailable branch passes with `Unavailable` diagnostics and no S2-tier result; available branch environment-skipped, recorded as an honest gap)
- [x] Unresolved propagates via lsp_facts (gate 4). (three exhaustion sites record `UnresolvedRecord{site,query,exhausted_tiers}`; no subject/target fabrication. W2 legacy error UX on definition/hover noted and fixed in e39.1 `d2358bcf`: deepest-attempted error variants restored, clean misses stay `Ok(None)`)
- [ ] CP-5 green; lint+test-unit per slice; A5 UAT deferred post-M4. (deferred, A5 — CP-5 3/3 gated, `just lsi-fixtures check` 42/42, `just lint`/fmt/clippy green and per-slice test-unit verified; formal UAT runs intentionally deferred to a post-M4 cycle, so this criterion stays unchecked)

## Proposal question round

1. Story: trust via declared precision vs pre-detector hardening (group 7)?
2. Numeric precision targets or declaration-only?
3. Counters user-visible or internal+tracing?
4. S0 mis-bind classed Ambiguous — acceptable or unresolved-only?

Assumptions: internal+tracing; declaration-only; jdtls optional; Ambiguous heuristics OK.
