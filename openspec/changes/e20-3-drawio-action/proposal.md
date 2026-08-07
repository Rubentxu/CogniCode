# Proposal: "Open in draw.io" Action for C4 Views & Inspector

## Intent
CogniCode now emits canonical Mermaid (E20-1 C4, E20-2 traces) and renders it inline via `MermaidRenderer`, but users have no path to hand that diagram to draw.io for hand-curation. ADR-003 Rule 3 makes "Open in draw.io" a first-class action alongside Inspect/Trace/Save so diagrams become editable, curatable artifacts — the whole point of the Mermaid-canonical pipeline.

## Scope

### In Scope
- Shared `drawio.ts` util: build draw.io launch payload from Mermaid text
- "Open in draw.io" button in **C4 view toolbar** (GraphLanding C4 perspective)
- "Open in draw.io" item in **pane inspector export menu** (PaneInspector)
- "Open in draw.io" item in **investigation action menu** (gated on E21 surface)
- Copy-to-clipboard + `window.open` with `Arrange > Insert > Mermaid` guidance
- Unit tests for the URL/clipboard builder

### Out of Scope
- Custom `mxGraphModel` generation (ADR-003 Rule 2 forbids it)
- Desktop deep-linking; draw.io plugin/extension
- Persisting the draw.io result (E21-6 artifacts, separate change)

## Capabilities
> CONTRACT with sddk-spec. Researched `openspec/specs/` (39 existing capabilities).

### New Capabilities
- `drawio-open-action`: Frontend action that consumes Mermaid text from any ViewKind block (E20-1/E20-2 emitters), builds a draw.io launch payload, and opens draw.io for its built-in Mermaid import workflow.

### Modified Capabilities
- None. The action reads existing `MermaidRenderer` / `renderer-registry-frontend` outputs additively; no requirement changes. The Investigation-menu surface depends on E21 (not a spec change here).

## Approach
1. `drawio.ts`: `openInDrawio(mermaidText)` → `navigator.clipboard.writeText(mermaidText)` + `window.open("https://app.diagrams.net/")`. Inline guidance: "Mermaid copied — use Arrange > Insert > Mermaid." (ADR-003 Rule 2).
2. URL-embed variant: best-effort `?title=...#U<data:...>` — **clipboard is the reliable contract**; URL-embed is enhancement only.
3. Three UI hosts reuse the same util: C4 toolbar (GraphLanding), inspector export menu (PaneInspector header), investigation menu (IntentFooter chip / E21 menu).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `apps/explorer-ui/src/utils/drawio.ts` | New | Shared draw.io launch util + clipboard fallback |
| `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx` | Modified | C4 toolbar action button (c4_context/container/component) |
| `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` | Modified | Export-menu action entry |
| `apps/explorer-ui/src/components/Spotter/IntentFooter.tsx` | Modified | Investigation-menu entry (E21-gated) |
| `apps/explorer-ui/src/utils/drawio.test.ts` | New | Unit tests for payload + fallback |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| URL-embed blocked by length / popup blockers | Medium | Clipboard is primary contract; URL-embed best-effort only |
| Investigation menu surface not yet wired (E21) | High | Land C4 + inspector hosts first; investigation entry gated on E21 |
| `navigator.clipboard` needs secure context | Low | Document https requirement; `<textarea>` select fallback |

## Rollback Plan
Pure frontend revert: delete `utils/drawio.ts` + test, remove the three button/menu entries. No backend, no schema, no DB, no API contract change. E20-1/E20-2 emitters and `MermaidRenderer` are untouched.

## Dependencies
- E20-1 (`c4-mermaid-export`) + E20-2 trace emitters — Mermaid source (DONE)
- E21 Investigation entity — for the investigation-menu host (forward)

## Success Criteria
- [ ] "Open in draw.io" button renders in C4 toolbar for c4_context/container/component
- [ ] Clicking it copies current Mermaid to clipboard AND opens draw.io in a new tab
- [ ] Same action available in pane inspector export menu
- [ ] `drawio.test.ts` covers payload building + clipboard fallback
- [ ] Reverting the change leaves E20-1/E20-2 and renderer-registry untouched
