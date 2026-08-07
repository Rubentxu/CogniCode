# Spec: Repository Trait Bridge (explorer + core)

> Companion to the engram-stored full spec (`sdd/explorer-graph-repository-bridge/spec`).
> This file mirrors the requirements in LogSeq `Spec: explorer-graph-repository-bridge` and the engram observation.

## Status

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


Active. PR1 of `e28-0-canonical-graph-revisions` introduced the workspace + revision pinning.

## Findings Reshaping Scope

The proposal claimed `cognicode-store-traits` is "dead, zero dependents, identical to `GraphStore` in core". **VERIFIED FALSE**:

- `cognicode-core/Cargo.toml` line 90: `cognicode-store-traits = { workspace = true }`
- `cognicode-db/Cargo.toml` line 15: `cognicode-store-traits.workspace = true`
- `cognicode-store-traits` defines its own `CallGraph`, `Symbol`, `GraphStore`, `StoreError`, `FileManifest` (no `Provenance` metadata)
- The shape of `GraphStore` IS identical between the two crates; the types referenced by it are NOT

**Reshape**: deprecate `cognicode-store-traits` in this slice. Removal is deferred to a follow-up slice once the PostgreSQL adapter is delivered and the workspace can be migrated atomically.

## Requirements

### Requirement: cognicode-explorer SymbolRepository is the canonical explorer port

`SymbolRepository` remains the canonical explorer port. `MetadataAwareRepository` extends it. Every metadata-aware read MUST be pinned to `(WorkspaceId, RevisionId)` and MUST fail closed (`Err(RepositoryError::UnknownRevision)`) when the pair does not exist. No method on `SymbolRepository` itself changes signature. (Previously: `SymbolRepository` was the only port; metadata-aware access required an adapter-specific method outside the trait.)

#### Scenario: Pinned read returns snapshot for the pinned revision
- GIVEN a graph seeded for `ws1` at revision `3`
- WHEN `callees_with_metadata(&id, ws1, RevisionId(3))` is called
- THEN every entry carries the exact `(provenance, confidence)`
- AND the set MUST NOT change if a concurrent ingest advances the head to `4` before this call completes

#### Scenario: Opt-in sub-trait on CallGraphRepository
- GIVEN a `CallGraphRepository` and a graph seeded with mixed-provenance edges
- WHEN `callees_with_metadata(&id)` is called
- THEN every returned entry MUST carry the exact `(provenance, confidence)` assigned by `ConfidenceRules`

#### Scenario: Sub-trait not required for base consumers
- GIVEN a mock implementor of `SymbolRepository` that does NOT implement `MetadataAwareRepository`
- WHEN the mock is passed where `dyn SymbolRepository` is expected
- THEN the call MUST succeed and metadata-aware methods MUST NOT be reachable

### Requirement: Optional metadata on RelationTarget

`RelationTarget` keeps `pub provenance: Option<Provenance>` and `pub confidence: Option<f64>`. `From<&ResolvedSymbol>` sets both to `None`. The base `SymbolRepository::callees` signature is unchanged. (Previously: `RelationTarget` did not carry metadata.)

#### Scenario: Backward compatibility
- GIVEN a `ResolvedSymbol` and `RelationTarget::from(&resolved)`
- WHEN the conversion runs
- THEN `provenance = None, confidence = None`
- AND the 295 existing tests MUST pass unmodified

### Requirement: Async-ready Repository trait in core

The `Repository` trait in `cognicode-core::domain::traits::repository` (annotated `#[async_trait]`) MUST expose `load_call_graph_pinned(workspace: &WorkspaceId, revision: RevisionId) -> Result<Option<CallGraph>, RepositoryError>` returning `Err(UnknownRevision)` when the pair does not exist. (Previously: no pinned read method existed.)

#### Scenario: Pinned load fails closed for unknown revision
- GIVEN `ws1` exists with head `RevisionId(5)`
- WHEN `load_call_graph_pinned(ws1, RevisionId(99))` is called
- THEN the result is `Err(UnknownRevision { workspace: ws1, revision: 99 })`

#### Scenario: Trait compiles and is dyn-compatible
- GIVEN the `Repository` trait annotated with `#[async_trait]`
- WHEN `cargo check -p cognicode-core` runs
- THEN compilation succeeds AND `Box<dyn Repository>` is `Send + Sync`

