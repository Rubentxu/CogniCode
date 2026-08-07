# Named View Persistence

## Purpose

Persist user-saved, named graph projections so callers can reload a `(level, lens, focus_node, max_depth)` projection by stable link across restarts and sessions. Backed by a new PostgreSQL `named_views` table behind the `postgres` feature flag. Four MCP tools (`view_save`, `view_load`, `view_list`, `view_delete`) expose pure CRUD. No sharing, no versioning, no editing in v1.

## Domain Model

| Field | Type | Notes |
|-------|------|-------|
| `id` | `Uuid` (PK) | Server-generated on save. Stable link handle. |
| `workspace_id` | `String` | Non-empty. Scopes list/load/delete lookups. |
| `owner` | `String` | Non-empty. Identifies the principal that created the view. |
| `name` | `String` | Non-empty, ≤ 200 chars. Unique per `(workspace_id, owner)`. |
| `description` | `Option<String>` | Free text, ≤ 2000 chars. |
| `level` | `String` | Non-empty. Projection level identifier (e.g. `function`, `module`). |
| `lens` | `String` | Non-empty. Lens identifier (e.g. `callgraph`, `overview`). |
| `focus_node` | `String` | Non-empty. Fully-qualified focus identifier. |
| `max_depth` | `i32` | `>= 0`. Projection depth cap. |
| `created_at` | `DateTime<Utc>` | Server-assigned on insert; never mutated. |

`NamedView` (full) is the save/load round-trip shape. `NamedViewDescriptor` is the list shape: same fields minus the projection tuple internals are KEPT, but `description` is truncated to ≤ 200 chars in the list response to keep payloads lean.

## ADDED Requirements

### Requirement: `named_views` PostgreSQL table

A `named_views` table MUST be created by additive DDL (`CREATE TABLE IF NOT EXISTS`) on every startup when the `postgres` feature is active. MUST NOT `ALTER` any existing table. MUST contain columns matching the Domain Model above with types: `id UUID PRIMARY KEY`, `workspace_id TEXT NOT NULL`, `owner TEXT NOT NULL`, `name TEXT NOT NULL`, `description TEXT`, `level TEXT NOT NULL`, `lens TEXT NOT NULL`, `focus_node TEXT NOT NULL`, `max_depth INTEGER NOT NULL`, `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`. A unique index `UNIQUE (workspace_id, owner, name)` MUST exist.

#### Scenario: Migration is idempotent

- GIVEN a PG database with the table already present
- WHEN the explorer binary starts with `--postgres`
- THEN startup MUST NOT fail and MUST NOT duplicate the table or index

#### Scenario: Unique index rejects duplicate names per scope

- GIVEN a row `(workspace="w1", owner="u1", name="hotspots")` exists
- WHEN a second insert with the same `(w1, u1, "hotspots")` runs
- THEN the insert MUST fail with a unique-violation error surfaced as `Err(ExplorerError::Conflict)`

#### Scenario: Distinct owners may share a name

- GIVEN a row `(workspace="w1", owner="u1", name="hotspots")` exists
- WHEN a row `(workspace="w1", owner="u2", name="hotspots")` is inserted
- THEN the second insert MUST succeed

### Requirement: `NamedView` and `NamedViewDescriptor` DTOs

`crates/cognicode-explorer/src/dto.rs` MUST define `NamedView` and `NamedViewDescriptor` with `serde::{Serialize, Deserialize}`. `NamedView` MUST contain all Domain Model fields. `NamedViewDescriptor` MUST contain `id`, `workspace_id`, `owner`, `name`, `description` (truncated to 200 chars in the list response, with original ellipsis `"…"` appended if truncated), `level`, `lens`, `focus_node`, `max_depth`, `created_at`. Both MUST derive `Debug, Clone, PartialEq, Eq`.

#### Scenario: Serde round-trip preserves all fields

- GIVEN a fully populated `NamedView`
- WHEN `serde_json::to_string` then `serde_json::from_str` runs
- THEN the deserialized value MUST equal the original (`assert_eq!`)

#### Scenario: List descriptor truncates long descriptions

