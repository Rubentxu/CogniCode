# Delta for postgres-callgraph-persistence

> Workspace-scoped + revision-aware save/load, deletion completeness,
> `workspace_id` in PK and uniqueness.

## MODIFIED Requirements

### Requirement: `save_call_graph` inherent write method

`PostgresRepository::save_call_graph(&self, graph: &CallGraph,
workspace: &WorkspaceId) -> Result<RevisionId, RepositoryError>` MUST
be `pub async`, `#[cfg(feature = "postgres")]`-gated, workspace-scoped
and revision-aware. Body in one `pool.begin()`: (1) open a new
`graph_revisions` row atomically demoting the previous head, (2) DELETE
`graph_nodes`/`graph_edges` for the workspace, (3) INSERT symbols and
edges binding `workspace_id`, (4) COMMIT / ROLLBACK. Returns the new
`RevisionId` only after COMMIT.

#### Scenario: Happy path opens a new revision

- GIVEN empty `graph_nodes`/`graph_edges` for `ws1` AND a `CallGraph`
  with 7 symbols, 12 edges
- WHEN `save_call_graph(&g, &ws1)` awaits
- THEN the result is `Ok(rev)` AND `rev > 0`
- AND `graph_revisions` for `ws1` has one row with `head_of=true` and
  `revision_id = rev`
- AND `count_symbols(ws1) == 7` AND `count_edges(ws1) == 12`

#### Scenario: Workspace-scoped delete-and-replace

- GIVEN `ws1` has 3 symbols (rev 4) AND `ws2` has 5 (rev 7)
- WHEN `save_call_graph(&graph_b, &ws1)` runs with 5 different symbols
- THEN `count_symbols(ws1) == 5` AND `count_symbols(ws2) == 5`
- AND `ws1` head advances to `5` while `ws2` head stays at `7`

### Requirement: Transactional atomicity on partial failure

If any INSERT fails, the transaction MUST roll back including the
newly-opened `graph_revisions` row. Prior rows are restored.

#### Scenario: Mid-INSERT failure rolls back workspace and revision

- GIVEN empty `graph_nodes`/`graph_edges` for `ws1` AND a `CallGraph`
  with one symbol colliding with a pre-seeded unique-index row
- WHEN `save_call_graph(&g, &ws1)` awaits
- THEN the result is `Err(RepositoryError::Store(_))`
- AND `count_symbols(ws1) == 0` AND `count_edges(ws1) == 0`
- AND `graph_revisions` for `ws1` has NO row

### Requirement: `load_call_graph` inherent read method

`PostgresRepository::load_call_graph(&self, workspace: &WorkspaceId,
revision: RevisionId) -> Result<Option<CallGraph>, RepositoryError>`
MUST be `pub async`, `#[cfg(feature = "postgres")]`-gated, read-only,
pinned to one `(workspace, revision)`. Returns `Ok(None)` iff both
tables are empty for that workspace+revision. Returns
`Err(RepositoryError::UnknownRevision { workspace, revision })` when
the revision row does not exist — NEVER silent fall-back to head.

#### Scenario: Populated workspace+revision returns exact rows

- GIVEN a 7 sym / 12 edge mixed-provenance `CallGraph` saved to `ws1`
  at revision `5`
- WHEN `load_call_graph(ws1, 5)` awaits
- THEN the result is `Ok(Some(g2))` AND `g2.symbol_count()==7` AND
  `g2.edge_count()==12`
- AND every edge's `(provenance, confidence)` matches source bit-for-bit

#### Scenario: Unknown revision fails closed

- GIVEN `ws1` head=5
- WHEN `load_call_graph(ws1, 99)` awaits
- THEN the result is `Err(UnknownRevision { workspace: ws1, revision: 99 })`
- AND no head fallback occurs

### Requirement: Semantic equivalence with in-memory `CallGraph`

Round trip `save_call_graph(G, ws) → load_call_graph(ws, returned_rev)`
MUST produce `G'` `PartialEq`-equal to `G`, pinned to the SAME revision
id that `save_call_graph` returned.

#### Scenario: assert_eq! with revision pin

- GIVEN a fixture saved to `ws1` at revision `r`
- WHEN `load_call_graph(ws1, r)` is called immediately after
- THEN `assert_eq!(g, loaded)` passes AND counts match

## ADDED Requirements

### Requirement: Deletion completeness across graph tables and manifest

When a workspace file is removed (no longer in the new
`scan_manifest`), the next ingest commit MUST delete every `graph_nodes`
row whose `source_path` no longer appears in the new manifest AND every
`graph_edges` row whose endpoints are now missing from `graph_nodes`.
Matching `scan_manifest` rows MUST also be removed. Deletion runs in the
same transaction as the revision open.

#### Scenario: Removed file disappears from nodes and edges

- GIVEN revision `3` has node `src/x.rs:foo:1` and an edge whose
  endpoints both live in `src/x.rs`
- WHEN revision `4` is committed WITHOUT `src/x.rs` in `scan_manifest`
- THEN `count_nodes(ws1, source_path='src/x.rs')` at rev 4 is `0`
- AND the edge is no longer present in `load_call_graph(ws1, 4)`

#### Scenario: Removed file disappears from scan_manifest

- GIVEN `scan_manifest(ws1, "src/x.rs")` exists at revision `3`
- WHEN revision `4` is committed WITHOUT `src/x.rs`
- THEN `count(*) from scan_manifest where workspace_id='ws1' and
  file_path='src/x.rs'` is `0`

#### Scenario: Other workspaces are unaffected

- GIVEN `ws1` removes `src/x.rs` AND `ws2` keeps `src/x.rs`
- WHEN revision `4` is committed for both
- THEN `ws1`'s node `src/x.rs:foo:1` is gone
- AND `ws2`'s node `src/x.rs:foo:1` is still present

## REMOVED Requirements

None.

## Out of Scope (locked)

`GraphStore` impl for PG; new async `GraphPersistence` trait; new
tables/columns/indexes other than the documented schema additions
(`graph_revisions`, workspace_id PK change, workspace-scoped unique
index); bincode/blob sidecar; explorer-PG adapter; MCP envelope;
petgraph projection; `ltree`/`pgvector`; removal of SQLite /
`cognicode-store-traits` / `GraphStore`.