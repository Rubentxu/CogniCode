# Delta for Named View Persistence

> Adds the migration path that connects the existing
> `named_views` PostgreSQL table to the new `ViewSpec` runtime
> store. The existing requirements (1–8) are unchanged. The
> delta is additive; the existing `view_save` / `view_load` /
> `view_list` / `view_delete` MCP tools keep working.

## ADDED Requirements

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

## REMOVED Requirements

None. All existing requirements (1–8) stay in place; the
runtime `view_specs` store is an additional table that
co-exists with `named_views` in v1.

(Previously: `NamedView` was the only persisted view shape.
This delta makes it a thin alias of `ViewSpec` without removing
it.)
