# Design: Named Views

## Technical Approach

Add a PostgreSQL `named_views` table and four MCP tools (`view_save`, `view_load`, `view_list`, `view_delete`) that persist and retrieve user-saved graph projection tuples `(level, lens, focus_node, max_depth)`. The `view_load` tool re-dispatches to the existing `ExplorerService::contextual_view` pipeline to rebuild the live `ContextualView`. All PG interaction is behind the `postgres` feature flag; without it, all four tools return `Err("named_views_require_postgres_feature")` — no panic, no sqlx linkage.

## Architecture Decisions

| Decision | Choice | Alternatives | Rationale |
|----------|--------|-------------|-----------|
| PG access layer | Add named_view methods directly to `PostgresRepository` in cognicode-core | New standalone repo in explorer crate | Keeps pool ownership, migration runner, and error mapping in one place; explorer crate stays sqlx-free behind the feature gate |
| Feature gate surface | `ExplorerService` methods return `FeatureDisabled`; MCP dispatch returns `"named_views_require_postgres_feature"` | Compile-time only gate (cfg on dispatch) | Service-level gate allows unit testing without PG binary; MCP gate provides user-visible error message |
| view_load rebuild | Store raw tuple, re-invoke `contextual_view(object_id, view_id)` on load | Store serialized `ContextualView` blob | Rebuild guarantees the view reflects the current graph state — a stale blob would silently lie |
| Description truncation | Truncate in service layer (list_views) on the fly | Store truncated copy alongside full text | Single source of truth; truncation is display-only and cheap |

## Data Flow

```
MCP client ──→ dispatch (mcp.rs) ──→ ExplorerService ──→ PostgresRepository
                    │                        │
                    │   view_load only:       │──→ contextual_view() ──→ build_*
                    │   rebuilds ContextualView
                    │
                    └── envelope_ok / envelope_err_with_code
```

Save path: `view_save` args → validate → `repo.save_named_view()` → `Ok(NamedView)`.
Load path: `view_load` args → `repo.load_named_view()` → `service.contextual_view(focus_node, lens)` → `Ok(ContextualView)`.
List path: `view_list` args → `repo.list_named_views()` → truncate descriptions → `Ok(Vec<NamedViewDescriptor>)`.
Delete path: `view_delete` args → `repo.delete_named_view()` → `Ok({deleted: true})`.

## File Changes

| File | Action | Description | ~Lines |
|------|--------|-------------|--------|
| `crates/.../persistence/schema_postgres.sql` | Modify | Append `named_views` CREATE TABLE + unique index | +15 |
| `crates/.../persistence/postgres_repository.rs` | Modify | Add `save/load/list/delete_named_view` methods | +120 |
| `crates/cognicode-explorer/src/dto.rs` | Modify | Add `NamedView`, `NamedViewDescriptor`, `SaveNamedViewRequest` structs | +50 |
| `crates/cognicode-explorer/src/error.rs` | Modify | Add `Conflict`, `NotFound`, `FeatureDisabled`, `InvalidInput` variants | +10 |
| `crates/cognicode-explorer/src/service.rs` | Modify | Add `save_view`, `load_view`, `list_views`, `delete_view` methods + `postgres_repo` field | +80 |
| `crates/cognicode-explorer/src/mcp.rs` | Modify | 4 constants, 4 args structs, 4 dispatch arms, 4 schemas, TOOL_NAMES 24→28 | +200 |
| `crates/cognicode-explorer/src/postgres_bridge.rs` | Modify | Return `Arc<PostgresRepository>` alongside `Arc<CallGraph>` for named views | +20 |
| `crates/cognicode-explorer/src/lib.rs` | Modify | (no change — postgres_bridge already feature-gated) | 0 |
| `docs/explorer-graph/glossary.md` | Modify | Add "Named View" entry | +5 |

**Total estimate**: ~500 lines added across 8 files.

## Interfaces / Contracts

### DDL (`schema_postgres.sql` append)

```sql
CREATE TABLE IF NOT EXISTS named_views (
    id            UUID PRIMARY KEY,
    workspace_id  TEXT NOT NULL,
    owner         TEXT NOT NULL,
    name          TEXT NOT NULL,
    description   TEXT,
    level         TEXT NOT NULL,
    lens          TEXT NOT NULL,
    focus_node    TEXT NOT NULL,
    max_depth     INTEGER NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_named_views_scope
    ON named_views (workspace_id, owner, name);
```

