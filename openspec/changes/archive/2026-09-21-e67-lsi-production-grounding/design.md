# e67 Design — LSI Production Grounding Activation (Rust)

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e67-lsi-production-grounding` |
| Phase | design |
| Spec | `spec.md` |
| Proposal | `proposal.md` |
| Date | 2026-09-16 |

## Ground-truth discovered in plan phase

| Element | State | Source |
|---|---|---|
| `extract_file` | sync; signature: `pub fn extract_file(config, path, source, content_hash) -> ExtractionResult` | `crates/cognicode-core/src/application/ingest/extractor.rs:30` |
| `tree_sitter_facts::collect` | already canonical; takes `&mut FactBatchBuilder + &ExtractionResult`; producer = `DeterministicAnalyzer`; emits `core:defines`, `core:contains`, `core:calls`, `core:imports`, `core:references` | `crates/cognicode-core/src/application/fact_bridge/tree_sitter_facts.rs` |
| `FactBatchBuilder` | `pub fn new(SnapshotId)`, `pub fn add_observation(...) -> Result<(), FactBridgeError>`, `pub fn finish(self) -> Vec<Fact>` | `crates/cognicode-core/src/application/fact_bridge/batch_builder.rs` |
| `FactStore::commit` | **async**; signature: `async fn commit(&self, ws: &WorkspaceId, snap: &SnapshotId, batch: Vec<Fact>) -> Result<Vec<FactId>, KernelError>` | `crates/cognicode-core/src/domain/evidence_kernel/ports.rs:91` |
| `FactStore` in-memory oracle | `InMemoryFactStore` already implements the trait | `crates/cognicode-core/src/infrastructure/evidence_kernel/in_memory.rs:112` |
| `WorkspaceId`, `SnapshotId`, `Fact`, `FactId`, `RelationKind` | all exist; `SnapshotId(u64)` per e37 D3 | `crates/cognicode-core/src/domain/evidence_kernel/` |
| `SnapshotEntityView` | canonical read view per snapshot, built from committed facts | `crates/cognicode-core/src/domain/evidence_kernel/continuity/view.rs:65` |
| `CanonicalEvidenceWriter` | exists | `crates/cognicode-core/src/application/findings/kernel_bridge.rs` |
| `FindingVerifier` | exists | `crates/cognicode-core/src/domain/findings/` |
| `GroundingRef::fact(F)` / `GroundingRef::Ungrounded` | exists | `crates/cognicode-core/src/domain/findings/grounding.rs` |
| `DetectorExecutor` (M6 path) | exists | `crates/cognicode-core/src/domain/findings/` |

### Critical finding: there are **no existing call-sites of `FactStore::commit`**

`grep -r "commit\b" crates/cognicode-core/src crates/cognicode-runtime/src` returns
zero production callers. e67 is genuinely **opening the first production
ingestion path into the canonical FactStore**. This confirms the proposal's
core motivation.

### Critical finding: `commit` is `async` — e67 propagates async end-to-end

`FactStore::commit` is async. e67 inherits this — the production grounding
orchestration MUST be async end-to-end. The caller (runtime composition root
or any future CLI/MCP caller) already runs under `tokio` (the runtime's
`bootstrap_with_backend` is async-aware and `cognicode-mcp` is a tokio
binary).

**Decision for this cycle**: `production_grounding::ingest_rust_facts(...)`
is `pub async fn`; the runtime seam is `pub async fn ingest_rust_source(...)`
that `await`s it. **No sync wrapper. No `Handle::current().block_on`. No
`Runtime::new().block_on`. No `futures::executor::block_on`. No nested
executor.** This is a hard invariant for the cycle — see REQ-DGN-001 below.

The rationale for "no block_on bridges" is the same the repository adopted
when it migrated ports to async: `Handle::current().block_on` is an
antipattern with real deadlock risk, and any sync bridge reintroduces the
problems the async migration eliminated.

## Architecture

```text
cognicode-runtime (composition root, async-aware)
  pub mod grounding {
      pub async fn ingest_rust_source(
          fact_store: &dyn FactStore,
          ws: &WorkspaceId, snap: &SnapshotId,
          path: &Path, source: &str, hash: &str,
      ) -> Result<GroundedIngestReceipt, ProductionGroundingError>
  }
            |
            | awaits
            v
