# Proposal: Mermaid C4 Architecture Export

## Intent
`build_architecture_impl` already infers a C4-leveled `SubgraphResponse` (System → Containers → Components) from `Cargo.toml`, but that structure is only consumable as JSON. Users and AI agents need a copy-pasteable Mermaid C4 diagram for docs, ADRs, and PR descriptions. `CallGraph::to_mermaid()` exists in core but emits `flowchart TD` — not C4 keywords. This change adds pure text generation that maps the inferred graph to canonical Mermaid C4 syntax for the three first-class levels.

## Scope

### In Scope
- `fn c4_to_mermaid(nodes, edges, level) -> String` — pure, in explorer
- `C4Level` enum: `Context | Container | Component`
- 3 internal builders: `build_c4_context`, `build_c4_container`, `build_c4_component`
- `sanitize_mermaid_id()` helper (factored out of the existing `to_mermaid` pattern)
- MCP handler `handle_export_c4_mermaid` with `level` param, gated behind `multimodal`
- REST endpoint `GET /api/workspaces/:id/architecture/mermaid?level=context|container|component`
- Unit tests per level + ID sanitizer

### Out of Scope
- ViewExecutor wiring (C4 ViewKinds already exist; this is export-only)
- SVG/PNG rendering via mermaid-rs-renderer (no guaranteed C4 keyword support — deferred)
- CLI command (follow-up change)
- Editing/round-tripping Mermaid back into the graph

## Capabilities

> CONTRACT with sddk-spec. Researched `openspec/specs/` (39 existing capabilities).

### New Capabilities
- `c4-mermaid-export`: Pure Mermaid C4 text generation from an inferred C4 `SubgraphResponse`, exposed via REST + MCP.

### Modified Capabilities
- None. `mcp-multimodal-tools` covers `docs_ingest`/`graph_search` semantics; an export tool is additive and touches no existing requirement. `contextual-views` explicitly excludes C4 multi-level traversal.

## Approach
1. Define `C4Level` enum in the explorer facade module alongside `build_architecture_impl`.
2. `c4_to_mermaid(nodes, edges, level)` filters nodes by `NodeKind` (`System`/`Container`/`Component`) and emits Mermaid C4 keywords (`System`, `Container`, `Component`, `Rel`).
3. Factor the existing `safe_id.replace([':', '(', ')', '<', '>', '{', '}'], "_")` from `CallGraph::to_mermaid` into a shared `sanitize_mermaid_id()`.
4. REST handler calls `build_architecture_impl` then `c4_to_mermaid`; the MCP handler mirrors it under `#[cfg(feature = "multimodal")]`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/facades/graph.rs` | New | `c4_to_mermaid`, `C4Level`, 3 builders, `sanitize_mermaid_id` |
| `crates/cognicode-explorer/src/mcp.rs` | Modified | Register `export_c4_mermaid` handler (multimodal-gated) |
| `crates/cognicode-explorer` route module (`api.rs`/equivalent) | Modified | `GET /api/workspaces/:id/architecture/mermaid` route |
| `crates/cognicode-explorer/src/` test module | New | Unit tests per `C4Level` + sanitizer |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Mermaid C4 keyword drift vs. renderer support | Medium | Emit canonical keywords per ADR-003; do NOT promise SVG in v1 |
| `NodeKind` filter mismatch (inferred kind vs. spec keyword) | Low | Map by `NodeKind::as_str()`; unit test each kind |
| ID collision after sanitization (`:`-heavy FQNs) | Low | Sanitizer + dedup pass; test with `crate::module::sym` inputs |

## Rollback Plan
Delete the new functions/enum, revert the MCP registration line in `mcp.rs`, revert the route in the API module. No schema migration, no DB changes, no public API contract break — `build_architecture_impl` is unchanged. Pure additive revert on a single crate.

## Dependencies
- `cognicode-core` `NodeKind` / `EdgeKind` (read-only reuse)
- Existing `build_architecture_impl` (unchanged consumer)
- ADR-003 (Mermaid C4 keywords are canonical; SVG is derived)

## Success Criteria
- [ ] `c4_to_mermaid` returns parseable Mermaid C4 syntax for all 3 levels
- [ ] `GET /api/workspaces/:id/architecture/mermaid?level=context` returns `200` with a Mermaid text body
- [ ] MCP `export_c4_mermaid` appears in `tools/list` only when the `multimodal` feature is enabled
- [ ] Unit tests pass for each `C4Level` and for `sanitize_mermaid_id`
- [ ] Existing `tools/list` count is unchanged when the feature is off (regression gate)
