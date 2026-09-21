# e67 Tasks — LSI Production Grounding Activation (Rust)

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e67-lsi-production-grounding` |
| Phase | tasks |
| Design | `design.md` |
| Path | a-lite |
| Date | 2026-09-16 |

## Governance: single-WU-per-checkpoint rule (degraded-but-governed mode)

This cycle runs without a delegated `sddk-apply` worker (DEBT-SDDK-003). To
compensate, **WU2 is NOT opened until WU1's checkpoint is GREEN**, and **WU3
is NOT opened until WU2's checkpoint is GREEN**. Each WU's acceptance
selector is a specific `cargo test` invocation. If a checkpoint is RED we
stop, fix the WU, re-run that WU's checkpoint, and re-run its dependent
checks. We do not silently widen.

## Hard invariant (carried from design REQ-DGN-001)

Production grounding orchestration is **asynchronous end-to-end**. The delta
introduced by e67 MUST NOT contain any of:

- `Handle::current().block_on(...)`
- `Runtime::new()....block_on(...)`
- `tokio::runtime::Builder::new_current_thread()....block_on(...)`
- `futures::executor::block_on(...)`
- any other "sync wrapper around async I/O" construct

`FactStore::commit(...).await` propagates to the caller. The caller owns
async execution. Enforced by `rg` static check in WU1 checkpoint.

## Fixture (created once, used by WU1/WU2/WU3)

| Step | Action | Acceptance |
|---|---|---|
| F1 | Create `sandbox/fixtures/lsi-grounding/sample.rs` with `mod inner { fn helper }` + `fn greet()` | file exists, parses with `extract_file` |
| F2 | Create `sandbox/fixtures/lsi-grounding/expected_facts.json` documenting the canonical predicate set and FactId range expected | file exists, JSON valid |
| F3 | Add `sandbox/fixtures/lsi-grounding/README.md` with the shape rationale (mirrors e37 goldens convention) | file exists |

These are NOT a work-unit; they are scaffolding shared by WU1/WU2/WU3.

---

## WU1 — Production ingest path (async end-to-end)

Goal: real Rust source → `extract_file` → `tree_sitter_facts::collect` →
`FactBatchBuilder::finish` → `FactStore::commit(...).await` → persisted
canonical Facts. **No detector yet.**

| Step | Task | Acceptance |
|---|---|---|
| 1.1 | Add `crates/cognicode-core/src/application/fact_bridge/production_grounding.rs` with: `pub struct GroundedIngestReceipt { pub snapshot: SnapshotId, pub fact_ids: Vec<FactId>, pub fact_count: usize }`; `pub enum ProductionGroundingError { Extract(...) }` (covers extraction failure); `pub async fn ingest_rust_facts(fact_store: &dyn FactStore, ws: &WorkspaceId, snap: &SnapshotId, path: &Path, source: &str, hash: &str) -> Result<GroundedIngestReceipt, ProductionGroundingError>`. Internally: parse the rust `LanguageConfig`, call `extract_file`, call `tree_sitter_facts::collect(&mut builder, &result)`, `builder.finish()`, `fact_store.commit(ws, snap, batch).await?`, return receipt. | file exists, compiles with `cargo check -p cognicode-core` |
| 1.2 | Add `pub mod production_grounding;` to `crates/cognicode-core/src/application/fact_bridge/mod.rs` | grep match |
| 1.3 | Add `crates/cognicode-runtime/src/grounding.rs` with `pub async fn ingest_rust_source(fact_store: Arc<dyn FactStore>, ws: WorkspaceId, snap: SnapshotId, path: &Path, source: &str, hash: &str) -> Result<GroundedIngestReceipt, ProductionGroundingError>`. Calls `cognicode_core::application::fact_bridge::production_grounding::ingest_rust_facts(...).await`. | file exists, compiles |
| 1.4 | Add `pub mod grounding;` to `crates/cognicode-runtime/src/lib.rs` | grep match |
| 1.5 | Confirm `tokio` is a dev-dep of `cognicode-core` (and `cognicode-runtime` if needed) for `#[tokio::test]`. If not present, add `[dev-dependencies] tokio = { version = "1", features = ["rt", "macros"] }`. **Do NOT add as runtime dep.** | `grep tokio crates/cognicode-core/Cargo.toml` shows dev-dep only |
| 1.6 | Add `#[cfg(test)] mod tests` in `crates/cognicode-core/src/application/fact_bridge/production_grounding.rs` with `#[tokio::test]` cases: `test_ingest_rust_fixture_into_canonical_factstore`, `test_batch_predicate_set_bounded`, `test_replay_byte_stable`, `rust_production_ingest_persists_facts_pinned_to_snapshot`, `production_ingest_is_not_visible_from_another_snapshot` | tests pass |
| 1.7 | Add `#[cfg(test)] mod tests` in `crates/cognicode-runtime/src/grounding.rs` with one `#[tokio::test]` case `test_runtime_grounding_ingest_rust_source` | tests pass |
| 1.8 | Run `just lsi-equivalence` — assert no drift | exits 0, harness pin unchanged |

