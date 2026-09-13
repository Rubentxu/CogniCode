## Exploration: e39-lsi-semantic-providers (LSI M4 — Semantic Provider Pipeline)

### Current State

**Umbrella contract** (`openspec/changes/cognicode-living-software-intelligence/specs/semantic-provider-pipeline/spec.md`, 2 requirements / 2 scenarios, read completely):
- *Provider precision is explicit*: every observation records provider id/version + precision tier; scenario "Fallback is visible" (lower-tier result carries fallback diagnostics + declared reduced precision).
- *Unknown beats fabricated resolution*: provider SHALL return unresolved/ambiguous rather than fabricate a target; scenario "Unresolved call" (no precise target Fact emitted, uncertainty recorded).

Design doc (`docs/CogniCode_Living_Software_Intelligence/docs/architecture/SEMANTIC-PROVIDERS.md`): tiers S0 tree-sitter / S1 local resolver / S2 LSP / S3 SCIP / S4 compiler IR; target output struct `SemanticObservation { claim, precision, provenance, diagnostics }`; fallback semantics list (register failure → policy-gated fallback → reduce confidence → propagate `knowledge_uncertainty` → escalate validation). Cohorts: Rust, TS/JS, Java.

**Provider stack today** — de facto tiers S0/S1-lite/S2; S3/S4 absent:
- `CodeIntelligenceProvider` trait — `crates/cognicode-core/src/domain/traits/code_intelligence.rs:13` (6 ops: get_symbols, find_references, get_hierarchy, get_definition, get_document_symbols, hover). Results carry NO tier, confidence, or diagnostics. `#![allow(dead_code)]` at :5 (e30.1 baseline).
- `CompositeProvider` — `crates/cognicode-core/src/infrastructure/lsp/providers/composite.rs:41`: hardcoded two-tier LSP→tree-sitter per method, `warn!`-only observability (composite.rs:126,142,159,182,209,223). `FallbackResult<T>` (composite.rs:16) exists but is dead scaffolding — never returned by the impl. `get_hierarchy` never tries LSP (composite.rs:171); `get_definition` passes LSP `Ok(None)` through WITHOUT fallback (composite.rs:193-194) — unknown stays unknown there.
- `LspIntelligenceProvider` (S2) — `providers/lsp.rs:11`, JSON-RPC via `LspProcessManager` (`lsp/process_manager.rs`, ServerStatus incl. `Crashed`). Server binaries per language: `tree_sitter_parser.rs:359-366` — rust-analyzer, pyright, typescript-language-server, gopls, **jdtls**, clangd.
- `TreesitterFallbackProvider` (S0 + S1-lite) — `providers/fallback.rs:12`: `LightweightIndex`-based definition heuristics (line-content matching `fn X(` / `struct X` …, fallback.rs:177-181,207-211 — can mis-resolve, never invents an entity); hover declares itself `HoverKind::Snippet` "(type: unknown — tree-sitter fallback)" (fallback.rs:266-270); hierarchy unsupported → Err (fallback.rs:128).
- Wiring: `application/workspace_session.rs:112,151,544-547`, MCP `handlers/mod.rs:358,617`, CLI `commands.rs:997-1071`. No SCIP/LSIF dependency anywhere in the workspace (only doc reference `docs/.../research/REFERENCES.md:20`).

**Fact seam** — `application/fact_bridge/` (feature `evidence-kernel`, OFF by default; `Cargo.toml:35`; not wired to runtime, exercised by tests/harnesses):
- `lsp_facts.rs` consumes `&dyn CodeIntelligenceProvider` (e37 D1): ReferenceKind→`core:calls/imports/references`, hierarchy→`core:inherits`; producer `RuntimeObserver` (lsp_facts.rs:27). e38.1 CP-3 `SubjectIndex` normalizes subjects to 1-based fact-side FQNs; unresolvable container → reference-site FILE PATH, a documented non-joinable form (lsp_facts.rs:140-152) — never an invented FQN.
- **The M4 gap is exactly here**: degradation is silent — `Err(_) => continue/return` (lsp_facts.rs:121,160,208) and the adapter cannot tell WHICH tier produced an observation (composite swallowed the fallback). `FactBatchBuilder::add_observation` already carries `detail: Option<String>` (batch_builder.rs:92-99, used for `ref=<Kind>`) — an existing channel for tier/diagnostic detail.

