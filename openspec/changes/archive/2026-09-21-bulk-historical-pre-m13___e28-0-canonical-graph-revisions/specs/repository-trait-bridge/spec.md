# Delta for repository-trait-bridge

> Round-trip equivalence pinned to one workspace + revision;
> typed-property preservation as a contract scenario.

## MODIFIED Requirements

### Requirement: cognicode-explorer SymbolRepository is the canonical explorer port

`SymbolRepository` remains the canonical explorer port.
`MetadataAwareRepository` extends it. Every metadata-aware read MUST be
pinned to `(WorkspaceId, RevisionId)` and MUST fail closed
(`Err(RepositoryError::UnknownRevision)`) when the pair does not exist.

#### Scenario: Pinned read returns snapshot for the pinned revision

- GIVEN a graph seeded for `ws1` at revision `3`
- WHEN `callees_with_metadata(&id, ws1, RevisionId(3))` is called
- THEN every entry carries the exact `(provenance, confidence)`
- AND the set MUST NOT change if a concurrent ingest advances the
  head to `4` before this call completes

### Requirement: Optional metadata on RelationTarget

`RelationTarget` keeps `pub provenance: Option<Provenance>` and
`pub confidence: Option<f64>`. `From<&ResolvedSymbol>` sets both to
`None`. The base `SymbolRepository::callees` signature is unchanged.

#### Scenario: Backward compatibility

- GIVEN a `ResolvedSymbol` and `RelationTarget::from(&resolved)`
- WHEN the conversion runs
- THEN `provenance = None, confidence = None`

### Requirement: Async-ready Repository trait in core

The `Repository` trait in `cognicode-core::domain::traits::repository`
(annotated `#[async_trait]`) MUST expose
`load_call_graph_pinned(workspace: &WorkspaceId, revision: RevisionId)
-> Result<Option<CallGraph>, RepositoryError>` returning
`Err(UnknownRevision)` when the pair does not exist.

#### Scenario: Pinned load fails closed for unknown revision

- GIVEN `ws1` exists with head `RevisionId(5)`
- WHEN `load_call_graph_pinned(ws1, RevisionId(99))` is called
- THEN the result is `Err(UnknownRevision { workspace: ws1, revision: 99 })`

#### Scenario: Trait compiles and is dyn-compatible

- GIVEN the `Repository` trait annotated with `#[async_trait]`
- WHEN `cargo check -p cognicode-core` runs
- THEN compilation succeeds AND `Box<dyn Repository>` is `Send + Sync`

### Requirement: cognicode-store-traits deprecation

`cognicode-store-traits` stays deprecated. The canonical PG port for
the multimodal / generic graph lives in `cognicode-core`.

#### Scenario: Crate still compiles

- GIVEN `cognicode-store-traits` present
- WHEN this slice lands
- THEN `cargo check --workspace` passes

### Requirement: Contract tests for MetadataAwareRepository

`cognicode-explorer/tests/metadata_aware_repository.rs` MUST also cover
revision-pinning (a read pinned to `r` MUST return the row set that
existed at `r` even after a concurrent ingest advances the head) and
typed-property preservation (a node with structured `properties`
MUST round-trip through PG and the snapshot unchanged).

#### Scenario: Revision pin survives concurrent ingest

- GIVEN a fixture saved at revision `r`
- WHEN `callees_with_metadata(id, ws, RevisionId(r))` is called AND a
  concurrent ingest advances the head to `r+1`
- THEN the returned entries reflect revision `r`'s rows
- AND the call MUST NOT block on or wait for the new head

#### Scenario: Typed JSONB properties round-trip unchanged

- GIVEN a `GraphNode` whose `properties` is
  `json!({"complexity": 12, "tags": ["auth"], "nested": {"k": "v"}})`
- WHEN persisted and re-loaded via the snapshot port
- THEN the loaded `properties` equals the original bit-for-bit

### Requirement: Migration and rollout safety

Single PR ≤ 400 lines for the bridge-touching surface. Land in order:
typed-property contract test → revision-pinned load → cross-workspace
read isolation → `MetadataAwareRepository` revision pin. Reversible
via single `git revert`.

#### Scenario: PR size budget

- GIVEN the planned bridge changes
- THEN `additions + deletions` is ≤ 400

## ADDED Requirements

### Requirement: Round-trip equivalence pinned to (workspace, revision)

For any `(g, ws, rev) = save_call_graph(g, ws)` followed by
`load_call_graph(ws, rev)` within the same process, the loaded graph
is `PartialEq`-equal to `g`. The guarantee holds even if a concurrent
ingest advances the head to `rev + 1`. Typed JSONB `properties` /
`metadata` MUST survive unchanged.

#### Scenario: Concurrent ingest does not perturb pinned read

- GIVEN `ws1` head=3
- WHEN `save_call_graph(&g, ws1)` opens revision 4 AND a concurrent
  ingest opens revision 5
- THEN `load_call_graph(ws1, RevisionId(4))` returns `g`
- AND `load_call_graph(ws1, RevisionId(5))` returns the new graph

## REMOVED Requirements

None.

## Out of Scope (locked)

PostgreSQL adapter, sqlx, schema DDL (owned by
postgres-callgraph-persistence delta); new node kinds, new edge kinds;
MCP envelope, ExplorerQL, Explorer UI; `CallGraphV1` removal, JSON
snapshots, bincode wire-format changes; removal of
`cognicode-store-traits` (deferred).