# Spec: postgres-callgraph-persistence (OBSOLETE — 2026-08-04)

> **Status: OBSOLETE** — PostgreSQL removed (ADR-026, e29-7).
> Archive this spec.

> Companion to engram `sdd/explorer-graph-postgres-graphstore/spec` and LogSeq `Spec: explorer-graph-postgres-graphstore`.

## Purpose

> **Reconciliation note (2026-08-01)**: the `save_call_graph` /
> `load_call_graph` inherent methods on `PostgresRepository` referenced in
> this spec were the **pre-Phase-0 surface**. The e29-0-define-new-ports +
> e29-0-refactor-call-sites changes relocated them behind the
> `CallGraphStore` domain port (with the `_ws` suffix):
>
> - `PostgresRepository::save_call_graph(&self, graph)` →
>   `CallGraphStore::save_call_graph_ws(&self, graph, ws)`
> - `PostgresRepository::load_call_graph(&self)` →
>   `CallGraphStore::load_call_graph_ws(&self, ws, rev)` or
>   `CallGraphStore::load_call_graph_current(&self, ws)`
>
> The **contract** (workspace-scoped, atomic per revision, idempotent re-save)
> is unchanged — only the port path changed. The pre-Phase-0
> `PostgresRepository` inherent method names remain in the concrete adapter
> as pass-through delegates to the new port.


Closes PG's write-path gap. PG has a full read-path (5 `Repository` methods over `symbols` + `call_edges`) but no canonical write-path. This domain adds async inherent methods `save_call_graph(&CallGraph)` and `load_call_graph() -> Option<CallGraph>` on `PostgresRepository`, atomically populating `symbols` + `call_edges` in one `sqlx` transaction. Pure additive; SQLite / `GraphStore` / `Repository` untouched.

## Requirements

### Requirement: `save_call_graph` inherent write method

`PostgresRepository::save_call_graph(&self, graph: &CallGraph, workspace: &WorkspaceId) -> Result<RevisionId, RepositoryError>` MUST be `pub async`, `#[cfg(feature = "postgres")]`-gated, workspace-scoped and revision-aware. Body in one `pool.begin()`: (1) open a new `graph_revisions` row atomically demoting the previous head, (2) DELETE `graph_nodes`/`graph_edges` for the workspace, (3) INSERT symbols and edges binding `workspace_id`, (4) COMMIT / ROLLBACK. Returns the new `RevisionId` only after COMMIT.

#### Scenario: Happy path opens a new revision

- GIVEN empty `graph_nodes`/`graph_edges` for `ws1` AND a `CallGraph` with 7 symbols, 12 edges
- WHEN `save_call_graph(&g, &ws1)` awaits
- THEN the result is `Ok(rev)` AND `rev > 0`
- AND `graph_revisions` for `ws1` has one row with `head_of=true` and `revision_id = rev`
- AND `count_symbols(ws1) == 7` AND `count_edges(ws1) == 12`

#### Scenario: Workspace-scoped delete-and-replace

- GIVEN `ws1` has 3 symbols (rev 4) AND `ws2` has 5 (rev 7)
- WHEN `save_call_graph(&graph_b, &ws1)` runs with 5 different symbols
- THEN `count_symbols(ws1) == 5` AND `count_symbols(ws2) == 5`
- AND `ws1` head advances to `5` while `ws2` head stays at `7`

#### Scenario: Idempotent re-save

- GIVEN a `CallGraph` saved once
- WHEN `save_call_graph(&same_graph)` runs again
- THEN counts equal post-first-save counts AND the row set is semantically equivalent (surr. `SERIAL` ids regenerate; semantic equality holds)

### Requirement: Transactional atomicity on partial failure

If any INSERT fails, the transaction MUST roll back including the newly-opened `graph_revisions` row. Prior rows are restored.

#### Scenario: Mid-INSERT failure rolls back workspace and revision

- GIVEN empty `graph_nodes`/`graph_edges` for `ws1` AND a `CallGraph` with one symbol colliding with a pre-seeded unique-index row
- WHEN `save_call_graph(&g, &ws1)` awaits
- THEN the result is `Err(RepositoryError::Store(_))`
- AND `count_symbols(ws1) == 0` AND `count_edges(ws1) == 0`
- AND `graph_revisions` for `ws1` has NO row

