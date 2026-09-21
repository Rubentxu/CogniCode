# Proposal: SVG/PNG Snapshot Export for Static Documentation

## Intent
CogniCode emits canonical Mermaid for C4 (E20-1) and traces (E20-2). ADR-003 Rule 1 makes "SVG/PNG snapshot (static documentation)" the final pipeline leg, but there is no path to raster/vector output for docs, ADRs, or PR descriptions. This change renders Mermaid text to PNG/SVG so diagrams become embeddable, reviewable artifacts.

## Scope

### In Scope
- Backend render service: Mermaid → PNG/SVG via `mermaid-cli` (`mmdc`)
- REST `GET /api/workspaces/:id/snapshot?view_kind=&target=&format=png|svg`
- MCP tool `export_snapshot(view_kind, target, format)` (multimodal-gated)
- "Download as PNG/SVG" button in C4 toolbar + pane inspector export menu
- Unit tests for the render invocation + format selection

### Out of Scope
- Live PNG/SVG preview in UI; custom themes/styling; server-side snapshot caching

## Capabilities
> CONTRACT with sddk-spec. Researched `openspec/specs/` (39 existing capabilities) + sibling changes e20-1/e20-3.

### New Capabilities
- `diagram-snapshot-export`: Renders Mermaid text (from E20-1/E20-2 emitters) to PNG/SVG via backend `mmdc`; exposed via REST + MCP. Produces static documentation artifacts; consumes existing Mermaid output.

### Modified Capabilities
- None. Additive REST endpoint + MCP tool + frontend actions; reads existing Mermaid emitters without changing requirements. `mcp-multimodal-tools` export tools remain additive.

## Approach
1. New domain module `snapshot.rs`: `render_mermaid(text, format) -> Vec<u8>` — writes Mermaid to a temp file, invokes `mmdc -i in.mmd -o out.{svg|png}` via `tokio::process::Command`, reads bytes.
2. REST handler resolves `view_kind`+`target` to Mermaid (reuse E20-1/E20-2 emitters), then calls `render_mermaid`; returns `image/svg+xml` or `image/png`.
3. MCP `export_snapshot` mirrors under `#[cfg(feature = "multimodal")]`.
4. Frontend `snapshot.ts` triggers a browser download from the endpoint; buttons reuse the draw.io action hosts (GraphLanding + PaneInspector).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/domain/snapshot.rs` | New | `render_mermaid` service, `SnapshotFormat` enum |
| `crates/cognicode-explorer/src/mcp.rs` | Modified | Register `export_snapshot` (multimodal-gated) |
| `crates/cognicode-explorer` route module (`api.rs`) | Modified | `GET .../snapshot` route |
| `apps/explorer-ui/src/utils/snapshot.ts` | New | Download trigger |
| `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx` | Modified | C4 toolbar download button |
| `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` | Modified | Export-menu entry |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `mmdc` needs Node.js + Chromium in container | High | Add to Dockerfile; document as required dep; pre-warm Puppeteer |
| Render latency (headless Chromium per request) | Medium | Temp-file lifecycle; future cache; cap concurrency |
| `mermaid-cli` version / C4 keyword drift | Low | Pin `mermaid-cli` version; smoke-test C4 levels |
| Container image size bloat | Medium | Multi-stage build; Chromium only in render stage |

## Rollback Plan
Delete `domain/snapshot.rs`, revert MCP registration in `mcp.rs`, revert the snapshot route, delete `utils/snapshot.ts` + the two button/menu entries. No schema, no DB, no public API contract break. E20-1/E20-2 emitters and `MermaidRenderer` are untouched.

## Dependencies
- `mermaid-cli` (`mmdc`) + Chromium/Puppeteer in the runtime image
- E20-1 `c4-mermaid-export` + E20-2 trace emitters (DONE)
- ADR-003 Rule 1 (Mermaid → SVG/PNG snapshot is the final pipeline leg)

## Success Criteria
- [ ] `GET .../snapshot?view_kind=c4_context&format=svg` returns valid SVG
- [ ] `format=png` returns a `200` `image/png` body
- [ ] MCP `export_snapshot` appears in `tools/list` only with the `multimodal` feature
- [ ] "Download as PNG/SVG" works from C4 toolbar + inspector export menu
- [ ] Reverting leaves E20-1/E20-2 emitters + renderer-registry untouched