- GIVEN a saved view with a 1500-char description
- WHEN `view_list` is called for that workspace
- THEN the returned `NamedViewDescriptor.description` MUST be ≤ 201 chars and end with `…`

#### Scenario: List descriptor preserves short descriptions verbatim

- GIVEN a saved view with a 50-char description
- WHEN `view_list` is called
- THEN `description` is returned unchanged (no truncation, no ellipsis)

### Requirement: `view_save` MCP tool

`view_save` MUST accept `{ workspace_id, owner, name, description?, level, lens, focus_node, max_depth }`. MUST validate: `workspace_id`, `owner`, `name`, `level`, `lens`, `focus_node` non-empty; `name` ≤ 200 chars; `description` ≤ 2000 chars; `max_depth >= 0`. On success MUST return `McpResultEnvelope<NamedView>` with a server-generated `id` and `created_at`. On a unique violation against `(workspace_id, owner, name)` MUST return `McpResultEnvelope { ok: false, error: Some("named_view_already_exists") }`. All non-`postgres` builds MUST return `McpResultEnvelope { ok: false, error: Some("named_views_require_postgres_feature") }` — never panic.

#### Scenario: Happy path save returns full `NamedView`

- GIVEN a PG-connected explorer with empty `named_views`
- WHEN `view_save` is called with valid inputs and `name="hotspots"`, `level="function"`, `lens="callgraph"`, `focus_node="crate::foo::bar"`, `max_depth=3`
- THEN the envelope is `Ok(NamedView { id, name: "hotspots", … })`; the row exists in PG; a second `view_load(id)` returns the same projection tuple

#### Scenario: Duplicate name returns conflict envelope

- GIVEN a saved view `(w1, u1, "hotspots")`
- WHEN `view_save` is called again with the same `(w1, u1, "hotspots")` and any other fields
- THEN the envelope is `Err` with `error == "named_view_already_exists"` and NO new row is inserted

#### Scenario: Empty name rejected before hitting PG

- GIVEN a PG-connected explorer
- WHEN `view_save` is called with `name=""`
- THEN the envelope is `Err` with `error == "invalid_input"` and NO row is inserted

#### Scenario: Negative `max_depth` rejected

- GIVEN a PG-connected explorer
- WHEN `view_save` is called with `max_depth=-1`
- THEN the envelope is `Err` with `error == "invalid_input"`

#### Scenario: Name > 200 chars rejected

- GIVEN a 201-char `name`
- WHEN `view_save` is called
- THEN the envelope is `Err` with `error == "invalid_input"`

#### Scenario: Feature gate off returns soft error

- GIVEN a build without `--features postgres`
- WHEN `view_save` is called
- THEN the envelope is `Err` with `error == "named_views_require_postgres_feature"` and the process does NOT panic

### Requirement: `view_load` MCP tool

`view_load` MUST accept `{ id, workspace_id, owner }`. MUST return `McpResultEnvelope<ContextualView>` rebuilt from the stored projection tuple by re-invoking the existing domain `build_*` path for `(level, lens, focus_node, max_depth)`. MUST require the row's `workspace_id` and `owner` to match the caller-supplied scope; mismatch MUST return `Err` with `error == "not_found"`. Unknown `id` MUST return `Err` with `error == "not_found"`. Feature-gate-off MUST return `Err` with `error == "named_views_require_postgres_feature"`.

#### Scenario: Load by id returns the rebuilt view

- GIVEN a saved `NamedView` with `id=I`, `level="function"`, `lens="callgraph"`, `focus_node="crate::foo::bar"`, `max_depth=3`
- WHEN `view_load { id: I, workspace_id, owner }` is called
- THEN the envelope is `Ok(ContextualView)` whose blocks equal what a direct `build_callgraph("crate::foo::bar", 3, repo)` call produces

#### Scenario: Unknown id returns not_found

- GIVEN no row with `id=I_missing`
- WHEN `view_load { id: I_missing, workspace_id, owner }` is called
- THEN the envelope is `Err` with `error == "not_found"`

#### Scenario: Workspace mismatch returns not_found (no leak)

