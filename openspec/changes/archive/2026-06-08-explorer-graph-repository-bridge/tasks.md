# Tasks: Repository Trait Consolidation & Metadata-Aware Bridge

**Change**: `explorer-graph-repository-bridge` (Phase 2 of Explorer Graph roadmap)
**Project**: cognicode
**Mode**: automatic, hybrid (LogSeq + Engram)
**Delivery**: single PR (auto-chain)
**Spec/Design Status**: approved

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~300 core + ~120 tests (~420 total) |
| 400-line budget risk | Low (within budget with tight scope) |
| Chained PRs recommended | No |
| Suggested split | single PR |
| Delivery strategy | single-pr |
| Chain strategy | size:exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

## Phase 1: Foundation — Sub-trait & Types in Explorer Port

- [ ] 1.1 Add `RelationTargetWithMetadata` and `EdgeWithMetadata` structs to `crates/cognicode-explorer/src/ports/symbol_repository.rs` (with `target: RelationTarget`, `dependency_type: DependencyType`, `provenance: Provenance`, `confidence: f64` — derives `Debug, Clone, PartialEq`).
- [ ] 1.2 Add `MetadataAwareRepository: SymbolRepository` sub-trait in same file with `callees_with_metadata`, `dependencies_with_metadata`, `edges_with_metadata` methods returning the new structs.
- [ ] 1.3 Add a one-paragraph doc section on `SymbolRepository` pointing to `MetadataAwareRepository` (preserves all existing signatures byte-for-byte).
- [ ] 1.4 Re-export new types in `crates/cognicode-explorer/src/ports/mod.rs` and `crates/cognicode-explorer/src/lib.rs`.

## Phase 2: Core Implementation — Adapter & Async Trait

- [ ] 2.1 Implement `MetadataAwareRepository` for `CallGraphRepository` in `crates/cognicode-explorer/src/adapters/call_graph_repository.rs` — delegate to existing inherent `callees_with_metadata` / `edges_with_metadata` on `CallGraph` (do NOT remove inherent methods; they are still called by `tests/explorer_graph_foundation.rs`).
- [ ] 2.2 Add `as_metadata_aware(&self) -> Option<&dyn MetadataAwareRepository>` helper on `CallGraphRepository` to avoid `Any` downcasts.
- [ ] 2.3 Create `crates/cognicode-core/src/domain/traits/repository.rs` with `RepositoryError` enum and standalone `#[async_trait] Repository: Send + Sync` trait (`find_symbol_by_qualified_name`, `count_symbols` — composed from, not inheriting, `GraphStore`).
- [ ] 2.4 Add `pub mod repository;` and re-exports in `crates/cognicode-core/src/domain/traits/mod.rs`.

## Phase 3: Deprecation — `cognicode-store-traits` (in place)

- [ ] 3.1 Append `(DEPRECATED)` to `description` in `crates/cognicode-store-traits/Cargo.toml`.
- [ ] 3.2 Add `DEPRECATED` directive at top of `crates/cognicode-store-traits/src/lib.rs` (first 20 lines) with one-line reason pointing to `cognicode-core::domain::traits`.
- [ ] 3.3 Add `// DEPRECATED: ...` notice at top of every `pub mod` file (`call_graph.rs`, `dependency_type.rs`, `file_manifest.rs`, `graph_store.rs`, `location.rs`, `symbol_kind.rs`, `symbol.rs`, `value_objects.rs`).
- [ ] 3.4 Remove `cognicode-store-traits` dep line from `crates/cognicode-core/Cargo.toml` (line 90) and `crates/cognicode-db/Cargo.toml` (line 15) — grep confirmed zero Rust `use` statements.
- [ ] 3.5 Verify `cargo check --workspace` and `cargo doc --workspace` pass with crate still in workspace members.

## Phase 4: Contract Tests — `MetadataAwareRepository`

- [ ] 4.1 Create `crates/cognicode-explorer/tests/metadata_aware_repository.rs` integration test file.
- [ ] 4.2 Golden test: frozen graph with 3 edges (DirectExtraction, Heuristic 0.7, Heuristic 0.4→ambiguous band) — assert exact `(Extracted, 1.0)`, `(Inferred, 0.7)`, `(Ambiguous, 0.3)` triples.
- [ ] 4.3 Invariant test: every entry in `edges_with_metadata()` has `confidence` finite, not NaN, in `[0.0, 1.0]`.
- [ ] 4.4 Backward-compat test: base `SymbolRepository::callees` returns targets with no metadata fields (sub-trait only on opt-in type).
- [ ] 4.5 Polymorphism test: `&dyn SymbolRepository` (no sub-trait) does NOT expose metadata methods.
- [ ] 4.6 Dyn-compatible test: `Box<dyn Repository>` compiles in test where `Send + Sync` is required.

## Phase 5: Verification & Rollout

- [ ] 5.1 Run `cargo check --workspace` — must pass with all dep removals.
- [ ] 5.2 Run `cargo test --workspace` — all 295 prior tests pass + new contract tests pass.
- [ ] 5.3 Run `cargo doc --workspace --no-deps` — no warnings introduced.
- [ ] 5.4 Verify single commit is reversible: `git revert <merge-sha>` on a clean checkout restores green workspace.
- [ ] 5.5 Confirm line budget: `git diff --stat <merge-base>..HEAD` shows ≤ 400 changed lines.
- [ ] 5.6 Update changelog/PR description with sub-trait pattern, deprecation note, and Phase 2 unblocking.

## Dependencies Between Tasks
- Phase 1 → Phase 2 (types must exist before adapter implements them)
- Phase 2.1, 2.2 → Phase 4 (impl + helper needed for tests)
- Phase 2.3, 2.4 → Phase 4.6 (dyn test)
- Phase 3 → Phase 5.1 (deprecation must land before final CI check)
- Phase 1 + Phase 2 + Phase 3 → Phase 4 → Phase 5

## Constraints Honored
- No PostgreSQL implementation
- No frontend changes
- No MCP changes
- `RelationTarget` untouched (no new fields, preserves `Eq` derive)
- `cognicode-store-traits` kept in workspace, only deprecated
- Synchronous `GraphStore` implementors (`InMemoryGraphStore`, `SqliteGraphStore`, `PetGraphStore`) untouched
