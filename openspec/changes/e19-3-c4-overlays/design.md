# Design: C4 Overlays — Drift Indicators & Hotspots

## Technical Approach

Frontend-only overlay layer for C4 perspective graphs. Two independent visual indicators applied as **additive Cytoscape classes** on top of base C4 node identity (`style_class` data attribute):

1. **Drift** — structural mismatch vs expected-architecture (`missing`/`extra`/`wrong-kind`), fetched `GET /api/workspaces/:id/drift`, matched to nodes by normalized container name.
2. **Hotspots** — risk scores from `GET /lens/hotspots`, rolled up to C4 nodes via `PartOf` traversal, thresholded to `hotspot-high`/`hotspot-med`.

Each indicator has an independent toolbar toggle. Toggling triggers lazy SWR fetch (session-cached) and re-computes overlay classes before graph mount.

## Architecture Decisions

### Decision: Overlay classes are additive Cytoscape classes, border-only

| | Detail |
|---|---|
| **Choice** | Extend `toCytoscapeElements` with optional `classesMap: Map<nodeId, string[]>`. Base C4 identity stays as `style_class` data attribute; overlays are Cytoscape classes setting `border-color` + `border-width` only. |
| **Rejected** | (a) Stuff overlays into `style_class` — breaks single-value contract + `resolveNodeStyleClass`. (b) Post-mount `cy.addClass()` — non-deterministic, re-render fragile. |
| **Rationale** | Separates identity (what kind of node) from state (what overlays apply). Border-only preserves C4 background color (blue/purple/gray = level); recoloring background erases that signal. Consistent with existing `.selected`/`.highlighted` class pattern. |

### Decision: Reducer owns toggle booleans; SWR owns fetched data

| | Detail |
|---|---|
| **Choice** | `c4OverlayState` slice holds `{ driftEnabled, hotspotsEnabled }` only. `useDrift`/`useC4Hotspots` hooks own SWR-cached data. Pure functions (`matchDriftFindingToNode`, `aggregateHotspotsForNode`, `computeOverlayClasses`) live as standalone utilities in `c4OverlayState.ts`. |
| **Rejected** | Proposal's slice shape `{ …, driftReport, hotspotMap }` in reducer. |
| **Rationale** | Mirrors existing pattern — `useLanding`/`useArchitecture` own data, reducer owns interaction state (`perspective`, `spotterOpen`). Duplicating SWR data into the reducer causes double-bookkeeping + stale-state bugs. `c4Levels.ts` already demonstrates pure-utilities-alongside-reducer. |

### Decision: Drift matching by normalized container name

| | Detail |
|---|---|
| **Choice** | Lowercase + trim + strip separators (`-_.`) from both `DriftFinding.container_name` and `GraphNode.label`, then equality-match. |
| **Rationale** | expected-architecture.yaml names and directory labels diverge in casing/separators. Exact match misses most findings. |

## Data Flow

```
Toolbar toggle ──→ dispatch TOGGLE_DRIFT/HOTSPOTS ──→ reducer
                                                          │
SWR hooks (useDrift, useC4Hotspots) ◀── lazy fetch ◀──────┤ (enabled gates SWR key)
        │                                                 │
        ▼                                                 ▼
DriftReport                              Map<nodeId, riskScore>
        └───────── computeOverlayClasses(nodeId, ...) ────┘
                                    │
                          classesMap ──→ toCytoscapeElements ──→ Cytoscape mount
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/hooks/useDrift.ts` | Create | SWR hook `GET /workspaces/:id/drift`; 404 → `driftReport: null` |
| `src/hooks/useC4Hotspots.ts` | Create | SWR hook `GET /lens/hotspots` + PartOf aggregation → `Map<nodeId, score>` |
| `src/state/c4OverlayState.ts` | Create | Reducer slice (toggles) + pure matching/overlay functions |
| `src/state/slices/index.ts` | Modify | Wire `c4OverlayReducer` into `rootReducer` |
| `src/state/context.ts` | Modify | Add `c4Overlays` to `AppState` + `Action` union; init in `initialState` |
| `src/api/types.ts` | Modify | Add `DriftReport`, `DriftFinding`, `HotspotRisk` + zod schemas |
| `src/components/InteractiveGraph/adapter.ts` | Modify | Accept optional `classesMap`; emit `classes` per node |
| `src/components/InteractiveGraph/stylesheet.ts` | Modify | Add 5 border-only overlay class selectors |
| `src/components/GraphLanding/GraphLanding.tsx` | Modify | Extend mount effect: C4 branch computes + passes `classesMap` |
| `src/components/PerspectiveToggle.tsx` | Modify | Add Drift/Hotspots toggle chips (C4 perspectives only) |
| `src/config/suggestedQuestions.ts` | Modify | Remove `compare` verb `disabledReason` guard (line 61) |