#### Scenario: Rollback unwinds the DELETE phase

- GIVEN `CallGraph_A` (3 sym, 4 edges) already persisted
- WHEN `save_call_graph(&graph_b)` fails AFTER both `DELETE`s (test seam: rejected `kind`)
- THEN A's 3 symbols and 4 edges are still present post-failure

### Requirement: `load_call_graph` inherent read method

`PostgresRepository::load_call_graph(&self, workspace: &WorkspaceId, revision: RevisionId) -> Result<Option<CallGraph>, RepositoryError>` MUST be `pub async`, `#[cfg(feature = "postgres")]`-gated, read-only, pinned to one `(workspace, revision)`. Returns `Ok(None)` iff both tables are empty for that workspace+revision. Returns `Err(RepositoryError::UnknownRevision { workspace, revision })` when the revision row does not exist — NEVER silent fall-back to head.

#### Scenario: Populated workspace+revision returns exact rows

- GIVEN a 7 sym / 12 edge mixed-provenance `CallGraph` saved to `ws1` at revision `5`
- WHEN `load_call_graph(ws1, 5)` awaits
- THEN the result is `Ok(Some(g2))` AND `g2.symbol_count()==7` AND `g2.edge_count()==12`
- AND every edge's `(provenance, confidence)` matches source bit-for-bit

#### Scenario: Unknown revision fails closed

- GIVEN `ws1` head=5
- WHEN `load_call_graph(ws1, 99)` awaits
- THEN the result is `Err(UnknownRevision { workspace: ws1, revision: 99 })`
- AND no head fallback occurs

#### Scenario: Mixed-provenance round trip preserves metadata

- GIVEN three edges `(Extracted,1.0)`, `(Inferred,0.7)`, `(Ambiguous,0.3)`
- WHEN round-tripped
- THEN `loaded.edges_with_metadata()` yields the same three pairs (order unspecified)

### Requirement: Semantic equivalence with in-memory `CallGraph`

Round trip `save_call_graph(G, ws) → load_call_graph(ws, returned_rev)` MUST produce `G'` `PartialEq`-equal to `G`, pinned to the SAME revision id that `save_call_graph` returned.

#### Scenario: assert_eq! with revision pin

- GIVEN a fixture saved to `ws1` at revision `r`
- WHEN `load_call_graph(ws1, r)` is called immediately after
- THEN `assert_eq!(g, loaded)` passes AND counts match

#### Scenario: Self-loop and multi-edge same-pair are preserved

- GIVEN a self-loop (caller==callee) AND a multi-edge between the same pair with different `DependencyType`s
- WHEN round-tripped
- THEN both edges are present in `loaded` AND each carries the source `(provenance, confidence)`

### Requirement: Deletion completeness across graph tables and manifest

When a workspace file is removed (no longer in the new `scan_manifest`), the next ingest commit MUST delete every `graph_nodes` row whose `source_path` no longer appears in the new manifest AND every `graph_edges` row whose endpoints are now missing from `graph_nodes`. Matching `scan_manifest` rows MUST also be removed. Deletion runs in the same transaction as the revision open.

#### Scenario: Removed file disappears from nodes and edges
- GIVEN revision `3` has node `src/x.rs:foo:1` and an edge whose endpoints both live in `src/x.rs`
- WHEN revision `4` is committed WITHOUT `src/x.rs` in `scan_manifest`
- THEN `count_nodes(ws1, source_path='src/x.rs')` at rev 4 is `0`
- AND the edge is no longer present in `load_call_graph(ws1, 4)`

#### Scenario: Removed file disappears from scan_manifest
- GIVEN `scan_manifest(ws1, "src/x.rs")` exists at revision `3`
- WHEN revision `4` is committed WITHOUT `src/x.rs`
- THEN `count(*) from scan_manifest where workspace_id='ws1' and file_path='src/x.rs'` is `0`

#### Scenario: Other workspaces are unaffected
- GIVEN `ws1` removes `src/x.rs` AND `ws2` keeps `src/x.rs`
- WHEN revision `4` is committed for both
- THEN `ws1`'s node `src/x.rs:foo:1` is gone
- AND `ws2`'s node `src/x.rs:foo:1` is still present