cognicode-core::application::fact_bridge::production_grounding
  pub async fn ingest_rust_facts(
      fact_store: &dyn FactStore,
      ws: &WorkspaceId, snap: &SnapshotId,
      path: &Path, source: &str, hash: &str,
  ) -> Result<GroundedIngestReceipt, ProductionGroundingError>
    - extract_file (sync, local)
    - tree_sitter_facts::collect (existing)
    - FactBatchBuilder::finish -> Vec<Fact>
    - fact_store.commit(ws, snap, batch).await
    - returns GroundedIngestReceipt { snapshot, fact_ids, fact_count }
            |
            v
cognicode-core::infrastructure::evidence_kernel
  InMemoryFactStore (oracle) / LadybugFactStore (later)
            |
            v
SnapshotEntityView (read surface for detectors, WU2)
```

### REQ-DGN-001 — Async end-to-end (hard invariant)

Production grounding orchestration is asynchronous. The delta introduced by
e67 MUST NOT contain any of:

- `Handle::current().block_on(...)`
- `Runtime::new()....block_on(...)`
- `tokio::runtime::Builder::new_current_thread()....block_on(...)`
- `futures::executor::block_on(...)`
- any other "sync wrapper around async I/O" construct

`FactStore::commit(...).await` propagates to the caller. The caller owns
async execution. This invariant is enforced by:

1. Code review (apply phase)
2. The static check command in WU1's checkpoint:
   `rg 'Handle::current\(\)\.block_on|Runtime::new\(\).*block_on|futures::executor::block_on' crates/cognicode-core crates/cognicode-runtime`
   — the **delta** of e67 must not introduce any new match.

This rule exists because `block_on`-style bridges are an antipattern in
this codebase (the async migration eliminated them precisely for this
reason). Reintroducing them in e67 would be a regression.

## New files

| Path | Purpose |
|---|---|
| `crates/cognicode-core/src/application/fact_bridge/production_grounding.rs` | New module: `pub async fn ingest_rust_facts(...) -> Result<GroundedIngestReceipt, ProductionGroundingError>`. Reuses `extract_file` + `tree_sitter_facts::collect` + `FactBatchBuilder::finish` + `fact_store.commit(...).await`. Also defines `GroundedIngestReceipt` and `ProductionGroundingError`. |
| `crates/cognicode-runtime/src/grounding.rs` | New module: `pub async fn ingest_rust_source(...)` composition seam. Mirrors `production_grounding::ingest_rust_facts` but also resolves the `LanguageConfig` for Rust (re-uses `crates/cognicode-core::infrastructure::parser::language_config`). |
| `sandbox/fixtures/lsi-grounding/sample.rs` | New fixture: a small Rust module with `mod inner { fn helper }` + `fn greet()`. Exercises `core:defines`, `core:calls`, `core:contains`. |
| `sandbox/fixtures/lsi-grounding/expected_facts.json` | Pin: expected canonical predicate set and FactId range for regression-detection. |
| `sandbox/fixtures/lsi-grounding/README.md` | Shape rationale (mirrors e37 goldens convention). |

## Modified files

| Path | Reason |
|---|---|
| `crates/cognicode-runtime/src/lib.rs` | Add `pub mod grounding;` |
| `crates/cognicode-core/src/application/fact_bridge/mod.rs` | Add `pub mod production_grounding;` |
| `crates/cognicode-core/Cargo.toml` | Confirm `tokio` is available as dev-dep for `#[tokio::test]`. **Do NOT add as runtime dep.** If absent in `cognicode-core`, add to `[dev-dependencies]`. |

## Files NOT touched (layering enforcement)

- `crates/cognicode-core/src/domain/**` — pure domain, no I/O. Read-set / Fact types already exist.
- `crates/cognicode-explorer/**` — does NOT construct Facts.
- `crates/cognicode-core/src/infrastructure/parser/**` — tree-sitter lives here, not in domain.
- `crates/cognicode-cli/**`, `crates/cognicode-mcp/**` — out of scope.

## Detector selection (Risk 1 from proposal)

TBD in WU2 (apply phase) by inspection of which existing detector already
consumes `SnapshotEntityView` and emits a Finding with `GroundingRef::fact(...)`.
The design constraint is: the detector's **existing** code must already use
the canonical grounding surface (so WU2 does not modify detector semantics),
and it must exercise the `core:defines` (and ideally `core:calls`) predicate.

Concrete search (WU2 apply phase): `grep -l "SnapshotEntityView\|core:defines"
crates/cognicode-core/src/application/findings/`. Whatever is found becomes
the anchored detector; if multiple qualify, pick the smallest one.

## Fixture

`sandbox/fixtures/lsi-grounding/sample.rs`:

```rust
//! Small canonical fixture for e67 grounded ingest.

pub mod inner {
    pub fn helper() -> u32 { 42 }
}

pub fn greet() -> u32 {
    inner::helper()
}
```

