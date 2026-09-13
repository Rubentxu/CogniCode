# Design: E39 LSI Semantic Provider Pipeline (M4)

> Delivery strategy: **auto-chain** — chained work-unit slices WU-1..WU-5, stacked-to-main, commit per completed+verified cycle (e38.1/e38.2 pattern). Resolved proposal assumptions are implemented, not re-opened: A1 counters internal+tracing (no new MCP/CLI output; the MAY-surface into existing `lsp_handlers` fields is deliberately not exercised — response shapes and goldens stay stable); A2 declaration-only precision targets; A3 jdtls optional (declared-unavailable is a valid fixture scenario); A4 S0/S1 heuristic mis-binds classed Ambiguous are accepted known-limit. `CodeIntelligenceProvider` keeps its 6 ops; `CodeIntelligenceProvider`-typed consumers (`workspace_session.rs:151`, `lsp_handlers.rs:12`, `commands.rs:1010+`, `lsp_proxy_service.rs:99`, `cognicode-core-mock`) need zero signature ripple.

## Technical Approach

Exploration Option 1 + thin Option-3 slice. The composite becomes an ordered, policy-gated tier pipeline whose results declare serving tier + structured diagnostics (`provider-tier-provenance` reqs 1–2); `lsp_facts` maps tier→`Provenance` class and appends `tier=<T> provider=<id>` detail entries (req 3); heuristic facts never claim Extracted and per-query exhaustion records uncertainty instead of facts (req 4). Conformance fixtures declare tier+outcome per query (`provider-pipeline-conformance`). SCIP: no adapter — spike criteria recorded in Open Questions.

## Architecture Decisions