- GIVEN a row owned by `(w1, u1)`
- WHEN `view_load` is called with `workspace_id="w2", owner="u1", id=row.id`
- THEN the envelope is `Err` with `error == "not_found"` (the same envelope as a truly missing id — no existence leak)

#### Scenario: Owner mismatch returns not_found

- GIVEN a row owned by `(w1, u1)`
- WHEN `view_load` is called with `workspace_id="w1", owner="u2", id=row.id`
- THEN the envelope is `Err` with `error == "not_found"`

#### Scenario: Feature gate off returns soft error

- GIVEN a build without `--features postgres`
- WHEN `view_load` is called
- THEN the envelope is `Err` with `error == "named_views_require_postgres_feature"`

### Requirement: `view_list` MCP tool

`view_list` MUST accept `{ workspace_id, owner }`. MUST return `McpResultEnvelope<Vec<NamedViewDescriptor>>` containing every saved view matching `(workspace_id, owner)`, ordered by `created_at DESC`. MUST return an empty `Vec` (not `Err`) when no rows match. Feature-gate-off MUST return `Err` with `error == "named_views_require_postgres_feature"`.

#### Scenario: List returns rows for the scope

- GIVEN 3 rows `(w1, u1, "a", "b", "c")` and 1 row `(w1, u2, "d")`
- WHEN `view_list { workspace_id: "w1", owner: "u1" }` is called
- THEN the envelope is `Ok(vec)` of length 3; none of the 3 have `owner == "u2"`

#### Scenario: Empty scope returns empty vec, not error

- GIVEN no rows for `(w2, u9)`
- WHEN `view_list { workspace_id: "w2", owner: "u9" }` is called
- THEN the envelope is `Ok(vec![])` — NOT an error envelope

#### Scenario: Order is newest-first

- GIVEN rows inserted in order `a` (oldest), `b`, `c` (newest)
- WHEN `view_list` is called
- THEN the returned `vec[0].name == "c"`, `vec[1].name == "b"`, `vec[2].name == "a"`

#### Scenario: Feature gate off returns soft error

- GIVEN a build without `--features postgres`
- WHEN `view_list` is called
- THEN the envelope is `Err` with `error == "named_views_require_postgres_feature"`

### Requirement: `view_delete` MCP tool

`view_delete` MUST accept `{ id, workspace_id, owner }`. MUST require the row's `workspace_id` and `owner` to match; mismatch MUST return `Err` with `error == "not_found"`. On successful delete MUST return `McpResultEnvelope<{ deleted: bool }>` with `deleted: true`. Unknown `id` MUST return `Err` with `error == "not_found"`. Feature-gate-off MUST return `Err` with `error == "named_views_require_postgres_feature"`.

#### Scenario: Delete removes the row

- GIVEN a saved view with `id=I`
- WHEN `view_delete { id: I, workspace_id, owner }` is called
- THEN the envelope is `Ok({ deleted: true })`; a follow-up `view_load { id: I, … }` returns `not_found`; a `SELECT … FROM named_views WHERE id=I` returns 0 rows

#### Scenario: Delete is idempotent at the not_found layer

- GIVEN no row with `id=I_missing`
- WHEN `view_delete { id: I_missing, workspace_id, owner }` is called
- THEN the envelope is `Err` with `error == "not_found"`

#### Scenario: Workspace/owner mismatch returns not_found, does NOT delete

- GIVEN a row owned by `(w1, u1)`
- WHEN `view_delete { id: row.id, workspace_id: "w2", owner: "u1" }` is called
- THEN the envelope is `Err` with `error == "not_found"`; the row is still present in PG

#### Scenario: Feature gate off returns soft error

- GIVEN a build without `--features postgres`
- WHEN `view_delete` is called
- THEN the envelope is `Err` with `error == "named_views_require_postgres_feature"`

### Requirement: Tool count 24 → 28 with regression test

`build_tool_schemas()` MUST list exactly 28 tools after this change. The existing regression test `tool_schemas_list_twentyfour_tools` MUST be renamed to `tool_schemas_list_twentyeight_tools` and its assertion updated to `assert_eq!(build_tool_schemas().len(), 28)`. The 4 new tool names MUST be present in `build_tool_schemas()`: `view_save`, `view_load`, `view_list`, `view_delete`. The pre-existing 24 tool names MUST remain present (no rename, no removal).