### Requirement: cognicode-store-traits deprecation

`cognicode-store-traits` stays deprecated. The canonical PG port for the multimodal / generic graph lives in `cognicode-core`. The crate SHALL remain in the workspace and SHALL keep compiling. (Previously: deprecation was limited to docs only; PR1 enforces the new home.)

#### Scenario: Crate still compiles
- GIVEN `cognicode-store-traits` present
- WHEN this slice lands
- THEN `cargo check --workspace` passes

### Requirement: Contract tests for MetadataAwareRepository

`cognicode-explorer/tests/metadata_aware_repository.rs` MUST also cover revision-pinning (a read pinned to `r` MUST return the row set that existed at `r` even after a concurrent ingest advances the head) and typed-property preservation (a node with structured `properties` MUST round-trip through PG and the snapshot unchanged).

#### Scenario: Revision pin survives concurrent ingest
- GIVEN a fixture saved at revision `r`
- WHEN `callees_with_metadata(id, ws, RevisionId(r))` is called AND a concurrent ingest advances the head to `r+1`
- THEN the returned entries reflect revision `r`'s rows
- AND the call MUST NOT block on or wait for the new head

#### Scenario: Typed JSONB properties round-trip unchanged
- GIVEN a `GraphNode` whose `properties` is `json!({"complexity": 12, "tags": ["auth"], "nested": {"k": "v"}})`
- WHEN persisted and re-loaded via the snapshot port
- THEN the loaded `properties` equals the original bit-for-bit

#### Scenario: Golden triples
- GIVEN a fixture with `DirectExtraction`, `Heuristic{0.7}`, `Heuristic{0.4}` edges
- WHEN `callees_with_metadata()` is called
- THEN the entries MUST be `(Extracted, 1.0)`, `(Inferred, 0.7)`, `(Ambiguous, 0.3)` (f64 exact)

### Requirement: Migration and rollout safety

Single PR ≤ 400 lines for the bridge-touching surface. Land in order: typed-property contract test → revision-pinned load → cross-workspace read isolation → `MetadataAwareRepository` revision pin. Reversible via single `git revert`. (Previously: PR budget covered 6 PR units; PR1 narrowed to a single 400-line PR.)

#### Scenario: PR size budget
- GIVEN the planned bridge changes
- THEN `additions + deletions` is ≤ 400

### Requirement: Round-trip equivalence pinned to (workspace, revision)

For any `(g, ws, rev) = save_call_graph(g, ws)` followed by `load_call_graph(ws, rev)` within the same process, the loaded graph is `PartialEq`-equal to `g`. The guarantee holds even if a concurrent ingest advances the head to `rev + 1`. Typed JSONB `properties` / `metadata` MUST survive unchanged.

#### Scenario: Concurrent ingest does not perturb pinned read
- GIVEN `ws1` head=3
- WHEN `save_call_graph(&g, ws1)` opens revision 4 AND a concurrent ingest opens revision 5
- THEN `load_call_graph(ws1, RevisionId(4))` returns `g`
- AND `load_call_graph(ws1, RevisionId(5))` returns the new graph

## MODIFIED (carry-over from earlier slice)

### Requirement: cognicode-core domain::traits is the canonical core port surface

`cognicode_core::domain::traits` is the canonical home for cross-cutting ports. The new `Repository` trait is added there. `cognicode-store-traits` is marked deprecated. (Previously: `cognicode-store-traits` claimed to be canonical; Phase 1 already moved the canonical `CallGraph` into `cognicode-core`.)

## REMOVED Requirements

None.

## Out of Scope (locked)

- PostgreSQL adapter specifics (owned by `postgres-callgraph-persistence` delta)
- New node kinds, new edge kinds
- MCP envelope, ExplorerQL, Explorer UI
- `CallGraphV1` removal, JSON snapshots, bincode wire-format changes
- Removal of `cognicode-store-traits` (deferred)

## Coverage

- Happy paths: covered
- Edge cases: covered (pinned, concurrent ingest, typed JSONB round-trip, unknown revision)
- Error states: covered (`UnknownRevision`, feature gate, dyn-compatibility)

## Open Questions

None blocking.