This yields at minimum: one `core:contains` (sample.rs contains `inner`),
2 `core:defines` (greet, helper), 1 `core:calls` (greet → inner::helper).
The expected predicates set = `{core:defines, core:contains, core:calls}`.

## Test plan

### WU1 — Production ingest (REQ-GRD-001)

Tests live in `crates/cognicode-core/src/application/fact_bridge/production_grounding.rs`
under `#[cfg(test)] mod tests`. All tests use `#[tokio::test]`.

| Test | What it asserts |
|---|---|
| `test_ingest_rust_fixture_into_canonical_factstore` | end-to-end: load fixture → `extract_file` → `tree_sitter_facts::collect` → `FactBatchBuilder::finish` → `InMemoryFactStore::commit`; asserts `core:defines` for `sample::greet` and `sample::inner::helper` present |
| `test_batch_predicate_set_bounded` | asserts only `core:defines`, `core:contains`, `core:calls`, `core:imports`, `core:references` predicates in batch (no rogue predicates) |
| `test_replay_byte_stable` | run ingest twice; assert the two `Vec<FactId>` outputs are identical |
| `rust_production_ingest_persists_facts_pinned_to_snapshot` | after commit, read back via the existing snapshot read API and confirm the facts are reachable for the same `(ws, snap)` |
| `production_ingest_is_not_visible_from_another_snapshot` | commit to `snap=1`; create `snap=2` for the same workspace; assert the facts from snap=1 are not visible in snap=2 (U42-style snapshot isolation regression guard) |

WU1 also adds a runtime seam test in `crates/cognicode-runtime/src/grounding.rs`:

| Test | What it asserts |
|---|---|
| `test_runtime_grounding_ingest_rust_source` | calls `cognicode_runtime::grounding::ingest_rust_source(...)` with the fixture; asserts success and FactId non-empty |

### WU2 — Grounded detector (REQ-GRD-002/003/004) — DEFERRED

Will add tests in `crates/cognicode-runtime/src/grounding.rs`:

- `test_grounded_detector_emits_finding_with_fact_grounding`
- `test_canonical_evidence_writer_persists_fact_ref`
- `test_lineage_evidence_to_fact_navigable`

### WU3 — Adversarial gate (REQ-GRD-005) — DEFERRED

- `test_ungrounded_finding_refused_at_gate`
- `test_fabricated_fact_id_refused_at_gate`
- `test_grounded_finding_admitted_at_gate`

## WU1 invariants (apply phase must satisfy all)

| # | Invariant | Enforcement |
|---|---|---|
| 1 | Real source → real facts (not hand-crafted `ExtractionResult`) | The test loads `sandbox/fixtures/lsi-grounding/sample.rs` from disk and feeds it to `extract_file` |
| 2 | Canonical persistence (facts recoverable from the same `(ws, snap)`) | Test 4 (`pinned_to_snapshot`) |
| 3 | Snapshot pinning (no leakage across snapshots) | Test 5 (`not_visible_from_another_snapshot`) |
| 4 | Determinism (same source + same config → same semantic batch) | Test 3 (`replay_byte_stable`) — semantic equality, not byte identity |
| 5 | Bounded vocabulary | Test 2 (`predicate_set_bounded`) |
| 6 | No Fact fabrication in tests | Apply phase review: tests use only the public FactStore API |
| 7 | Layering | Static check: zero `tree-sitter` imports in `domain/`; zero concrete `FactStore` imports in `application/` |
| 8 | No regression | `just lsi-equivalence` stays green |
| 9 | No async bridge | Static check (REQ-DGN-001) |

## Risks and mitigations

1. **Detector semantics drift (WU2)**: WU2 must not modify any detector. If a
   detector cannot produce a grounded Finding without code change, the
   design's detector selection is wrong → fall back to a different detector.
2. **Async boundary (WU1)**: addressed by `pub async fn` propagation. Tests
   use `#[tokio::test]`. No sync bridge introduced.
3. **Fixture too trivial**: the `mod inner { fn helper }` + `greet()` shape
   gives three predicates and a calls edge — minimal but non-degenerate.
4. **Equivalence harness drift**: `just lsi-equivalence` MUST pass without
   changes; this is the strongest regression gate.
5. **Tokio in runtime**: tokio is **NOT** added as a runtime dep. Only dev-dep
   for `#[tokio::test]`. The runtime seam is async.

## Decision

PROCEED to WU1 apply phase. WU2 and WU3 are explicitly deferred until WU1
checkpoint is GREEN. The cycle does not advance to release/archive until
WU3's adversarial gate test passes.