#### Scenario: Schema count test passes post-change

- GIVEN the updated source
- WHEN `cargo test -p cognicode-explorer tool_schemas_list_twentyeight_tools` runs
- THEN the assertion passes (count == 28) and the test name compiles

#### Scenario: New tool names are registered

- GIVEN the updated source
- WHEN `build_tool_schemas().iter().map(|t| &t.name).collect::<Vec<_>>()` is called
- THEN the vec contains `"view_save"`, `"view_load"`, `"view_list"`, `"view_delete"` (each exactly once)

#### Scenario: Existing 24 tool names are preserved

- GIVEN a snapshot of the 24 tool names before this change
- WHEN `build_tool_schemas()` is called after the change
- THEN the snapshot's 24 names are all present in the new 28-element list

### Requirement: ExplorerService delegation

`ExplorerService` MUST gain four methods — `save_view`, `load_view`, `list_views`, `delete_view` — that delegate to the postgres repository under `#[cfg(feature = "postgres")]` and return `Err(ExplorerError::FeatureDisabled)` otherwise. The four methods MUST be reachable from the MCP dispatch layer.

#### Scenario: PG-enabled service round-trips a save+load

- GIVEN a `ExplorerService` constructed with a `PostgresRepository`
- WHEN `save_view(input)` is called and then `load_view(saved.id, scope)` is called
- THEN the loaded `ContextualView` equals what a direct domain `build_*` call produces for the stored tuple

#### Scenario: PG-disabled service returns FeatureDisabled for all four methods

- GIVEN a `ExplorerService` built without the `postgres` feature
- WHEN any of `save_view`, `load_view`, `list_views`, `delete_view` is called
- THEN each returns `Err(ExplorerError::FeatureDisabled)`; no PG connection is opened

## Edge Cases

- **PG unreachable mid-call** — the call returns `Err(ExplorerError::Storage(...))`; the binary does NOT crash and the process keeps serving other tools.
- **Empty `name` / `description` / etc.** — input validation rejects BEFORE any PG call; no row is inserted.
- **Workspace/owner scoping on load and delete** — treated identically to "not found" to avoid leaking existence to the wrong principal.
- **Concurrent saves with the same `(workspace_id, owner, name)`** — one wins, the other gets `Conflict`; no partial / duplicated row.
- **Long descriptions in list responses** — truncated to 200 chars + `…`; the full text is preserved in the saved row and returned by `view_load` flows that surface it (load returns the full `NamedView` context, not the descriptor).
- **Feature gate off + caller passes valid input** — soft `Err`, no panic, no PG call attempted, no sqlx symbol linked.
- **Migration on a database with stale rows** — `IF NOT EXISTS` keeps existing data; no destructive `ALTER`.
- **Tool name collisions** — the 4 new tool names MUST be distinct from every existing name; a unit test asserts no duplicates in `build_tool_schemas()`.

## TDD RED Gate

These tests MUST be written FIRST and MUST FAIL before any implementation lands:

| Test | File | Asserts |
|------|------|---------|
| `named_views_migration_is_idempotent` | `crates/cognicode-explorer/src/postgres_bridge.rs` (test) | Running the DDL twice yields one table and one unique index |
| `named_views_unique_index_rejects_duplicate_name` | `postgres_repository.rs` (test, `#[sqlx::test]`) | Second insert with same `(ws, owner, name)` returns `Conflict` |
| `named_view_serde_roundtrip` | `dto.rs` (test) | `to_string` + `from_str` equality |
| `view_save_happy_path_persists_row` | `mcp.rs` integration | `Ok(NamedView{id, …})` envelope; row visible via repo |
| `view_save_duplicate_returns_conflict_envelope` | `mcp.rs` integration | `error == "named_view_already_exists"`; no second row |
| `view_save_rejects_empty_name` | `mcp.rs` integration | `error == "invalid_input"`; no row |
| `view_save_rejects_negative_max_depth` | `mcp.rs` integration | `error == "invalid_input"` |
| `view_load_returns_rebuilt_view` | `mcp.rs` integration | Equals direct `build_*` output |
| `view_load_unknown_id_returns_not_found` | `mcp.rs` integration | `error == "not_found"` |
| `view_load_workspace_mismatch_returns_not_found` | `mcp.rs` integration | `error == "not_found"`; row untouched |
| `view_list_returns_only_matching_scope` | `mcp.rs` integration | Length and ownership match |
| `view_list_empty_scope_returns_ok_empty_vec` | `mcp.rs` integration | `Ok(vec![])` |
| `view_list_orders_newest_first` | `mcp.rs` integration | Insertion order reversed |
| `view_delete_removes_row` | `mcp.rs` integration | `Ok({deleted:true})`; row gone; load returns `not_found` |
| `view_delete_mismatch_does_not_remove` | `mcp.rs` integration | `error == "not_found"`; row present |
| `feature_gate_off_all_four_tools_return_soft_error` | `mcp.rs` integration (no `--features postgres`) | Each tool returns `error == "named_views_require_postgres_feature"`; process does not panic |
| `tool_schemas_list_twentyeight_tools` | `mcp.rs` (test) | `assert_eq!(build_tool_schemas().len(), 28)` |
| `tool_schemas_no_duplicate_names` | `mcp.rs` (test) | All 28 names distinct |
| `tool_schemas_preserve_existing_24_names` | `mcp.rs` (test) | Pre-change name set ⊆ new name set |
| `explorer_service_pg_disabled_returns_feature_disabled` | `service.rs` (test) | All four methods → `Err(FeatureDisabled)` |

### Requirement: 9. `NamedView` ↔ `ViewSpec` shape compat

The `NamedView` struct MUST gain an optional
`view_spec_compat: bool` field, defaulting to `true` on rows
written by the v1 codebase and `false` on legacy rows from before
this change. The field is `#[serde(default, skip_serializing_if =
"Option::is_none")]` so the wire format is unchanged for legacy
readers.

A new `NamedView::to_view_spec() -> ViewSpec` constructor MUST
map the existing 4-tuple `(level, lens, focus_node, max_depth)`
to a `ViewSpec` with:

| `NamedView` | → `ViewSpec` |
|-------------|--------------|
| `id` | `id` |
| `name` | `title` |
| `level` | `applies_to` (resolved via the level → `InspectableObjectType` map below) |
| `lens` | `view_kind` (resolved via the lens → `ViewKind` map below) |
| `focus_node` | encoded in `props.focus_node` |
| `max_depth` | encoded in `props.max_depth` |
| `data_source` | `DataSource::Moldql { query: "" }` (filled in by the user later via the authoring wizard) |
| `renderer_kind` | `RendererKind::Json` (the safest fallback; the authoring wizard upgrades it) |

Level → `InspectableObjectType` map:
`function|method` → `Symbol`; `file` → `File`; `module|scope` →
`Scope`; `system` → `Workspace`; unknown → `Symbol` (safe
default).

Lens → `ViewKind` map:
`callgraph` → `CallGraph`; `overview` → `Overview`-equivalent
(uses `Custom("overview")` until the enum grows a variant);
`quality` → `QualityHotspots`; unknown → `ViewKind::Custom(lens)`
(preserves the original id).

#### Scenario: Round-trip through to_view_spec

- GIVEN a `NamedView { id, name: "hotspots", level: "function",
  lens: "callgraph", focus_node: "crate::foo::bar", max_depth: 3,
  … }`
- WHEN `to_view_spec()` runs
- THEN the result is `ViewSpec { id, title: "hotspots",
  applies_to: Symbol, view_kind: CallGraph,
  data_source: Moldql { query: "" },
  renderer_kind: Json, props: { focus_node: "crate::foo::bar",
  max_depth: 3 }, … }`
- AND no fields are dropped

#### Scenario: Unknown lens becomes Custom

- GIVEN `NamedView { lens: "experimental_lens", ... }`
- WHEN `to_view_spec()` runs
- THEN `view_kind` is `ViewKind::Custom("experimental_lens")`
- AND no error is raised