## Interfaces / Contracts

```typescript
// api/types.ts — new zod schemas + inferred types
interface DriftReport {
  findings: DriftFinding[];
  summary: { missing: number; extra: number; wrong_kind: number };
}
interface DriftFinding {
  kind: "missing" | "extra" | "wrong_sub_kind";
  container_name: string;   // matched to GraphNode.label (normalized)
  expected?: string; actual?: string;
}
interface HotspotRisk {
  symbol_id: string; file_id: string;
  risk_score: number; fan_in: number; fan_out: number;
}

// c4OverlayState.ts — pure overlay computation
function computeOverlayClasses(
  nodeId: string,
  state: { driftEnabled: boolean; hotspotsEnabled: boolean },
  driftReport: DriftReport | null,
  hotspotMap: Map<string, number>,
): string[]
// Priority on border conflict: hotspot-high > hotspot-med > drift-* (one winner)
```

Overlay CSS (stylesheet.ts additions):
```css
.drift-missing   { border-color: #ef4444; border-width: 3px }  /* red    */
.drift-extra     { border-color: #f59e0b; border-width: 3px }  /* amber  */
.drift-wrong-kind{ border-color: #eab308; border-width: 3px }  /* yellow */
.hotspot-high    { border-color: #dc2626; border-width: 4px }  /* dark red */
.hotspot-med     { border-color: #f97316; border-width: 2px }  /* orange */
```

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | `matchDriftFindingToNode` normalization | Table: `"Auth-Service"` ↔ `"auth_service"` |
| Unit | `aggregateHotspotsForNode` PartOf traversal | Mock graph, verify sum + threshold bucketing |
| Unit | `computeOverlayClasses` priority | Hotspot+drift → hotspot wins; disabled → `[]` |
| Unit | `useDrift` 404 handling | MSW 404 → `data: null`, no throw |
| Integration | GraphLanding C4 + overlays mounted | MSW fixtures → assert Cytoscape element classes |

## Migration / Rollout

No migration. Backend endpoints (`/drift`, `/lens/hotspots`) may not exist yet — hooks degrade gracefully (404 → null report, empty map → no overlays). Toggle chips render only when `perspective !== "graph"`.

## Open Questions

- [ ] Does `GET /lens/hotspots` return symbol-level `HotspotRisk[]` or reuse the existing scope-based `HotspotItem`? Wire shape unconfirmed — aggregation logic differs.
- [ ] `PartOf` edge direction in `ArchitecturePayload`: is `source` the parent or child? Aggregation traversal depends on it.
- [ ] Should `wrong_sub_kind` use `container_name` or a separate `node_id` for matching? (spec assumes name-based)

## ADR Candidates

- **Border-only additive Cytoscape overlay classes** — hard to reverse (visual contract baked into stylesheet + adapter), surprising (borders not fills, overlays don't recolor), real trade-off (preserves C4 level color vs visual prominence) → ADR-NNN
