# e67 Spec — LSI Production Grounding Activation (Rust)

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e67-lsi-production-grounding` |
| Phase | specify |
| Proposal | `proposal.md` |
| Path | a-lite |
| Date | 2026-09-16 |

This spec refines the requirements introduced in `proposal.md` into executable
Given/When/Then scenarios. The canonical facts flow into the existing M6/M7
kernel — the spec declares new behavior only at the **producer boundary** and
at the **gate boundary**.

---

## REQ-GRD-001 — Real Rust ingest reaches the canonical FactStore

A real Rust source file MUST flow end-to-end through `extract_file` →
`tree_sitter_facts::collect` → `FactBatchBuilder::finish` →
`FactStore::commit_batch` (or the actual port method discovered in design),
producing a snapshot whose `EntityIdTable` is non-empty and whose batch
contains at least one `core:defines` fact for the fixture's primary symbol.

### Scenario 1.1 — Production ingest (UAT-U67.1)

- **Given** a Rust fixture file `sandbox/fixtures/lsi-grounding/sample.rs`
  containing at least one `fn` definition with a known FQN
  (e.g. `sample::greet`)
- **When** the runtime ingestion path runs on that file (call surface defined
  in design: `cognicode_runtime::grounding::ingest_rust_source(path)`)
- **Then** the resulting snapshot has `facts.iter().any(|f| f.predicate == core:defines && f.object == "sample::greet")`
- **And** the snapshot's `EntityIdTable` is non-empty
- **And** the batch is byte-stable across two consecutive runs of the same
  fixture (deterministic replay)

### Scenario 1.2 — FactBatch predicate set is bounded

- **Given** the same fixture
- **When** the batch is finished
- **Then** every predicate present is one of: `core:defines`, `core:contains`,
  `core:calls`, `core:imports`, `core:references` (the canonical tree-sitter
  predicate set from `tree_sitter_facts::collect`)
- **And** no predicate outside this set is present (design D2 enforcement)

---

## REQ-GRD-002 — Detector on real source produces a Finding with `GroundingRef::fact(...)`

The existing detector selected in design MUST consume the projection of the new
snapshot and emit a Finding whose causal reference resolves to a `FactId` in
that same snapshot.

### Scenario 2.1 — Grounded Finding (UAT-U67.2)

- **Given** the snapshot from REQ-GRD-001
- **When** the detector runs through the M6 normal path (no detector code
  change)
- **Then** the resulting Finding's `grounding` field is
  `GroundingRef::fact(F)` where `F` is a `FactId` of the committed batch
- **And** `grounding != GroundingRef::Ungrounded`

---

## REQ-GRD-003 — Evidence emission for the grounded Finding is accepted

`CanonicalEvidenceWriter::write` for the Finding emitted in REQ-GRD-002 MUST
succeed and produce an Evidence record whose `fact_ref` matches the `FactId`
in the Finding's `GroundingRef`.

### Scenario 3.1 — Evidence chain (UAT-U67.3)

- **Given** a Finding with `GroundingRef::fact(F)`
- **When** `CanonicalEvidenceWriter::write(finding, &kernel_read_model)` runs
- **Then** the persisted Evidence record's `fact_ref == F`
- **And** `FindingVerifier::accept(finding)` returns `Accept`
- **And** the read-set consumer surface (e66 type) can be **populated** from
  the Evidence's `fact_ref` (full invalidation traversal is e68's job)

---

## REQ-GRD-004 — Causal/read lineage from Finding to Fact

From the persisted Evidence of REQ-GRD-003, the lineage edge
`Evidence → Fact` MUST exist and be navigable.

### Scenario 4.1 — Lineage navigation (UAT-U67.4)

- **Given** the Evidence record from REQ-GRD-003
- **When** the lineage traversal is invoked (read-set enumeration helper
  defined in design)
- **Then** the originating `FactId` is reachable
- **And** no `UngroundedLineage` placeholder is encountered on the path

---

## REQ-GRD-005 — Negative case: ungrounded Finding cannot gate

The same detector against a stub-fed or hand-crafted snapshot that bypasses
the canonical FactStore MUST emit a Finding that **structurally looks the
same** but `FindingVerifier::accept(...)` rejects OR
`PolicyGate::evaluate(...)` refuses to open.

### Scenario 5.1 — Ungrounded refusal (UAT-U67.5 — the decisive test)

- **Given** a synthetic Finding where:
  - Either `grounding == GroundingRef::Ungrounded`
  - Or `grounding == GroundingRef::fact(F)` but `F` does NOT resolve to a
    committed fact in the FactStore
- **When** `PolicyGate::evaluate(finding)` runs
- **Then** the gate decision is `Refuse`
- **And** the refusal reason cites missing-grounding (not a generic test
  failure or panic)

### Scenario 5.2 — Grounded admission (the complementary test)

- **Given** the grounded Finding from REQ-GRD-002/003
- **When** `PolicyGate::evaluate(finding)` runs
- **Then** the gate decision is `Admit` (or whatever "grounded → can gate"
  verdict the existing verifier returns — design phase names it)

---

## REQ-GRD-006 — Minimum canonical vocabulary is bounded

The cycle's grounded predicates are limited to those actually exercised by
the selected detector plus at most one auxiliary predicate for lineage
navigation.

### Scenario 6.1 — Vocabulary document

- **Given** the detector chosen in design
- **Then** `openspec/changes/e67-lsi-production-grounding/grnd-vocab.md`
  enumerates exactly the predicates exercised by the detector + at most one
  auxiliary predicate (e.g. `core:contains` for file-level lineage)

---

## Layering invariants (carried from proposal, enforced by review)

| Layer | Allowed to import | Must NOT import |
|-------|-------------------|------------------|
| `cognicode-core::domain::*` | domain only | `tree-sitter`, `tokio`, `sqlx`, `ladybug`, I/O |
| `cognicode-core::application::fact_bridge` | domain + `application::ingest::types::ExtractionResult` | `runtime`, `explorer` |
| `cognicode-core::infrastructure::parser` | `tree-sitter`, `application` | `domain` |
| `cognicode-runtime` | everything (composition root) | (none) |
| `cognicode-explorer` | `core`, `runtime` (port surface only) | `tree-sitter`, Fact construction |

## Conformance

Each scenario above is bound to a `cargo test` invocation enumerated in
`tasks.md`. The adversarial scenario 5.1 is the **cycle's close gate**: if it
does not pass, the cycle does not close.

## Out of spec

- Multi-language producer framework
- LSP-driven `RuntimeObserver` integration with the canonical fact path
- Read-set **invalidation** consumer (read-set *type* is from e66; full
  invalidation binding is e68)
- Distributed / async FactStore paths
- Performance tuning (fact dedup, batch sizing, indexing) — must remain
  byte-identical to e37's reference for the existing equivalence corpus
- Any change to the canonical predicates or to `GroundingRef`'s API surface