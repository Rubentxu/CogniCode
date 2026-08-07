# Kernel Design: Fix Pre-Existing TS Build Failure

## Context Reuse Check
| Input | Status | Notes |
|-------|--------|-------|
| Knowledge coverage | present | Explore #2674 (59 errors/28 files) + Proposal #2676 (Rust verification state (b)) |
| Exploration | present | Per-error analysis, clusters A-L |
| Proposal/spec alignment | ok | Proposal recommended 10 commits; design refines to 11-13 (see handlers.ts gap) |
| Code verification | ok | api.rs:65/101/102/103, schemas.ts L962-995, PaneInspector L247-251, rendererRegistry L140/L155, viewKind.ts L31 — all read; `tsc -b` re-run confirms 59 |
| Context quality | C2 | durable code reading + Rust source; effort = verify (DONE) |
| Problem taxonomy | present | schema-sync, type-drift, dead-code, mock-divergence |
| Domain language | present | style_class taxonomy (node-/edge-), ViewportState, RuntimeContext |
| Recommended effort | verify | drove mechanical depth + one careful seam (Cluster F) |

> **Verification note (persona discipline):** the caller's affected-files list
> omits `src/mocks/handlers.ts(93,45)` (`last_scan_at` not on workspace type).
> It is a real error in the 59. Design adds it as **Cluster M**. The bench
> cluster is ~27 in a fresh `tsc -b` (proposal said 28); exact count is
> confirmed at apply time, not a design blocker.

## Technical Approach
Mechanical type-synchronization PR. No domain modeling, no API contract change.
Two real seams: (1) Rust↔TS style_class divergence (confirmed runtime bug — Zod
rejects valid C4 payloads), (2) PaneInspector registry call drops `RuntimeContext`
(functional bug, not only a type error). Everything else is drift cleanup.

## Knowledge Impact
- Durable reused: explore #2674, proposal #2676, ADR-039 (C4 code node), CONTEXT.md style_class taxonomy.
- May become stale: stylesheet.ts `ResolvedNodeStyleClass` (L392-407) — it already lists `node-code`; after schema widen they re-agree, no supersession needed.
- Post-merge recommendation: ADR for "Rust `style_class_for`/`edge_style_class_for` output set ⊆ TS Zod enum" as a cross-layer invariant.

## Applied Lenses
| Lens | Delegation | Status | Why Applied | Design Impact |
|------|------------|--------|-------------|---------------|
| base-discipline | kernel | applied | always | verified Rust source; flagged handlers.ts gap + functional bug in F |
| entropy-sdd | heuristic | applied | schema-sync + drift | OCP-compliant additive enum widen (H(Δ)<1 bit); fixes REDUCE existing Name connascence |

## Invariants And Constraints
| Invariant / Constraint | Enforcement Point | Verification |
|------------------------|-------------------|--------------|
| Rust style_class output ⊆ TS Zod enum | schemas.ts L962-995 | schemas.test.ts parses fixture with all 4 new values |
| `npm run build` exits 0 | CI gate | `tsc -b && vite build` |
| Graph renderer receives RuntimeContext | rendererRegistry.getOrJson().render | existing PaneInspector/GraphViewRenderer tests |

---

## 1. Architecture Overview

### Schema widening data flow (Rust → Zod → stylesheet → renderer)
```
Rust api.rs                        TS schemas.ts              stylesheet.ts            cytoscape renderer
style_class_for()  ──HTTP JSON──▶  graphNodeStyleClassSchema  ──ResolvedNodeStyleClass──▶  SvgGraph/Cytoscape
edge_style_class_for()             graphEdgeStyleClassSchema
  node-code      (api.rs:65)   ──▶  MISSING  ──────────────✗──▶  HAS (L403)
  edge-part-of   (api.rs:101)  ──▶  MISSING  ──────────────✗──▶  HAS (L443)
  edge-deployed-as(api.rs:102) ──▶  MISSING  ──────────────✗──▶  HAS (L444)
  edge-in-system (api.rs:103)  ──▶  MISSING  ──────────────✗──▶  HAS (L445)
                         ↑ Zod REJECTS valid C4 payloads today (runtime bug)
```

