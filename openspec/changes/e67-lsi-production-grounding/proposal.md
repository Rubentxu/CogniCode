# e67 Proposal — LSI Production Grounding Activation (Rust, bounded vertical slice)

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e67-lsi-production-grounding` |
| Phase | propose |
| Path | a-lite |
| Base SHA | `a48674d9` (post-e66 amendment; e66 admin closure still deferred via DEBT-SDDK-002) |
| Branch | `feat/e66-lsi-m7-5-read-sets` (continues; new commits only) |
| Author | jcode-orchestrator (direct execution — provider fallback chain MiniMax→Anthropic 404, see Risks §3) |
| Date | 2026-09-16 |

## Goal

**Connect one real production producer (Rust source via tree-sitter) to the canonical
`FactBatch → FactStore → grounded analysis → Evidence → Finding` chain so that an
M6 Finding generated from a real Rust file can be verified and gated.**

If this cycle lands, all M6/M7 infrastructure (evidence kernel, canonical grounding,
behavior authority, budgets, read-sets) becomes product-visible: a Finding that
arises from real source code now resolves through Evidence, can be linked back to
its originating Fact, and can therefore be allowed to gate a CI decision — whereas
the same Finding against ungrounded synthetic data must be refused at the gate.

This is the **single vertical slice** that closes the gap between "we can verify
grounded Findings" and "we have a grounded Finding to verify".

## Why this cycle now

M6 (canonical grounding, e55) through M7.5 (read-sets, e66) built the substrate.
The README/umbrella change states explicitly: **no production producer is currently
emitting canonical grounded Facts**. Real runs in the workspace remain ungrounded.

This means e66's M8 invalidation works on a corpus that is, in practice, still
fixture-grade at the producer boundary. e67 is therefore the **first product-visible
vertical slice** of the program.

## Scope (this cycle — bounded)

### In scope

1. **Composition wiring** in `crates/cognicode-runtime` so that a Rust source path
   flows: `extract_file` → `tree_sitter_facts::collect` → `FactBatchBuilder::finish()`
   → `FactStore::commit` → snapshot N.
2. **One existing detector** exercised end-to-end on real Rust source, with
   `GroundingRef` populated by the canonical fact side, not by a hand-rolled
   stub. (Candidate detector: TBD in design phase — likely one that already
   consumes `core:defines` or `core:calls`.)
3. **Persistence** through the canonical `FactStore` port (Ladybug / in-memory
   oracle, per existing harness).
4. **Evidence emission** via `CanonicalEvidenceWriter` to make the resulting
   Finding resolvable.
5. **One UAT**: `UAT-U6X` (id assigned in spec phase) — same Rust fixture run
   twice, once via the new grounded pipeline and once bypassing it; the grounded
   run produces a Finding that `FindingVerifier` accepts and the `PolicyGate`
   admits; the ungrounded run produces a structurally identical Finding that the
   gate refuses.
6. **Causal/read lineage check** that from a Finding's `Evidence` we can navigate
   to the originating `FactId` (the read-set consumer surface from e66 is the
   natural seam).

### Out of scope (explicit non-goals)

| Excluded | Reason / future cycle |
|----------|----------------------|
| Other languages (TS/Java/Python) | e67 covers Rust only; multi-language producer framework is a later, separate decision |
| New crate `cognicode-source-rust` | e37 already declined this; reuse `fact_bridge::tree_sitter_facts::collect` |
| Producer in `cognicode-explorer` | Violates layering (explorer → core); runtime owns composition |
| LSP grounding (`lsp_facts` flow) | Different producer class; not in this slice |
| Ladybug durable optimization | Production tuning is post-grounding |
| Generic producer framework | No second consumer yet; deferred |
| CI scheduling, EvidenceBundle, `why_scheduled` | Those are e68/e69/e70 |
| Pack governance, architecture-constraint execution | M10 |
| Behavior authority runtime changes | Already accepted (ADR-044, e64); e67 is a producer-side change |
| Read-set runtime binding (only the type exists in e66) | Deferred to e68, which depends on a real grounded corpus |
| Tree-sitter grammar upgrades | Out of scope; we use whatever `extract_file` already produces |

## Architecture (target — refined in design phase)

```text
                          (cognicode-runtime composition root)
                                    │
   rust source path ───► extract_file()  ───┐
                                    │       │
                                    │       ▼
                                    │  fact_bridge::tree_sitter_facts::collect()
                                    │       │
                                    │       ▼
                                    │  FactBatchBuilder::finish()   (canonical D3)
                                    │       │
                                    │       ▼
                                    │  FactStore::commit_batch()   (port trait)
                                    │       │
                                    │       ▼
                                    │  Snapshot N  (existing snapshot view)
                                    │       │
                                    │       ▼
                                    │  projection used by detector X  (existing)
                                    │       │
                                    │       ▼
                                    │  DetectorExecutor::run()        (M6 path)
                                    │       │
                                    │       ▼
                                    │  CanonicalEvidenceWriter::write()
                                    │       │
                                    │       ▼
                                    │  Finding (with GroundingRef resolved via FactId)
                                    │       │
                                    │       ▼
                                    │  FindingVerifier::accept()  ──► yes ──► gateable
                                    │       │                         no  ──► refused
                                    │       ▼
                                    │  (read-set consumer surface e66 — ungrounded lineage rejected)