### DTOs (`dto.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamedView {
    pub id: String,
    pub workspace_id: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub level: String,
    pub lens: String,
    pub focus_node: String,
    pub max_depth: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamedViewDescriptor {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub level: String,
    pub lens: String,
    pub focus_node: String,
    pub max_depth: i32,
    pub created_at: String,
}
```

### ExplorerError variants (`error.rs`)

```rust
#[error("conflict: {0}")]
Conflict(String),
#[error("not found: {0}")]
NotFound(String),
#[error("feature disabled: {0}")]
FeatureDisabled(String),
#[error("invalid input: {0}")]
InvalidInput(String),
```

### MCP tool constants (`mcp.rs`)

```rust
pub const TOOL_VIEW_SAVE: &str = "view_save";
pub const TOOL_VIEW_LOAD: &str = "view_load";
pub const TOOL_VIEW_LIST: &str = "view_list";
pub const TOOL_VIEW_DELETE: &str = "view_delete";
```

### MCP args structs

```rust
struct ViewSaveArgs { workspace_id, owner, name, description?, level, lens, focus_node, max_depth }
struct ViewLoadArgs { id, workspace_id, owner }
struct ViewListArgs { workspace_id, owner }
struct ViewDeleteArgs { id, workspace_id, owner }
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | NamedView serde round-trip | `to_string` + `from_str` equality |
| Unit | Input validation (empty name, negative depth, long name) | Assert `InvalidInput` before any PG call |
| Unit | Feature gate off → `FeatureDisabled` for all 4 service methods | Build without `postgres` feature |
| Unit | TOOL_NAMES count 28, no duplicates, all 24 pre-existing names present | `tool_schemas_list_twentyeight_tools` |
| Integration | Migration idempotency (DDL twice → one table, one index) | `named_views_migration_is_idempotent` |
| Integration | Unique index rejects duplicate `(workspace_id, owner, name)` | Second insert → `Conflict` |
| Integration | Save → Load round-trip produces same `ContextualView` as direct `build_*` | `view_load_returns_rebuilt_view` |
| Integration | List returns only matching scope, ordered newest-first | `view_list_orders_newest_first` |
| Integration | Delete removes row; scope mismatch returns `NotFound` without deleting | `view_delete_removes_row` |
| Integration | Workspace/owner mismatch on load/delete → `NotFound` (no existence leak) | `view_load_workspace_mismatch_returns_not_found` |

## TDD RED Gate Sequence

Tests written FIRST, MUST FAIL before implementation:

1. **`tool_schemas_list_twentyeight_tools`** — `assert_eq!(build_tool_schemas().len(), 28)` (currently 24)
2. **`tool_schemas_no_duplicate_names`** — `HashSet` from tool names == 28
3. **`tool_schemas_preserve_existing_24_names`** — pre-change 24 ⊆ new 28
4. **`named_view_serde_roundtrip`** — serialize + deserialize equality
5. **`view_save_rejects_empty_name`** — `error == "invalid_input"`
6. **`view_save_rejects_negative_max_depth`** — `error == "invalid_input"`
7. **`feature_gate_off_all_four_tools_return_soft_error`** — `error == "named_views_require_postgres_feature"`
8. **`named_views_migration_is_idempotent`** — DDL twice → single table/index (PG integration)
9. **`named_views_unique_index_rejects_duplicate_name`** — second insert → `Conflict` (PG)
10. **`view_save_happy_path_persists_row`** — `Ok(NamedView{id, …})`; row visible via repo (PG)
11. **`view_save_duplicate_returns_conflict_envelope`** — `error == "named_view_already_exists"` (PG)
12. **`view_load_returns_rebuilt_view`** — equals direct `build_*` output (PG)
13. **`view_load_unknown_id_returns_not_found`** — `error == "not_found"` (PG)
14. **`view_load_workspace_mismatch_returns_not_found`** — scope guard (PG)
15. **`view_list_returns_only_matching_scope`** — length + ownership (PG)
16. **`view_list_empty_scope_returns_ok_empty_vec`** — `Ok(vec![])` (PG)
17. **`view_list_orders_newest_first`** — insertion order reversed (PG)
18. **`view_delete_removes_row`** — `Ok({deleted:true})`; row gone (PG)
19. **`view_delete_mismatch_does_not_remove`** — `error == "not_found"`; row present (PG)
20. **`explorer_service_pg_disabled_returns_feature_disabled`** — all four methods (unit)

## Migration / Rollout

No data migration required. The `named_views` table is additive (`CREATE TABLE IF NOT EXISTS`). Rollback: remove 4 dispatch arms, revert tool-count test to 24, `DROP TABLE named_views`.

## Open Questions

- [ ] Should `view_load` also accept an optional `max_depth` override at load time, or always use the saved value? (Spec says saved value only for v1 — deferred)
- [ ] Should `ExplorerService` hold `Option<Arc<PostgresRepository>>` or should a new `NamedViewRepository` trait be introduced? (Design chooses direct field for simplicity — revisit if >1 persistence backend needed)