### Requirement: Non-breaking behavior vs SQLite `GraphStore`

MUST NOT modify `SqliteGraphStore`, `GraphStore`, the sync write-path, or any pre-slice public API of `cognicode-core` (other than additive inherent methods). With `postgres` disabled, new methods MUST NOT be reachable.

#### Scenario: Default build still passes the pre-slice suite

- GIVEN pre-slice public API
- WHEN `cargo test --workspace` runs WITHOUT `--features postgres`
- THEN every pre-slice test passes AND `cargo doc --no-deps -p cognicode-core` exposes the same items AND `use …::PostgresRepository::save_call_graph;` fails to compile

#### Scenario: `SqliteGraphStore` and `GraphStore` untouched

- GIVEN pre-slice revisions of `cognicode-db/src/graph.rs` and `domain::traits::graph_store.rs`
- WHEN this slice lands
- THEN `git diff HEAD --` for both files is empty AND the sync write-path functions identically

### Requirement: Reused `postgres` feature flag (no new flag)

Reuses the existing `postgres` feature from prior slices. All new code (methods + tests) MUST be `#[cfg(feature = "postgres")]`-gated. No new feature flag is introduced.

#### Scenario: Default build stays sqlx-free

- GIVEN clean workspace
- WHEN `cargo check --workspace` runs WITHOUT `--features postgres`
- THEN build succeeds AND `sqlx` is absent from the dep graph AND new methods are unreachable

#### Scenario: Feature-enabled build exposes new methods

- GIVEN `--features postgres`
- WHEN `cargo check -p cognicode-core --features postgres --no-default-features` runs
- THEN both new methods are reachable AND the gated test module compiles

### Requirement: Testability — per-test isolation, contract coverage

Tests in `postgres_repository.rs` under `#[cfg(all(test, feature = "postgres"))]` MUST use `#[sqlx::test]` for per-test isolated DBs. Suite MUST cover: save happy path, load empty→`None`, load populated→`Some` with exact metadata, round-trip `assert_eq!`, mid-INSERT rollback, delete-and-replace, idempotent re-save, success contract.

#### Scenario: Per-test isolation

- GIVEN two parallel `#[sqlx::test]` functions with disjoint `CallGraph`s
- WHEN they run in parallel
- THEN each observes an isolated DB AND rows of one are not visible to the other AND both assertion sets pass

#### Scenario: No ignored contract tests

- GIVEN the save/load contract test module
- WHEN `cargo test -p cognicode-core --features postgres -- postgres_repository::tests::save_load` runs
- THEN every listed scenario has ≥1 passing test AND no test is `#[ignore]` without documented rationale

### Requirement: Rollback and rollout safety

Revertible with one `git revert`. No trait change, no schema change, no other file modified. Re-deploying MUST NOT alter schema or rows until `save_call_graph` is invoked.

#### Scenario: `git revert` restores pre-slice build

- GIVEN slice merged
- WHEN a single `git revert <merge-sha>` runs
- THEN `cargo check --workspace` (no `postgres` feature) succeeds AND the `postgres`-feature build does not regress vs pre-slice

#### Scenario: No schema drift in PG

- GIVEN a PG instance with pre-slice `symbols` + `call_edges` populated
- WHEN `run_migrations()` re-runs after redeploy
- THEN schema is byte-identical (no `ALTER TABLE`, no new tables/indexes) AND existing rows preserved

#### Scenario: PR size budget

- GIVEN the planned changes (1 file, additive inherent methods + contract tests)
- WHEN the diff is computed
- THEN `additions + deletions` ≤ 400 AND `400-line-budget-risk` is Low

## Status

Draft. Awaiting `sdd-design`.

## Coverage

Happy paths: covered. Edge cases: covered (empty, self-loop, multi-edge, f64 boundaries). Error states: covered (mid-INSERT rollback, DELETE-phase rollback, success contract). Non-breaking: covered.

## Out of Scope (locked)

`GraphStore` impl for PG; new async `GraphPersistence` trait; new `Repository` methods; new tables/columns/indexes in PG; bincode/blob sidecar; explorer-PG adapter; MCP envelope; petgraph projection; `ltree`/`pgvector`; `Component`/`Container`/`System` kinds; removal of SQLite / `cognicode-store-traits` / `GraphStore`.