**Kernel fit (Q3/Q4)**:
- `EvidenceGrade` = Supports/Refutes/Corroborates (`evidence_kernel/evidence.rs:16-23`) — a *grade*, not a precision scale; consumers today: kernel + in-memory store only (e36 unconsumed status stands). It is NOT the natural home for tiers; Evidence entries CAN record fallback corroboration/refutation, but tier belongs in provenance.
- `ProvenanceRecord { class, producer, detail }` (`evidence_kernel/fact.rs:76-83`); legacy `Provenance` = Extracted / Inferred (heuristic 0.5-0.9) / Ambiguous (≤0.5) / Manual / Tested (`value_objects/provenance.rs:37-50`) — a ready-made tier→class mapping (S2/S3→Extracted, S1→Inferred, S0 heuristic/unresolved→Ambiguous). Precedent: `ConfidenceTier→Provenance` in `infrastructure/extraction/docs_confidence_rules.rs:60-62`, `issues_confidence_rules.rs:72-74`.
- Producer rule: bincode-format-sensitive — NO new `ProducerKind` variant; providers stay `RuntimeObserver` + richer `detail` (e38.1 precedent). Umbrella design "Data authority" table: LSP/compiler adapter may write Fact AND Evidence.
- Predicates are a pinned six (`evidence_kernel/bootstrap.rs:16-23`); an `analysis:unresolved` predicate would need schema-registry extension — but the spec scenario demands NOT emitting a fact at all, so unresolved state lives in provider diagnostics + uncertainty records, not a new predicate.

**Fixtures (Q5)**: golden-fixture pattern exists at `sandbox/fixtures/` (lsi-baseline with `goldens/`, `inventory.json`, `coverage.json`); Rust (rust-hello/indexing/callgraph/refactor/multifile), TS (ts-hello/analysis/indexing), Java (java-hello, java-sample) all present; `multi-lang-types` quarantined (pending GAP S2). Grammars for rust/ts/js/java in core `Cargo.toml:47-51`. Real-LSP Java (jdtls) is heavyweight — fixtures must be runnable without live servers (S0/S1 targets always; S2 targets where the server is available, else the declared-unavailable path is itself a fixture scenario).

**SCIP (Q6)**: zero tooling/deps in-repo. Task 5.4 says "only where spikes justify" — nothing on any golden/harness path needs S3 today; the two-tier pipeline + declared tiers satisfies the spec. Recommend: no SCIP adapter in M4; record spike-criteria (exact identity needs, available index producers per language) in design.md.

**Riders (Q8)**:
- CP-5 (cross-layer tie-break test on the lexicographically-smallest-FQN rule): FITS — M4 touches exactly the lsp_facts join surface; small, natural addition.
- DUP-2/3/6 (shared test-support extraction): only if M4's new provider tests would otherwise duplicate mocks (a `JoinObserver`-style shared mock already exists in batch_builder.rs:495 area); otherwise defer — not provider-specific.
- GAP S2 (generic-projection equivalence): projections, not providers — keep on the ledger for a separate cycle; keep `multi-lang-types` quarantined.

### Affected Areas
- `crates/cognicode-core/src/domain/traits/code_intelligence.rs` — tier/diagnostic-aware result types (or parallel observation type); trait is `allow(dead_code)`, so extension is low-blast.
- `crates/cognicode-core/src/infrastructure/lsp/providers/{composite,fallback,lsp}.rs` — pipeline refactor; composite is the 5.2 target; `FallbackResult` dead code gets realized or removed.
- `crates/cognicode-core/src/application/fact_bridge/{lsp_facts,batch_builder,mod}.rs` — tier/diagnostics flow into `detail` + per-file degradation records.
- `crates/cognicode-core/src/domain/evidence_kernel/fact.rs` + `value_objects/provenance.rs` — REUSE only (no new variants; bincode rule).
- `sandbox/fixtures/` + `crates/cognicode-core/tests/` — per-language semantic conformance fixtures with precision targets.
- `application/workspace_session.rs`, `interface/mcp/handlers/lsp_handlers.rs` — status/diagnostics surfacing (exit gate 1).

### Approaches