| # | Decision | Alternatives | Rationale |
|---|----------|--------------|-----------|
| D1 | Tier types live in `domain/traits/code_intelligence.rs` (proposal-pinned): `PrecisionTier{S0,S1,S2,S3,S4}` (S3 SCIP/S4 compiler-IR declared-reserved, no M4 producer; Display/FromStr `"S0"`..; **no serde** — never persisted), `ProviderOutcome{Unavailable,Error,Degraded}`, `ProviderDiagnostic{provider, attempted_tier, outcome, message}`, `Tiered<T>{value, tier, diagnostics}`, `TieredOutcome<T>{Served(Tiered<T>), Unresolved(Vec<ProviderDiagnostic>)}`; `const fn PrecisionTier::provenance_class()` = S2→Extracted, S1→Inferred, S0→Ambiguous (S3/S4→Extracted, doc-marked unreachable in M4; `docs_confidence_rules.rs:60` precedent). New ADDITIVE trait `TieredCodeIntelligenceProvider` (6 tiered ops) in the same file; existing trait untouched. | Tiered results only on a wrapper type (exploration option 2); serde on tier enums | Spec: diagnostics attached to results, not side-channels — guessing tier at the adapter violates "precision is explicit"; `&dyn` trait keeps `lsp_facts` testable; no serde = no bincode surface whatsoever (tier crosses persistence ONLY as detail strings) |
| D2 | `CompositeProvider` refactored to a fixed highest-first tier list `[S2, S1, S0]` with per-op support matrix + per-op none-fall-through policy pinned as consts: S2 = `LspIntelligenceProvider` (attempt always readiness-gated, bounded by `wait_timeout_secs`); S1 = the LightweightIndex-backed definition resolution (fallback.rs:156–193 — the local resolver); S0 = all other `TreesitterFallbackProvider` parse ops (get_symbols/find_references-occurrences/hover/document_symbols). `get_definition` LSP `Ok(None)` stays authoritative-unresolved (no fall-through, composite.rs:193 precedent); hover/symbols/references empty falls through (today's behavior). New ctor `with_policy(workspace_root, TierPolicy)` (per-tier enable); existing ctors default all-on. Old-trait impl delegates to the pipeline and maps `Unresolved` to today's shapes per op (get_definition/hover→`Ok(None)`; get_symbols/find_references/get_hierarchy→`Err(Internal(<diagnostic summary>))`). Dead `FallbackResult` deleted with its 2 tests. Named behavior delta: `get_hierarchy` now attempts S2 first (op exists at lsp.rs:140) where it hard-coded the fallback (composite.rs:171); `test_composite_hierarchy_always_uses_fallback` updates accordingly. | Splitting `TreesitterFallbackProvider` into two provider types; feature-gating the pipeline (see D7) | Per-op attribution in one place avoids new types and keeps the two concrete providers intact; splitting duplicates the index wiring for one op; per-op policy preserves today's serving semantics exactly except the hierarchy delta the spec's fixed-order rule demands |
| D3 | Counters: lock-free atomics per (tier × attempts/served), `pub fn status(&self) -> CompositeStatus{tiers: Vec<TierCounters{tier, attempts, served, unavailable, errors, degraded}>}`. Tracing: each fall-through/degraded attempt emits structured `warn!`/`debug!` fields (op, tier, provider, outcome). Consumers in M4: composite unit tests only — the conformance runner asserts declared tiers/outcomes, not counters (verify S1); `workspace_session`/`lsp_handlers`/CLI untouched. | Exposing counters via new MCP tool/CLI flag; new response fields | Resolved assumption A1; response-shape changes risk MCP contract tests and goldens for zero M4 gate value |
| D4 | `lsp_facts::collect` + `FactBatchBuilder::add_provider` switch to `&dyn TieredCodeIntelligenceProvider` (only in-crate test mocks ripple: 4 in lsp_facts.rs + `JoinObserver` batch_builder.rs:495). `RelationRecord` gains `tier: Option<PrecisionTier>` + `provider_id`; `finish()` derives `class = tier.map(provenance_class).unwrap_or(Extracted)` — fixing today's all-Extracted stamping (batch_builder.rs:155); extraction facts keep `tier=None`/Extracted (AST-observed, correct). Contradiction check at observation admission (verify W1): a record whose class and tier disagree is rejected by `add_tiered_observation` (returns `Err(FactBridgeError::TierProvenanceContradiction)`, non-persisted error enum — safe to extend); `finish()` derives the class from the pinned tier map. Detail entries appended after existing `ref=<Kind>`: `tier=<T> provider=<id>` (deterministic order). | Deriving class in lsp_facts only (no builder check); adding `analysis:unresolved` predicate | Spec pins the contradiction failure at batch construction; a 7th predicate violates the pinned-six registry (bootstrap.rs:16–23); extraction facts must stay Extracted or the e37 equivalence multiset comparison breaks |
| D5 | Unresolved propagation: `FactBatchBuilder` gains `unresolved: Vec<UnresolvedRecord{site, query, exhausted_tiers}>` + `pub fn take_unresolved(&mut self)` (before `finish()`; `finish` signature unchanged). `lsp_facts` records exhaustion per query (site = queried symbol's fact-side FQN; file path for `get_symbols`) instead of silent `Err(_) => return/continue` (lsp_facts.rs:116,167,205). No fact, no fabricated subject/target, no persisted artifact (in-memory conformance surface + tracing). `get_definition` remains unconsumed by lsp_facts (honest no-op, e38 deviation precedent) — pipeline supports it; conformance covers it at composite level. | New kernel uncertainty artifact; persisting unresolved rows | Gate 4 needs uncertainty recorded, not persisted; kernel has no uncertainty type and adding one (or a predicate) violates the bincode/pinned-six rules |
| D6 | Ungated landing (D7 rationale); fixtures: `sandbox/fixtures/lsi-providers/{rust,ts,java}/` (source files + `expected.json`, lsi-identity `expected-mapping.json` pattern: `schema_version, language, queries[{op, file, line, column, expected:{tier, outcome: Served\|Unresolved, diagnostics?}}]`). Runner `crates/cognicode-core/tests/provider_conformance.rs` (ungated, composite-level): Rust/TS run `with_policy(lsp=off)` → **no server spawn, serverless-deterministic**, declared S0/S1 tiers verified unconditionally; Java probes the `jdtls` binary name (tree_sitter_parser.rs:365) via PATH scan — no spawn: available→full S2-declared run; unavailable→declared-unavailable branch (S2 attempted with 5s fixture timeout, `Unavailable` diagnostic asserted, no S2-tier result). Tier contradiction fails naming query/declared/observed. Just recipe `lsi-providers` (fixed argv: runner + gated `fact_bridge` unit filters). | Feature-gating the runner; spawning servers to probe | Spec REQ availability-gating: declared-unavailable is the fixture scenario; PATH probe avoids CI flakiness (proposal risk 2) |
| D7 | **Ungated** pipeline. The composite is core LSP surface; gating behind `evidence-kernel` would force a second, non-tiered composite for `workspace_session.rs:151` — a fork of the default path. Default-path stability is protected instead: legacy extraction path untouched → `just lsi-fixtures check` 42/42 byte-identical; MCP/CLI responses unchanged (D3); e37 `PINNED_IDENTITY_DIGEST` / e38 `PINNED_MATCHER_DIGEST` are convention-text hashes (harness.rs:93–108) and multiset comparisons — detail-string additions don't move them. | Gating behind evidence-kernel | Behavior-preserving refactor + diagnostic-only additions are safe ungated; the fact-bridge tier mapping (D4/D5) stays behind the existing gate |
| D8 | CP-5 rider lands as `crates/cognicode-core/tests/cp5_tie_break.rs` (file-level `#![cfg(feature = "evidence-kernel")]`, e38 `workspace_isolation.rs` precedent): same-name symbols across files must agree across the three layers — batch_builder canonical sort (first `core:defines` = lexicographically smallest FQN), `SubjectIndex` tie-break (lsp_facts.rs:50–51), and continuity view first-defines recovery; any disagreement fails. | Folding into the runner file (mixed gating); deferring CP-5 | M4 touches exactly the lsp_facts join surface (exploration Q8); separate gated file keeps the ungated runner clean |

## Data Flow

```
query (hover/get_definition/…) ─► CompositeProvider
      │ policy gate (TierPolicy)          gated-off tier: skipped, no diagnostic
      ▼
[S2 LSP]──not ready/unavailable/err──► ProviderDiagnostic{S2,…} ──fall-through──┐
   │ served value                                                               │
   ▼                                                                            ▼
TieredOutcome::Served{value, tier, diagnostics}            [S1 index resolver] → [S0 tree-sitter]
      │                                                                       │ all exhausted
      │                                                                       ▼
      │                                                  TieredOutcome::Unresolved(diagnostics)
      ▼                                                                           │
old-trait mapping (MCP/CLI/…) ────────────────────────────────────────────────────┤
lsp_facts: class=provenance_class(tier), detail += "tier=<T> provider=<id>"        │
           → FactBatchBuilder (contradiction check in finish)                      ▼
                                                       UnresolvedRecord{site, query, exhausted} (no fact)
```

```mermaid
sequenceDiagram
    participant C as Caller (MCP/CLI/harness)
    participant P as CompositeProvider
    participant T as Tier pipeline [S2,S1,S0]
    participant B as FactBatchBuilder
    C->>P: query (op, location)
    P->>T: attempt highest eligible tier
    T-->>P: served → Tiered{value, tier, diagnostics} / Unresolved(diagnostics)
    P->>P: counters (atomics) + structured tracing
    P-->>C: old-trait shape (unchanged) or TieredOutcome (tiered trait)
    C->>B: add_provider (tiered trait)
    B->>B: class = provenance_class(tier); detail += tier=<T> provider=<id>
    B->>B: finish(): contradiction check → Fact ; take_unresolved() → UnresolvedRecords
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/traits/code_intelligence.rs` | Modify | D1 types + additive `TieredCodeIntelligenceProvider` + `provenance_class` |
| `crates/cognicode-core/src/infrastructure/lsp/providers/composite.rs` | Modify | D2 pipeline, `TierPolicy`, `with_policy`, status atomics, old-trait mapping, delete `FallbackResult` |
| `crates/cognicode-core/src/infrastructure/lsp/providers/{fallback,lsp}.rs` | Modify | Provider id consts + doc'd tier roles only (no signature changes) |
| `crates/cognicode-core/src/application/fact_bridge/batch_builder.rs` | Modify | D4 tier on records/contradiction check; D5 `UnresolvedRecord` + `take_unresolved`; `add_provider` tiered |
| `crates/cognicode-core/src/application/fact_bridge/lsp_facts.rs` | Modify | D4 tier→class + detail entries; D5 exhaustion records; test mocks → tiered trait |
| `crates/cognicode-core/src/application/fact_bridge/mod.rs` | Modify | `FactBridgeError::TierProvenanceContradiction` |
| `sandbox/fixtures/lsi-providers/{rust,ts,java}/` | Create | Sources + `expected.json` manifests (D6) |
| `crates/cognicode-core/tests/provider_conformance.rs` | Create | Conformance runner (ungated) |
| `crates/cognicode-core/tests/cp5_tie_break.rs` | Create | CP-5 cross-layer tie-break (gated) |
| `justfile` | Modify | `lsi-providers` recipe |

Interfaces / Contracts:

```rust
// domain/traits/code_intelligence.rs — ADDITIVE (existing trait untouched)
pub enum PrecisionTier { S0, S1, S2, S3, S4 }            // Display/FromStr; NOT persisted
impl PrecisionTier { pub const fn provenance_class(self) -> Provenance; }
pub enum ProviderOutcome { Unavailable, Error, Degraded }
pub struct ProviderDiagnostic { pub provider: String, pub attempted_tier: PrecisionTier,
    pub outcome: ProviderOutcome, pub message: String }
pub struct Tiered<T> { pub value: T, pub tier: PrecisionTier, pub diagnostics: Vec<ProviderDiagnostic> }
pub enum TieredOutcome<T> { Served(Tiered<T>), Unresolved(Vec<ProviderDiagnostic>) }
#[async_trait] pub trait TieredCodeIntelligenceProvider: Send + Sync { /* 6 ops → TieredOutcome<T> */ }

// composite.rs — pipeline surface
pub struct TierPolicy { pub lsp: bool, pub local_resolver: bool, pub tree_sitter: bool }
impl CompositeProvider { pub fn with_policy(root: &Path, policy: TierPolicy) -> Self;
    pub fn status(&self) -> CompositeStatus; }

// batch_builder.rs (cfg evidence-kernel)
pub async fn add_provider(&mut self, p: &dyn TieredCodeIntelligenceProvider, files: &[PathBuf]);
pub struct UnresolvedRecord { pub site: String, pub query: String, pub exhausted_tiers: Vec<PrecisionTier> }
impl FactBatchBuilder { pub fn take_unresolved(&mut self) -> Vec<UnresolvedRecord>; }
```

## Testing Strategy

| Layer | What | Command |
|-------|------|---------|
| Unit domain | Display/FromStr; `provenance_class` mapping; tiered-trait defaults | `cargo test -p cognicode-core --lib code_intelligence` |
| Unit composite | Fall-through + no false attribution; policy gating; per-op none semantics (definition-None authoritative); counters/status; old-trait Unresolved mapping; hierarchy S2 delta | `cargo test -p cognicode-core --lib composite` |
| Unit bridge | tier→class per fact; `ref=` + `tier=` + `provider=` detail order; contradiction fails batch; exhaustion → UnresolvedRecord + no fact; determinism; detail-updated assertions (lsp_facts.rs:378/383/421) | `cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel` |
| Integration | Runner: Rust/TS serverless declared tiers pass; Java both branches; contradiction names query/declared/observed; CP-5 tie-break | `just lsi-providers` |
| Guard | Goldens 42/42; e37/e38 digests + scores unchanged; fmt; clippy zero delta | `just lsi-fixtures check`, `just lsi-equivalence`, `just lsi-identity`, `just lint` |

## Threat Matrix

| Boundary | Applicability | Design response | Planned RED tests |
|---|---|---|---|
| Shell/subprocess | Applicable (limited) | `just lsi-providers` fixed argv (e37 D8 class); Java LSP attempt reuses the pre-existing `LspProcessManager` spawn (bounded `wait_timeout_secs`, fixture 5s); jdtls probe is a PATH scan — no spawn; no user-supplied paths (in-repo fixture roots) | jdtls absent → `Unavailable` diagnostic, suite passes; no other new process boundary exists |
| Routing, git selection, commit/push state, PR commands, executable-file classification | N/A | No VCS/PR automation, routing change, or executable classification | — |

## Migration / Rollback

No data migration (tier identity crosses persistence as detail strings only; no enum variants, no predicate, no schema change). Rollback per WU table: each slice is one revert; restoring the two-tier composite from history restores default behavior exactly; new domain types stay dormant; fixtures/tests are additive.

## Work-Unit Slices (auto-chain)

| WU | Scope | Finish / Verify | Rollback |
|----|-------|-----------------|----------|
| 1 Domain types | D1: types + `provenance_class` + additive tiered trait + unit tests | lib `code_intelligence` tests green; `cargo check` both feature states | revert the file |
| 2 Composite pipeline | D2/D3: tier list, policy, matrix, status atomics, tracing, old-trait mapping, `FallbackResult` removal, hierarchy delta + tests | composite lib tests green; `cargo check -p cognicode-explorer -p cognicode-runtime`; clippy zero delta | revert slice commit (two-tier composite from history) |
| 3 Fact-bridge mapping | D4/D5: tiered `add_provider`, class derivation + contradiction, detail entries, unresolved records, mock updates | gated `fact_bridge` tests green; scoped clippy clean | revert batch_builder/lsp_facts/mod edits |
| 4 Fixtures + runner + CP-5 | D6/D8: fixtures, `provider_conformance.rs`, `cp5_tie_break.rs`, `just lsi-providers` | `just lsi-providers` green (Rust/TS serverless; Java gated; contradiction path exercised) | delete fixtures + 2 test files + recipe |
| 5 Guards + handoff | `just lsi-fixtures check` (42/42), `just lsi-equivalence`, `just lsi-identity` (digests unchanged), fmt/lint, `.agent/TESTING-STATE.md` handoff | all green; handoff recorded | n/a (verification-only) |

## Open Questions

- [ ] SCIP spike criteria (S3, M5+, no M4 work): justified only when a golden/harness path needs index-grade identity that S2 cannot serve AND a per-language index producer exists in-repo; until then S3/S4 stay reserved variants.
- [ ] `get_hierarchy` S2 attempt may surface server-side capability gaps (e.g. servers without typeHierarchy) as `Error` diagnostics in tracing — accepted noise; tighten to per-server capability policy only if it pollutes counters materially.
