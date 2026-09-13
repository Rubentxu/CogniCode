# Tasks: e39 LSI Semantic Provider Pipeline (M4)

## Review Workload Forecast

~750–950 authored lines (goldens excluded); >400: chained; apply no-commit, orchestrator commits post-verify.

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | PR | Test | Harness | Rollback |
|------|------|----|------|---------|----------|
| 1 | Types+trait | PR 1 | `cargo test -p cognicode-core --lib code_intelligence` | N/A (types) | Revert file |
| 2 | Composite | PR 2 | `cargo test -p cognicode-core --lib composite` | `cargo check -p cognicode-explorer -p cognicode-runtime` | Revert slice |
| 3 | Bridge (gated) | PR 3 | `cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel` | N/A (gated) | Revert 3 files |
| 4 | Fixtures/runner/CP-5 | PR 4 | `cargo test -p cognicode-core --test provider_conformance --test cp5_tie_break --features evidence-kernel` | `just lsi-providers` | Delete additions |
| 5 | Guards/handoff | PR 5 | `just lsi-fixtures check` | `just lsi-equivalence`, `just lsi-identity` | N/A (verify) |

Umbrella `semantic-provider-pipeline`: "Fallback is visible"→P2–3; "Unresolved call"→P3–4.

## Phase 1: Domain Types (WU1)

- [x] 1.1 `domain/traits/code_intelligence.rs`: `PrecisionTier{S0..S4}` (no serde), `ProviderOutcome`, `ProviderDiagnostic`, `Tiered<T>`, `TieredOutcome<T>`, `provenance_class()` S2→Extracted/S1→Inferred/S0→Ambiguous.
- [x] 1.2 Additive `TieredCodeIntelligenceProvider` (6 ops); trait/consumers untouched.
- [x] 1.3 Tests; `cargo test -p cognicode-core --lib code_intelligence`; `cargo check -p cognicode-core` ± features.

## Phase 2: Composite (WU2) — RED-first

- [x] 2.1 RED `composite.rs`: gated/erroring S2 falls through declaring serving tier ("Gated or failed tier falls through").
- [x] 2.2 Fixed `[S2,S1,S0]` matrix; `TierPolicy`/`with_policy`; S2 readiness-gated; definition `Ok(None)` authoritative.
- [x] 2.3 Per-tier atomics, `status()->CompositeStatus`; `warn!`/`debug!`.
- [x] 2.4 Map `Unresolved`: definition/hover→`Ok(None)`, rest→`Err(Internal)`; delete `FallbackResult`+2 tests; hierarchy S2-first.
- [x] 2.5 Test "Fallback diagnostic is attached and counted"; `cargo test -p cognicode-core --lib composite`.

## Phase 3: Fact-Bridge Mapping (WU3, gated) — RED-first

- [x] 3.1 RED (gated) `batch_builder.rs`: S0 fact Ambiguous, never Extracted.
- [x] 3.2 RED (gated): class-vs-tier contradiction fails batch (`FactBridgeError::TierProvenanceContradiction`).
- [x] 3.3 GREEN: tiered `add_provider`/`collect`; `RelationRecord`+tier/provider_id; `finish()` class+contradiction; detail `tier=<T> provider=<id>`.
- [x] 3.4 `UnresolvedRecord{site,query,exhausted_tiers}`+`take_unresolved()`; `lsp_facts` exhaustion records (silent continues :116,167,205); extraction stays Extracted; update mocks+`JoinObserver`.
- [x] 3.5 Tests "Tier decides provenance class", "Ambiguous heuristic match stays Ambiguous", "Unresolved site propagates without a fact"; `cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel`.

## Phase 4: Fixtures+Runner+CP-5 (WU4)

- [x] 4.1 Create `sandbox/fixtures/lsi-providers/{rust,ts,java}/` + `expected.json` (tier/outcome/diagnostics per query).
- [x] 4.2 Create `tests/provider_conformance.rs` (ungated): Rust/TS `with_policy(lsp=off)` serverless S0/S1.
- [x] 4.3 RED: flipped Rust declaration fails naming query/declared/observed ("Contradicting tier fails the run"); restore.
- [x] 4.4 Java `jdtls` PATH probe (no spawn): available→S2; unavailable→"Java without its server degrades by declaration"; "Matching declarations pass without servers".
- [x] 4.5 Create `tests/cp5_tie_break.rs` (`#![cfg(feature = "evidence-kernel")]`): batch sort, SubjectIndex tie-break, continuity first-defines agree.
- [x] 4.6 `lsi-providers` recipe (runner+gated filters); green both Java branches.

## Phase 5: Guards + Handoff (WU5)

- [x] 5.1 `just lsi-fixtures check` 42/42; identity+equivalence digests unchanged.
- [x] 5.2 `cargo check` core/explorer/runtime ± features; `just lint`+fmt.
- [x] 5.3 Update `.agent/TESTING-STATE.md` (Active Change+handoff).