1. **Domain pipeline port (recommended)** — introduce a tiered `SemanticObservation`-shaped response in the domain (per doc: claim + PrecisionTier + provenance detail + diagnostics), refactor `CompositeProvider` behind it (5.2), plumb tier/diagnostics through `add_provider` into fact `detail` (5.1/5.3), expose a provider-status/diagnostics query (MCP/CLI) + structured tracing for fallback observability (gate 1).
   - Pros: matches umbrella spec and hexagonal rules (traits stay domain, no I/O); realizes dead `FallbackResult`; adapter finally sees the producing tier; single seam for later S3.
   - Cons: touches trait surface consumed by MCP handlers, CLI, workspace_session (compile-bounded; mock updates).
   - Effort: Medium.
2. **Incremental wrapper** — keep `CodeIntelligenceProvider` as-is; wrap composite with a side-channel (mpsc/tracing layer or a `ProviderDiagnostics` observer trait) recording fallback events; stamp tiers only at the fact_bridge layer by querying which tier is "likely".
   - Pros: zero trait churn; fastest.
   - Cons: "likely" tier at adapter level = guessing — violates "precision is explicit"; side-channel drifts from results; diagnostics not attached to observations.
   - Effort: Low, but contract-dishonest — rejected.
3. **Full pipeline + Evidence-first semantics** — option 1 plus emitting kernel Evidence per fallback (Supports/Refutes) and unresolved-uncertainty records into the committed batch.
   - Pros: first real Evidence consumer (e36 debt); strongest gate-1/exit-gate story.
   - Cons: fact bridge is feature-gated and NOT runtime-wired — Evidence emission lands only in harness/kernel tests this cycle; risk of over-building before the Intelligence Event Log (M7) defines event consumers.
   - Effort: Medium-High.

### Recommendation
Option 1, scoped with a thin slice of 3: implement PrecisionTier + ProviderDiagnostic in the domain; refactor CompositeProvider into an ordered, policy-gated tier pipeline where every response declares tier + diagnostics and unresolved is a first-class outcome (no fabricated targets — LSP `Ok(None)` semantics generalized, fallback heuristics marked S1/S0-Ambiguous); surface fallback via (a) diagnostics attached to results, (b) per-tier counters/status accessor on the composite exposed through existing MCP/CLI seams, (c) `tracing` — the Intelligence Event Log stays M7. In fact_bridge, map tier→Provenance class and append `tier=<T>`/fallback diagnostics to `detail`; emit Evidence only where the kernel harness already exercises batches (do not force a runtime consumer yet). SCIP: no adapter; write spike-justification criteria. Fixtures: per-language conformance fixtures under `sandbox/fixtures` with declared precision targets per tier (S0/S1 always; S2 gated on server availability — unavailability itself is a fixture scenario). Riders: take CP-5; take DUP-2/3/6 only if the new test surface needs the shared mock; leave GAP S2 on the ledger.

### Risks
- Trait-surface change ripples to MCP handlers/CLI/mocks (`lsp_handlers.rs`, `commands.rs`, `cognicode-core-mock/src/lib.rs`) — bounded but broad; keep the existing 6 methods intact and add the observation type alongside, or adapt via new trait methods with default impls.
- Fallback heuristics in `TreesitterFallbackProvider::get_definition` can mis-resolve (substring/content matching across files) — must be classed Ambiguous/Inferred, never Extracted, or "no fabricated resolutions" gate is technically violated by mis-join risk.
- Java (jdtls) availability in CI: live-LSP Java fixtures are flaky-prone; precision targets must degrade to declared-unavailable scenarios without failing the suite.
- 400-line review budget: trait + composite + lsp_facts + fixtures will exceed one PR — plan chained work-unit slices (delivery_strategy: auto-chain already set).
- Bincode sensitivity: any temptation to add enum variants (ProducerKind, EvidenceGrade) is a cardinal sin here — reuse `detail`.

### Ready for Proposal
Yes — sdd-propose should scope: 5.1 PrecisionTier/ProviderDiagnostic domain types; 5.2 composite pipeline refactor; 5.3 fallback semantics (diagnostics + tier reduction + unresolved propagation, tracing/status observability); 5.4 spike-criteria only; 5.5 Rust/TS/Java fixtures with precision targets; riders CP-5 (+DUP conditionally).