### Error-cluster dependency graph
```
 A (schema widen) ─────────┐  (unblocks 3 node-code errors in NeighborMinigraph/
 B (nav exports)    indep  │   GraphLanding/InteractiveGraph + 3 edge-part-of in fixtures)
 C (Set type)       indep  │
 D (null|undefined) indep  │
 E (import path)    indep  │
 F (PaneInspector)  MEDIUM ◀── depends on rendererRegistry signature (verified getOrJson path)
 G (ViewBlock)      indep  │
 H (GraphView)      indep  │
 I (test mocks)     indep  │
 J (CytoscapeOpts)  indep  │
 K (bench ~27)      indep  ◀── largest, mechanical
 L (unused vars)    indep  │
 M (handlers.ts)    indep  ◀── GAP: not in caller's list, added by verification
```
All clusters are independent for review; no cluster blocks another's compile
(the build fails as a whole until all land, but commits don't review-depend).

---

## 2. Module-level Types

`schemas.ts` L962-977 (add one node bucket, grouped with C4 block):
```ts
export const graphNodeStyleClassSchema = z.enum([
  "function", "module", "external",
  "node-decision", "node-doc", "node-issue", "node-evidence",
  "node-component", "node-container", "node-system",
  "node-code",                 // ◀── ADD (api.rs:65, ADR-039 C4 code)
  "entry-point", "hot", "god",
]);
```
`schemas.ts` L987-995 (add three C4 edge buckets):
```ts
export const graphEdgeStyleClassSchema = z.enum([
  "edge.calls", "edge.implements", "edge.uses",
  "edge-cites", "edge-justifies", "edge-resolves", "edge-corroborated",
  "edge-part-of",      // ◀── ADD (api.rs:101)
  "edge-deployed-as",  // ◀── ADD (api.rs:102)
  "edge-in-system",    // ◀── ADD (api.rs:103)
]);
```
`ViewDescriptorPlus.source` (Cluster D2): widen from `"runtime" | null` to
`string | null` — the Zod `viewDescriptorSchema.source` is `z.string().nullable()`
and is the source of truth.

`viewKind.ts` GRAPH_KINDS — see §5.

---

## 3. Schema Widening Detail

| Value | Rust emits | TS schema | TS stylesheet | Consumed by |
|-------|-----------|-----------|---------------|-------------|
| `node-code` | api.rs:65 | ADD | L403 ✓ | NeighborMinigraph, GraphLanding, InteractiveGraph |
| `edge-part-of` | api.rs:101 | ADD | L443 ✓ | architectureFixtures (PartOf edges) |
| `edge-deployed-as` | api.rs:102 | ADD | L444 ✓ | C4 subgraph responses (facades/graph.rs) |
| `edge-in-system` | api.rs:103 | ADD | L445 ✓ | C4 subgraph responses |

**Test pattern** — `schemas.test.ts`:
```ts
it("accepts all backend-emitted style_class values", () => {
  const node = graphNodeStyleClassSchema.parse("node-code");
  for (const e of ["edge-part-of","edge-deployed-as","edge-in-system"]) {
    expect(graphEdgeStyleClassSchema.parse(e)).toBe(e);
  }
});
```

---

## 4. PaneInspector Render Strategy (Cluster F — most careful)

**Current** PaneInspector.tsx L247-251:
```tsx
return rendererRegistry.render(
  strategy.rendererKind,
  display.body ?? display,   // ❌ TS2339: ContextualView has no `body`
  runtimeContext,            // ❌ TS2554: render(id, body) is 2-arg; ctx is DROPPED
);
```
**Two bugs, one functional:** `rendererRegistry.render(id, body)` (L140) is a
convenience wrapper — it calls `getOrJson(id).render(body)` with **no `extra`**.
So the graph renderer (L155) reads `extra?.objectId ?? ""` → always `""`, and
never gets `onClose`. GraphView is silently broken on the registry path today.

**Correct:**
```tsx
return rendererRegistry
  .getOrJson(strategy.rendererKind)   // entry with json fallback
  .render(display, runtimeContext);   // (body, extra) — body IS the full view
```
- `display` (full `ContextualView`) is `body`; graph renderer casts it at L156.
- `runtimeContext` flows as `extra` → GraphView gets objectId/paneId/onClose.
- Drop the unused `useApp` import (F3, L9).

---

## 5. viewKind.ts Set Fix (Cluster C)

`Set<T>` has no numeric index signature, so `(typeof GRAPH_KINDS)[number]` is
TS2537. Derive the union from the array *before* wrapping in Set:
```ts
const GRAPH_KIND_ARRAY = [
  "call_graph", "dependency_graph", "data_flow", "impact_radius", "seam_map",
] as const;
export type GraphViewKind = typeof GRAPH_KIND_ARRAY[number];
export const GRAPH_KINDS = new Set<GraphViewKind>(GRAPH_KIND_ARRAY);
```
`isGraphViewKind` and `resolveRenderStrategy` unchanged.

---

## 6. Migration Sequence (atomic commits)

| # | Commit | Cluster | Risk |
|---|--------|---------|------|
| 1 | `fix(schema): sync Zod with Rust (node-code + 3 C4 edges)` | A | HIGH value, LOW risk |
| 2 | `fix(schema): exclude landing kinds from MULTIMODAL_KIND_INFO` | A3 | LOW |
| 3 | `fix(navigation): re-export ViewportState, drop unused imports` | B | LOW |
| 4 | `fix(viewKind): derive GraphViewKind from array not Set` | C | LOW |
| 5 | `fix(useViews): align null/undefined + source type` | D | LOW-MED |
| 6 | `fix(ViewBlocks): correct import path, drop unused type` | E | LOW |
| 7 | `fix(PaneInspector): use getOrJson().render(view, ctx)` | F | **MED** (functional) |
| 8 | `fix(ViewBlock): narrow block union at component props` | G | MED |
| 9 | `fix(GraphView): align Dispatch<Action> param type` | H | LOW |
| 10 | `fix(InteractiveGraph): extend CytoscapeOptions for renderer` | J | LOW |
| 11 | `fix(bench): resolve ~27 strict-mode violations` | K | LOW-MED (largest) |
| 12 | `fix(tests): GraphViewRenderer mocks + unused vars` | I + L | LOW |
| 13 | `fix(mocks): add last_scan_at to workspace fixture type` | **M (NEW)** | LOW |

**Order rationale:** A first (unblocks 6 downstream errors, fixes runtime bug).
F before G/H only because F is the careful seam — review it early. K is largest
but fully mechanical; safe to land late. **M is required** — without it the
"all 59 resolved" gate fails.

---

## 7. Test Strategy
| Layer | What | Approach |
|-------|------|----------|
| schemas.test.ts | 4 new enum values parse | fixture with all values |
| PaneInspector | registry path returns GraphView w/ ctx | existing dispatch tests (may need ctx assertion) |
| GraphViewRenderer.test.tsx | layout fn mock (I) | `vi.mock` instead of `mockReturnValue` on real fn |
| bench | strict-mode compile | no behavior change; tests stay green |
| Gate | `npm run build` exits 0, `npm run lint` no new errors, `npm run test` no new failures | CI |

## 8. Risks and Mitigations
| Risk | Lk | Mitigation |
|------|----|-----------|
| Schema widen misses a value | Low | divergence table verified vs api.rs:42-106 |
| F changes dispatch surface (E1.5) | Med | getOrJson path is the *intended* API; verify GraphView gets ctx in test |
| K brittle to upstream | Low | mechanical null-checks/`!`/explicit fields only |
| handlers.ts (M) scope creep | Low | one-line fixture type widen |

## 9. Open Architectural Questions
- None blocking. Post-merge: ADR for style_class enum invariant (recommended by proposal #2676).
- Minor: proposal said bench=28, fresh `tsc -b` shows ~27 — confirm exact at apply (non-blocking).

## File Changes
| File | Action |
|------|--------|
| api/schemas.ts | widen 2 enums (+4 values) |
| ObjectInspector/viewKind.ts | array-derive Set type |
| ObjectInspector/PaneInspector.tsx | getOrJson().render(view,ctx); drop useApp |
| hooks/useViews.ts | null/undefined + source widen |
| ObjectInspector/ViewBlocks/types.ts | fix import path, drop unused |
| ObjectInspector/ViewBlock.tsx | narrow block union |
| ObjectInspector/multimodal.ts | exclude landing kinds |
| GraphView/GraphView.tsx | Dispatch<Action> type |
| InteractiveGraph/InteractiveGraph.tsx | CytoscapeOptions |
| bench/renderers/types.ts | erasableSyntaxOnly (explicit fields) |
| bench/runner.ts, *.test.ts | ~27 strict fixes |
| mocks/architectureFixtures.ts | (auto-fixed by A) |
| mocks/handlers.ts | **add last_scan_at type (M)** |
| state/navigation/index.ts, slices/index.ts, slices/navigation.ts, context.ts | B cleanup |
| components/ScanBar.tsx, GraphLanding.test.tsx, viewKind.test.ts | L unused/required |

## Entropy Constraints
| Interface | Risk | Constraint |
|-----------|------|-----------|
| Rust style_class_for ↔ TS schema | divergence recurrence | invariant + schemas.test.ts gate |
| rendererRegistry.render (2-arg) vs getOrJson().render (body,extra) | callers misuse 2-arg wrapper | F fix; consider deprecating/renaming the 2-arg `render` |
