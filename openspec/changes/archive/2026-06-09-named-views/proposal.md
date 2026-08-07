# Proposal: Named Views

## Intent

Users navigate the graph via contextual views but cannot save projections for later reuse. Named views fill that gap: user-saved, named, link-stable snapshots of a graph projection that persist across restarts and sessions. No sharing, versioning, or editing in v1 — pure CRUD.

## Scope

### In Scope
- CRUD for named views: save, load by ID, list for workspace, delete
- PostgreSQL `named_views` table behind `postgres` feature flag
- 4 MCP tools: `view_save`, `view_load`, `view_list`, `view_delete`
- NamedView & NamedViewDescriptor DTOs in `dto.rs`
- Glossary entry in `docs/explorer-graph/glossary.md`

### Out of Scope
- Share-by-link, version history, editing, access control
- UI surface (MCP-only in v1)
- In-memory fallback persistence

## Capabilities

### New Capabilities
- **named-view-persistence**: Save/load/list/delete named graph projections to PostgreSQL. A view is a four-tuple `(level, lens, focus_node, max_depth)` plus `name`, `description`, `workspace_id`, `owner`, and `created_at`.

### Modified Capabilities
None — purely additive. Existing 24 tools, projection logic, and visualization are untouched.

## Approach

New `named_views` PG table via additive DDL (no `ALTER` on existing tables). `NamedView` struct in `dto.rs` mirrors the four-tuple projection model. `ExplorerService` gains four methods delegating to the postgres repository. MCP layer follows the existing `envelope_ok` dispatch pattern — four new arms, tool count 24→28. Feature gate: when `postgres` is off, tools return a "not available" error instead of panicking.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | New DTOs | `NamedView`, `NamedViewDescriptor` |
| `crates/cognicode-explorer/src/service.rs` | New methods | `save_view`, `load_view`, `list_views`, `delete_view` |
| `crates/cognicode-explorer/src/mcp.rs` | New dispatch | 4 tools + test assertion 24→28 |
| `crates/.../schema_postgres.sql` | New table | `named_views` DDL |
| `crates/.../postgres_repository.rs` | New methods | CRUD implementation |
| `docs/explorer-graph/glossary.md` | Modified | Named view term entry |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Scope creep (v2 features leaking into v1) | Medium | Explicit out-of-scope list; review gate |
| Feature-gate regression when PG is off | Low | Tools return error, not panic; CI tests both paths |
| Schema-DTO drift | Low | Single source struct; serde roundtrip test |

## Rollback Plan

Remove 4 MCP dispatch arms, revert tool-count test to 24. `DROP TABLE named_views`. No existing rows mutated — pure additive change.

## Dependencies

- PostgreSQL feature active (`FeatureGate::postgres`)
- Existing `McpResultEnvelope<T>` and `build_tool_schemas()` patterns

## Success Criteria

- [ ] All 4 tools return valid envelopes (save→NamedView, load→ContextualView, list→Vec<NamedViewDescriptor>, delete→{deleted:true})
- [ ] `tool_schemas_list_twentyeight_tools` test passes
- [ ] Views survive server restart (PG persistence confirmed)
- [ ] Feature-gate off: tools return error, no panic
- [ ] Existing 24 tools unchanged (regression test passes)