### WU1 invariants (all must hold before WU2 opens)

| # | Invariant | Enforcement |
|---|---|---|
| 1 | Real source → real facts (not hand-crafted `ExtractionResult`) | Test loads fixture from disk |
| 2 | Canonical persistence | Test `pinned_to_snapshot` |
| 3 | Snapshot pinning | Test `not_visible_from_another_snapshot` |
| 4 | Determinism | Test `replay_byte_stable` (semantic equality) |
| 5 | Bounded vocabulary | Test `predicate_set_bounded` |
| 6 | No Fact fabrication in tests | Apply review: only public FactStore API |
| 7 | Layering | Static check: zero `tree-sitter` imports in `domain/`; zero concrete `FactStore` imports in `application/` |
| 8 | No regression | `just lsi-equivalence` stays green |
| 9 | No async bridge | Static check (REQ-DGN-001) |

### WU1 checkpoint (must pass before WU2 opens)

```text
cargo test -p cognicode-core --lib production_grounding --features evidence-kernel
cargo test -p cognicode-runtime --lib grounding --features evidence-kernel
cargo clippy -p cognicode-core -p cognicode-runtime --all-targets --features evidence-kernel -- -D warnings
cargo fmt --check -p cognicode-core -p cognicode-runtime
just lsi-equivalence

# REQ-DGN-001 enforcement — no new block_on bridges
rg 'Handle::current\(\)\.block_on|Runtime::new\(\).*block_on|futures::executor::block_on' \
   crates/cognicode-core crates/cognicode-runtime
# Expected: zero matches (or matches predating e67 baseline only)
```

---

## WU2 — Grounded detector execution (DEFERRED)

Goal: `SnapshotEntityView` of the WU1 snapshot → existing detector →
`Finding` with `GroundingRef::fact(F)` → `CanonicalEvidenceWriter::write` →
evidence navigable to the originating fact. **No gate yet (that's WU3).**

Pre-condition: WU1 checkpoint GREEN.

(Detailed WU2 tasks will be added when WU1 checkpoint is GREEN; they are
intentionally omitted here to keep this slice focused on WU1.)

---

## WU3 — Adversarial gate (DEFERRED)

Goal: prove that the same detector's output gates iff grounded.

Pre-condition: WU2 checkpoint GREEN.

(Detailed WU3 tasks will be added when WU2 checkpoint is GREEN.)

---

## WU3 — Status: GREEN (committed)

Implementation: `crates/cognicode-core/src/application/findings/grounded_finding_flow.rs`.

Composition only — no new authority path. The flow composes the existing
authorities: WU1 ingest → WU2 grounded projection → `DetectorAdmission`
+ `EligibleSourceVerifier` promotion → `AstBackend` execution →
`CanonicalEvidenceWriter::persist` → `PreparedExecution::finalize` →
`FindingAssembler::assemble` → `KernelEvidenceReadModel::load` →
`FindingVerifier::can_block`.

Coverage (10/10 passing):

- Positive UAT: full vertical grounded → gateable.
- Negative UAT: same detector/source with `grounding=None` cannot block.
- 6-case adversarial matrix:
  - ungrounded detector output → refused
  - wrong-fact subject → refused
  - cross-snapshot fact id → refused (snapshot mismatch)
  - subject-mismatch finding → refused
  - ambiguous canonical source → `grounding=None`, refused
  - refuting evidence → refused
- Sanity guards: receipt carries committed fact ids only.

Regression (all green):

- `just lsi-equivalence` → 7/7 PASS (same scores, multi-lang-types still QUARANTINED).
- e66 readset → 1/1.
- `findings_ast_e2e` 8/8, `findings_graph_e2e` 4/4,
  `findings_dataflow_e2e` 7/7, `findings_canonical_grounding_e2e` 10/10.
- `intelligence_event_log_e2e` → 4/4.

Static gates:

- `rg 'Handle::current\(\)\.block_on' <file>` → no match.
- `cargo fmt --check -p cognicode-core` → clean.
- `cargo clippy -p cognicode-core --all-targets --features evidence-kernel -- -D warnings` → clean.
- `cargo test -p cognicode-core --lib --features evidence-kernel --no-run` → only
  pre-existing warnings in `admission.rs` (`run_dfg`, `budget_payload`,
  `time_decl`, `scope` never used; one useless comparison) — none in e67 files.

---

## Risk-driven additions (apply phase discretion)

If during WU1 apply the `FactStore::commit` signature proves incompatible
(e.g. different parameter shape than expected), the design's Risk 2 mitigation
is: read the trait and adjust the call without changing the design intent.
This is **not** a design change — it is a correct implementation against the
real signature.

## Out of WU scope (explicit non-goals carried forward)

- Multi-language producer
- LSP / RuntimeObserver integration
- Read-set **invalidation** consumer (only `ReadSet::contains`/iteration)
- Distributed/async FactStore optimization
- Performance tuning
- New `core:*` predicates
- `GroundingRef` API changes
- Pack governance / Architecture constraints
- CI / EvidenceBundle / PolicyGate external consumers