### Requirement: 10. Auto-conversion on read

`ExplorerService::load_view(id, scope)` MUST return
`McpResultEnvelope<ContextualView>` exactly as it does today, and
MUST additionally return the converted `ViewSpec` in a new
`McpResultEnvelope<LoadViewCompat> { view: ContextualView,
view_spec: ViewSpec }` shape. The legacy `view_load` MCP tool
keeps its `ContextualView` return type for backward compatibility;
a new `view_spec_get` tool returns the `ViewSpec` shape.

The runtime `view_spec` table (Phase 2) MAY be populated lazily:
on the first `view_spec_get` for a row that exists in
`named_views` but not in `view_specs`, the service MUST
auto-convert and persist the `ViewSpec` (idempotent: same id
twice is a no-op).

#### Scenario: Legacy row auto-converts on first read

- GIVEN a `named_views` row with id `V` and no matching
  `view_specs` row
- WHEN `view_spec_get { id: V, workspace_id, owner }` is called
- THEN the service converts the row via `to_view_spec()`,
  inserts a matching `view_specs` row, and returns the
  `ViewSpec` envelope
- AND a second call does NOT trigger a second insert (idempotent)

#### Scenario: view_load still works

- GIVEN the same legacy row
- WHEN the legacy `view_load { id: V, ... }` is called
- THEN the response is the same `ContextualView` it was before
  the change
- AND no `ViewSpec` is included in the response body

### Requirement: 11. Deprecation timeline

`NamedView` and the four legacy `view_*` MCP tools MUST be
marked `#[deprecated(note = "use ViewSpec / view_spec_*")]` in
Rust and `/** @deprecated ... */` in TS. The deprecation message
MUST name the replacement. The deprecation is **soft**: the
tools keep working in v1 (Phase 0–5) and are removed in v2.

The MCP tool registry (`build_tool_schemas()`) MUST keep the
four legacy names in the schema list; the assertion in
`tool_schemas_list_twentyeight_tools` (or its successor) MUST
NOT shrink. New `view_spec_*` tools are added in addition, not
in replacement.

#### Scenario: Rust deprecation attribute

- GIVEN `pub struct NamedView` in `dto.rs`
- WHEN `cargo doc --no-deps` runs after the change
- THEN the generated docs page for `NamedView` shows
  `**Deprecated**: use ViewSpec / view_spec_*`

#### Scenario: MCP tool count stays the same in v1

- GIVEN the new `view_spec_*` tools (added in Phase 2)
- WHEN `build_tool_schemas().len()` is asserted
- THEN the count is `28 + 4 = 32` (or whatever Phase 2 ships);
  the four legacy `view_*` tools are still present

### Requirement: 12. Migration script (one-shot)

The system MUST provide a one-shot migration script
`scripts/migrate_named_views_to_view_specs.sh` that:

1. Reads every row from `named_views`.
2. Calls `to_view_spec()` to produce the equivalent
   `ViewSpec`.
3. Inserts the result into `view_specs` (skip on unique
   violation — idempotent).
4. Prints `migrated=N, skipped=M` and exits 0 on success.

The script is **not** run automatically; it is a manual
operator step before the v2 cutover. The script MUST be
covered by an integration test that runs against a transient
Postgres container and asserts the row counts after the run.

#### Scenario: Script is idempotent

- GIVEN a Postgres with 5 `named_views` rows
- WHEN the migration script is run twice
- THEN after the first run, `view_specs` has 5 rows
- AND after the second run, `view_specs` still has 5 rows
  (all inserts hit the unique violation; no duplicates)

#### Scenario: Script exit code is 0

- GIVEN any number of input rows (including 0)
- WHEN the script finishes
- THEN `$? == 0`

## Out of Scope (locked for v1)

- Sharing views across users / share-by-link
- Version history, edit-in-place, rename
- Access control / ACLs beyond the per-call `(workspace_id, owner)` scope check
- In-memory or SQLite fallback persistence
- UI surface (MCP-only in v1)
- Bulk import / export
- Tags, folders, favorites
- Soft delete / trash / restore
- Telemetry, audit log, rate limiting