```

### Hard layering rules (enforced in design and apply phases)

| Layer | Allowed to import | Must NOT import |
|-------|-------------------|------------------|
| `cognicode-core::domain::*` | domain only | `tree-sitter`, `tokio`, `sqlx`, `ladybug`, I/O |
| `cognicode-core::application::fact_bridge` | domain, `application::ingest::types::ExtractionResult` | runtime, explorer |
| `cognicode-core::infrastructure::parser` | tree-sitter, application | domain |
| `cognicode-runtime` | everything (composition root) | (none) |
| `cognicode-explorer` | `core`, `runtime` (for the port surface) | tree-sitter, fact construction |

The `Explorer → constructing Fact directly` route is forbidden. The `Runtime →
knows tree-sitter nodes` route is forbidden. The `Domain → tree-sitter/tokio/Ladybug`
route is forbidden (already a review-enforced rule, AGENTS.md § "Architecture rules").

## Requirements (specify phase)

### REQ-GRD-001 — Real Rust ingest reaches the canonical FactStore

A real Rust source file (workspace fixture or small repo) MUST flow end-to-end
through `extract_file` → `tree_sitter_facts::collect` → `FactBatchBuilder::finish`
→ `FactStore::commit_batch`, producing a snapshot whose `EntityIdTable` is
non-empty and whose batch contains at least one `core:defines` fact for the
fixture's primary symbol.

**Scenario 1.1 (UAT-U6X.1 — production ingest):**
- GIVEN a Rust fixture file with at least one `fn` definition
- WHEN the runtime ingestion path runs on that file
- THEN `FactStore::latest_snapshot()` returns a snapshot where
  `batch.facts.iter().any(|f| predicate == core:defines && object == fqn_of_fixture_fn)` holds

### REQ-GRD-002 — Detector on real source produces a Finding with `GroundingRef::fact(...)`

The existing detector selected in design MUST consume the projection of the new
snapshot and emit a Finding whose causal reference resolves to a `FactId` in
that same snapshot (i.e. `GroundingRef::fact(FactId)` is `Some`, not the
ungrounded stub).

**Scenario 2.1 (UAT-U6X.2 — grounded finding):**
- GIVEN the snapshot produced by REQ-GRD-001
- WHEN the detector runs through the M6 normal path
- THEN the resulting Finding's `grounding` field is `GroundingRef::fact(some FactId)` where
  `some FactId` exists in the committed batch

### REQ-GRD-003 — Evidence emission for the grounded Finding is accepted

`CanonicalEvidenceWriter::write` for the Finding emitted in REQ-GRD-002 MUST
succeed and produce an Evidence record whose `fact_ref` matches the `FactId` in
the Finding's `GroundingRef`.

**Scenario 3.1 (UAT-U6X.3 — evidence chain):**
- GIVEN a Finding with `GroundingRef::fact(F)`
- WHEN `CanonicalEvidenceWriter::write(...)` is called
- THEN the persisted Evidence record's `fact_ref == F`
- AND `FindingVerifier::accept(...)` returns `Accept`

### REQ-GRD-004 — Causal/read lineage from Finding to Fact

From the persisted Evidence of REQ-GRD-003, the read-set consumer surface (e66)
MUST be able to enumerate the originating `FactId` (i.e. the lineage edge
`Evidence → Fact` exists).

**Scenario 4.1 (UAT-U6X.4 — lineage):**
- GIVEN the Evidence from REQ-GRD-003
- WHEN `ReadSet` enumeration traverses the lineage
- THEN the originating `FactId` is reachable without traversing an
  `UngroundedLineage` placeholder

### REQ-GRD-005 — Negative case: ungrounded Finding cannot gate

The same detector against a hand-crafted or stub-fed snapshot that bypasses
the canonical FactStore MUST emit a Finding that **structurally looks the same**
but `FindingVerifier::accept(...)` rejects OR `PolicyGate::evaluate(...)`
refuses to open.

**Scenario 5.1 (UAT-U6X.5 — ungrounded refusal):**
- GIVEN a synthetic Finding with `GroundingRef::Ungrounded` (or a fabricated
  FactId that does not resolve to a committed fact)
- WHEN `PolicyGate::evaluate(finding)` runs
- THEN the gate decision is `Refuse` (or equivalent "cannot gate" verdict)
- AND the refusal reason cites missing-grounding (not a generic test failure)

### REQ-GRD-006 — Minimum canonical vocabulary is bounded

The cycle's grounded predicates are limited to those actually exercised by the
selected detector plus one adjacent predicate (for lineage). The proposal MUST
NOT propose grounding the entire `core:*` ontology at once.

**Scenario 6.1:**
- GIVEN the list of predicates the detector uses
- THEN `e67-grnd-vocab.md` enumerates exactly those predicates + at most one
  auxiliary predicate (e.g. for `core:contains` lineage navigation)

## Work-unit decomposition

| WU | Description | Acceptance | Touches |
|----|-------------|------------|---------|
| WU1 | Production ingest path: runtime wiring + fact bridge to canonical FactStore, fixture-based unit test | `cargo test -p cognicode-core --lib production_grounding` passes; snapshot has `core:defines` for fixture fn | `crates/cognicode-runtime/src/lib.rs` (composition only), `crates/cognicode-core/src/application/fact_bridge/production_grounding.rs` (new, thin orchestration) |
| WU2 | Detector execution on real snapshot + `GroundingRef::fact(...)` propagation | `cargo test -p cognicode-core --lib grounded_detector` passes; Finding has `GroundingRef::fact(F)` with F in the snapshot | `crates/cognicode-core/src/application/findings/kernel_bridge.rs` (no behavior change), `crates/cognicode-core/src/application/fact_bridge/production_grounding.rs` (extended) |
| WU3 | Evidence emission + `FindingVerifier` accept on grounded case + `PolicyGate::Refuse` on ungrounded case (UAT-U6X) | UAT test (one): grounded → Accept + gateable; ungrounded → Refuse; CI test green | `crates/cognicode-core/src/application/findings/`, `crates/cognicode-core/src/domain/findings/` (verifier policy only, no protocol change) |

Single 3-WU decomposition. Each WU is independently revertable. The umbrella
keeps the next-vertical slice (e68 — Semantic Diff + Affected Work) blocked
behind this one's gate.

## Risks

1. **Detector selection risk.** Several detectors exist; some may already be
   grounded via fixtures and "just work" without proving anything new. The
   design phase MUST pick a detector that **currently cannot run end-to-end on
   real source** without e67's wiring, so the UAT proves genuine progress.

2. **`FactStore::commit_batch` API surface.** If the existing port has a
   different name (`commit`, `add_facts`, etc.) the design phase MUST verify
   it before writing WU1. Mitigation: read the port trait in design and use
   its real method; do not assume naming.

3. **Provider fallback executed in this cycle (NOT a code risk; an orchestration risk).**
   The propose phase was executed by the orchestrator directly because:
   - `minimax-coding-plan/MiniMax-M3` (prescribed model) had no `OPENROUTER_API_KEY`.
   - `minimax-coding-plan/MiniMax-M2.7-highspeed` (default session model) was
     rejected by the Anthropic provider when routed through Claude API.
   - `claude-opus-5` (first available Anthropic model) returned `404 Not Found`
     from the API.
   This means the orchestrator's **delegation contract is broken in this
   session** for both model routes. Mitigation: this is recorded here so the
   user knows the spec/design/apply phases should be re-attempted under a
   restored provider. Until then, only the orchestrator can write; **no
   `sddk-apply` worker should be spawned** until a provider succeeds.

4. **Tree-sitter fixture quality.** If the fixture is trivial (one `fn main() {}`)
   the UAT is weak. Mitigation: pick a fixture with at least one
   `mod`/struct/fn/call edge so `core:defines` and `core:calls` both populate.

5. **Read-set consumer surface is still type-only (e66).** REQ-GRD-004 tests
   lineage traversal but the read-set's full invalidation consumer (e68) does
   not yet exist. Mitigation: REQ-GRD-004 verifies the **read-set can be
   populated from a grounded Finding**, not that invalidation runs — that is
   e68's job.

6. **Existing `tree_sitter_facts::collect` already does the conversion.** If
   the design phase finds that everything in WU1 already exists in some form,
   the cycle degenerates to a UAT-only cycle (still legitimate — it proves the
   chain works on real source). Mitigation: in that case, the proposal reduces
   to a single-WU cycle focused on the UAT and composition wiring only.

## Decision

PROCEED to design phase with the 3-WU decomposition above, conditional on:

- Design phase verifying the actual `FactStore` port trait API (Risk 2).
- Design phase picking a detector that genuinely requires the wiring (Risk 1).
- Apply phase running under a restored model provider (Risk 3).

If Risk 6 materializes (chain already exists in code), collapse to a single-WU
UAT-only cycle in the design phase and proceed.

The next-vertical slice (e68 — Semantic Diff + Affected Work) MUST NOT be
opened until e67's UAT (UAT-U6X.5 ungrounded refusal) is observed passing.