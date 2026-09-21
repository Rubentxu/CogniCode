# Proposal: Canonical Graph Revisions (E28.0)

## Intent
E28 (MoldQL Pattern Profile + Graph Analytics, ADR-014) cannot ship until graph identity, workspace isolation, typed property round-trip, immutable revisions, deletion, and snapshot refresh are stable. Verified defects: `graph_nodes.id` is a global PK (`workspace_id` excluded); edge uniqueness ignores workspace; `GraphNode.properties`/`GraphEdge.metadata` are `HashMap<String,String>` while PG uses JSONB (structured data lost on round-trip); `NodeKind::as_str()` returns `"symbol"` for all sub-kinds (Display/FromStr not inverse); no canonical revision table exists (only in-memory `VersionedGraphCache`); `notify_graph_change` covers nodes but not edges; post-ingest repositories may read a stale `Arc<CallGraph>`. E28.0 is the prerequisite foundation.

## Scope

### In Scope
- Immutable canonical graph revisions in PostgreSQL (`RevisionId`, `graph_revisions` table, pin semantics)
- Workspace-scoped identity and uniqueness for nodes and edges
- Typed JSONB property/metadata round-trip (domain ↔ PG ↔ snapshot, no flattening)
- Deletion completeness (removed files vanish from nodes, edges, manifest)
- Snapshot refresh contract (post-ingest query observes the new revision-pinned snapshot)

### Out of Scope
- MoldQL Pattern Profile grammar (E28.1)
- Graph analytics registry and algorithms (E28.2+)
- Neo4j CI oracle wiring
- `GraphTopology` / `FlowTrace` domain contracts
- Frontend, MCP, or REST surface changes

## Capabilities

> CONTRACT with sddk-spec. Researched `openspec/specs/` before filling.

### New Capabilities
- `graph-revisions`: immutable canonical graph revisions in PG; `RevisionId` value object; a revision is opened on ingest commit; every read pins one workspace + revision and fails closed if the revision is unknown.
- `graph-snapshot-refresh`: snapshot lifecycle tied to revision; repositories consume a `SnapshotProvider` (replaces fixed `Arc<CallGraph>`); PG notification covers edge-only changes; post-ingest refresh is observable without process restart.

### Modified Capabilities
- `generic-graph-model`: node/edge identity becomes workspace-scoped and round-trip-stable; `properties`/`metadata` upgrade from `HashMap<String,String>` to typed JSONB (`serde_json::Value`) surviving PG ↔ snapshot without loss; `NodeKind` Display/FromStr becomes inverse for symbol sub-kinds.
- `postgres-callgraph-persistence`: `save_call_graph`/`load_call_graph` become workspace-scoped and revision-aware; `graph_nodes` PK and `graph_edges` uniqueness include `workspace_id`; deletion of removed files is complete and manifest-consistent.
- `repository-trait-bridge`: PG ↔ snapshot round-trip equivalence is pinned to one workspace + revision; typed-property preservation is a contract scenario, not an implementation detail.

## Approach
Add `graph_revisions(workspace_id, revision_id, created_at, head_of)` with monotonic revision IDs per workspace. Ingest commit opens a revision; reads accept `(workspace, revision)`. Repositories consume a `SnapshotProvider` serving the pinned `CallGraph`. Extend `notify_graph_change` to `graph_edges`. Upgrade `properties`/`metadata` to `serde_json::Value`. Stabilize `NodeKind` inversibility.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/value_objects/node_kind.rs` | Modified | Inverse Display/FromStr for symbol sub-kinds |
| `crates/cognicode-core/src/domain/aggregates/generic_graph.rs` | Modified | NodeId stabilization; typed JSONB properties/metadata |
| `crates/cognicode-core/src/infrastructure/persistence/m0010_pipeline_schema.sql` | Modified | workspace_id in PK/uniqueness; graph_revisions table; edge trigger |
| `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` | Modified | Workspace-scoped + revision-aware save/load; deletion completeness |
| `crates/cognicode-core/src/infrastructure/graph/graph_cache.rs` | Modified | SnapshotProvider replaces fixed Arc |
| `crates/cognicode-core/src/infrastructure/graph/checkpoint.rs` | Modified | Revision-pinned snapshot reads |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| PK/uniqueness migration on populated DB | Med | Additive migration + data-backfill guard; companion down-migration restores old index |
| Typed-property upgrade breaks existing `HashMap` consumers | Med | Adapter layer + contract tests pin round-trip before behavioral change |
| Edge notify trigger causes notification storm | Low | Debounce/batch in listener |

## Rollback Plan
Single `git revert`. Schema migrations are additive (`graph_revisions` is a new table; index changes are guarded). A companion down-migration restores the pre-E28.0 `(source_id, target_id, kind)` unique index and drops `graph_revisions`. Re-running migrations on a reverted build MUST NOT alter existing rows.

## Dependencies
- ADR-014 (PROPOSED) — canonical state and execution scope
- Existing `VersionedGraphCache` / `CheckpointId` (in-memory) — reused as snapshot provider backbone
- `scan_manifest` (ADR-017/020) — deletion completeness source

## Success Criteria
- [ ] Same node/edge identity round-trips PG ↔ snapshot with typed-property equality (no flattening)
- [ ] Two workspaces with homonymous symbols do not collide
- [ ] Every read pins one workspace + revision; a revision-pinned run survives a concurrent ingest
- [ ] A removed file disappears from `graph_nodes`, `graph_edges`, and `scan_manifest`
- [ ] A post-ingest query observes the new snapshot without process restart
