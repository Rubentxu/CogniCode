# named-views-rationale Delta Specification (MODIFIED)

> **MODIFIED capability** on top of `named-view-persistence`.
> Companion to proposal `sdd/corroboration-rationale-views/proposal`.
> The existing `view_save` / `view_load` / `view_list` / `view_delete` behavior for non-rationale lenses (`callgraph`, `overview`, etc.) is preserved unchanged.

## MODIFIED Requirements

### Requirement: `view_load` MCP tool — rationale dispatch

(Previously: `view_load` MUST return `McpResultEnvelope<ContextualView>` rebuilt from the stored projection tuple by re-invoking the existing domain `build_*` path for `(level, lens, focus_node, max_depth)`.)

`view_load` MUST accept `{ id, workspace_id, owner }`. The dispatch MUST match on `lens` as follows:

| `lens` value | Dispatch target |
|--------------|-----------------|
| `"rationale"` | `build_rationale_graph(focus_node, max_depth, max_nodes=50)` from `rationale-traversal` → returns `SubgraphResponse` |
| any other value | existing `contextual_view(focus_node, lens)` → returns `ContextualView` (preserved from prior version) |

The `SubgraphResponse` for `"rationale"` MUST be wrapped in a new envelope variant: `McpResultEnvelope<RationaleViewPayload> { ... payload: { subgraph: SubgraphResponse, corroboration_scores: HashMap<String, f64>, source_count: u32 } }`. The `max_depth` field on the saved `NamedView` MUST be passed through to `build_rationale_graph` as the depth cap. If the saved `max_depth > 5` (the rationale hard cap), the dispatch MUST clamp to 5 silently and emit a `tracing::info!` log line. Scope checks (`workspace_id` / `owner` mismatch) and `not_found` behavior MUST remain identical to the prior version. Feature-gate-off MUST still return `Err("named_views_require_postgres_feature")`. A new error variant `Err("lens_rationale_requires_multimodal_feature")` MUST be returned when `lens="rationale"` is requested on a build without `--features multimodal`.

#### Scenario: Load with lens="rationale" returns rationale payload

- GIVEN a saved `NamedView` with `id=I`, `level="function"`, `lens="rationale"`, `focus_node="crate::foo::bar"`, `max_depth=3`
- AND a build with `--features multimodal --features postgres`
- WHEN `view_load { id: I, workspace_id, owner }` is called
- THEN the envelope is `Ok(RationaleViewPayload)` whose `subgraph` equals a direct `build_rationale_graph("crate::foo::bar", 3, 50)` call
- AND `corroboration_scores` is non-empty (computed from `corroboration-scoring`)
- AND `source_count` equals the number of edges in `subgraph.edges`

#### Scenario: Load with lens="callgraph" still returns ContextualView (no regression)

- GIVEN a saved `NamedView` with `lens="callgraph"`, `focus_node="crate::foo::bar"`
- WHEN `view_load { id, workspace_id, owner }` is called
- THEN the envelope is `Ok(ContextualView)` identical to the prior behavior
- AND no `build_rationale_graph` call is dispatched

#### Scenario: Unknown id returns not_found (preserved)

- GIVEN no row with `id=I_missing`
- WHEN `view_load { id: I_missing, workspace_id, owner }` is called
- THEN the envelope is `Err` with `error == "not_found"`

#### Scenario: Workspace mismatch returns not_found (preserved)

- GIVEN a row owned by `(w1, u1)`
- WHEN `view_load` is called with `workspace_id="w2", owner="u1", id=row.id`
- THEN the envelope is `Err` with `error == "not_found"`

#### Scenario: Owner mismatch returns not_found (preserved)

- GIVEN a row owned by `(w1, u1)`
- WHEN `view_load` is called with `workspace_id="w1", owner="u2", id=row.id`
- THEN the envelope is `Err` with `error == "not_found"`

#### Scenario: Feature gate (postgres) off returns soft error (preserved)

- GIVEN a build without `--features postgres`
- WHEN `view_load` is called
- THEN the envelope is `Err` with `error == "named_views_require_postgres_feature"`

#### Scenario: Feature gate (multimodal) off with lens="rationale" returns soft error

- GIVEN a build without `--features multimodal` (with `--features postgres`)
- WHEN `view_load { id: I, ..., lens: "rationale" }` is called
- THEN the envelope is `Err` with `error == "lens_rationale_requires_multimodal_feature"`
- AND no graph traversal is attempted

#### Scenario: max_depth > 5 is clamped to 5

- GIVEN a saved `NamedView` with `max_depth=10`, `lens="rationale"`
- WHEN `view_load` is called
- THEN `build_rationale_graph(focus, 5, 50)` is invoked (depth clamped)
- AND a `tracing::info!` log line is emitted with the original and clamped depths

## ADDED Requirements

### Requirement: `RationaleViewPayload` DTO

`crates/cognicode-explorer/src/dto.rs` MUST define `RationaleViewPayload { subgraph: SubgraphResponse, corroboration_scores: HashMap<String, f64>, source_count: u32 }` with `#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]`. The `source_count` field MUST equal `subgraph.edges.len()`. The DTO MUST roundtrip through `serde_json`.

#### Scenario: DTO round-trips

- GIVEN a `RationaleViewPayload` with 4 edges and 3 score entries
- WHEN `serde_json::to_string` then `from_str` runs
- THEN the deserialized value equals the original
- AND `source_count == 4`

#### Scenario: source_count matches edges length

- GIVEN any payload
- WHEN the DTO is constructed
- THEN `source_count == subgraph.edges.len()`

### Requirement: ExplorerService `load_rationale_view` method

`ExplorerService` MUST gain a new method `load_rationale_view(id: Uuid, scope: NamedViewScope) -> Result<RationaleViewPayload, ExplorerError>`. The method MUST: (1) fetch the row by id; (2) verify `(workspace_id, owner)` match — `Err(NotFound)` on mismatch; (3) check `lens == "rationale"` — `Err(InvalidLens)` otherwise (delegates to `load_view` for non-rationale); (4) check the `multimodal` feature is enabled — `Err(FeatureDisabled)` on a no-multimodal build; (5) call `build_rationale_graph(focus, max_depth.min(5), 50)`; (6) wrap the result in `RationaleViewPayload`. The method MUST be reachable from the MCP dispatch layer.

#### Scenario: Happy path

- GIVEN a PG row + multimodal feature enabled
- WHEN `load_rationale_view(I, scope)` runs
- THEN the result is `Ok(RationaleViewPayload { subgraph, scores, source_count })`

#### Scenario: Wrong lens returns InvalidLens

- GIVEN a row with `lens="callgraph"`
- WHEN `load_rationale_view(I, scope)` runs
- THEN the result is `Err(InvalidLens)` AND the error message is `"lens must be 'rationale' for load_rationale_view"`

#### Scenario: Multimodal feature off returns FeatureDisabled

- GIVEN a multimodal-disabled build
- WHEN `load_rationale_view(I, scope)` runs
- THEN the result is `Err(FeatureDisabled("multimodal"))`

#### Scenario: Not found bubbles up unchanged

- GIVEN no row with id I
- WHEN `load_rationale_view(I, scope)` runs
- THEN the result is `Err(NotFound)`

### Requirement: Tool schema unchanged for view_load

The `view_load` MCP tool's `input_schema` MUST remain `{"id": "string", "workspace_id": "string", "owner": "string"}`. The `lens` MUST NOT be added to the input — it is read from the saved row. No new tools are introduced (this is a behavior change to `view_load`, not a new tool). The `build_tool_schemas()` count remains `28` — no regression in the existing `tool_schemas_list_twentyeight_tools` test.

#### Scenario: Schema unchanged

- GIVEN the updated `mcp.rs`
- WHEN `view_load`'s schema is enumerated
- THEN `properties` is `{"id": "string", "workspace_id": "string", "owner": "string"}` AND `required == ["id", "workspace_id", "owner"]` AND no `lens` key is present

#### Scenario: Tool count is still 28

- GIVEN the existing `tool_schemas_list_twentyeight_tools` test
- WHEN `cargo test` runs
- THEN the assertion `build_tool_schemas().len() == 28` still passes

## TDD RED Gate

These tests MUST be written FIRST and MUST FAIL before any implementation lands:

| Test | File | Asserts |
|------|------|---------|
| `view_load_lens_rationale_returns_rationale_payload` | `mcp.rs` integration | `Ok(RationaleViewPayload { … })` |
| `view_load_lens_callgraph_still_returns_contextual_view` | `mcp.rs` integration | No regression |
| `view_load_unknown_id_returns_not_found_preserved` | `mcp.rs` integration | `error == "not_found"` |
| `view_load_workspace_mismatch_returns_not_found_preserved` | `mcp.rs` integration | `error == "not_found"` |
| `view_load_owner_mismatch_returns_not_found_preserved` | `mcp.rs` integration | `error == "not_found"` |
| `view_load_postgres_feature_off_preserved` | `mcp.rs` integration | `error == "named_views_require_postgres_feature"` |
| `view_load_multimodal_feature_off_with_rationale_lens` | `mcp.rs` integration | `error == "lens_rationale_requires_multimodal_feature"` |
| `view_load_max_depth_clamped_to_5` | `mcp.rs` integration | `tracing::info!` emitted, depth=5 used |
| `rationale_view_payload_serde_round_trip` | `dto.rs` (test) | Equality after to_string + from_str |
| `rationale_view_payload_source_count_matches_edges` | `dto.rs` (test) | `source_count == edges.len()` |
| `explorer_service_load_rationale_view_happy_path` | `service.rs` (test) | Returns `Ok(RationaleViewPayload)` |
| `explorer_service_load_rationale_view_wrong_lens` | `service.rs` (test) | `Err(InvalidLens)` |
| `explorer_service_load_rationale_view_multimodal_off` | `service.rs` (test) | `Err(FeatureDisabled)` |
| `explorer_service_load_rationale_view_not_found` | `service.rs` (test) | `Err(NotFound)` |
| `view_load_input_schema_unchanged` | `mcp.rs` (test) | No `lens` key in schema |
| `tool_schemas_list_twentyeight_tools_still_passes` | `mcp.rs` (regression) | Count remains 28 |

## Out of Scope (locked)

- Per-call `lens` override on `view_load` (lens always comes from the saved row)
- New MCP tool `view_load_rationale` (would require schema count change; rejected to keep the surface stable)
- Materialized corroboration scores in the named-view row (scoring is always re-computed)
- `max_nodes` storage on the row (always `50` for rationale in v1)
- Migrating existing `lens="callgraph"` rows to a different schema (no migration needed)
