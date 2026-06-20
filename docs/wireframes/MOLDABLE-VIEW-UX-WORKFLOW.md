# Moldable View UX Workflow — CogniCode Explorer

**Fecha:** 2026-06-20
**Estado:** Wireframes v1 — atado a implementación del roadmap
**Tecnología:** React + TypeScript + Playwright (e2e)
**Base arquitectónica:** ADR-040 + Roadmap MOLDABLE-VIEW-PANE-STATE-2026.md

---

## 1. Personas y Goals

### Persona 1: Developer Onboarding — "Lucía"

- **Rol:** Backend engineer, nueva en el proyecto CogniCode
- **Goal:** Entender la arquitectura en 30 minutos sin leer docs
- **Frustración:** "No sé por dónde empezar. Hay 50 crates."

### Persona 2: Senior Maintainer — "Carlos"

- **Rol:** Tech lead, 3 años en el proyecto
- **Goal:** Debugging rápido cuando un test falla en CI
- **Frustración:** "Necesito saltar del test al código que rompe en 3 clicks."

### Persona 3: Code Reviewer — "Marta"

- **Rol:** Reviewer de PRs, multi-proyecto
- **Goal:** Entender qué afecta un cambio antes de aprobar
- **Frustración:** "Cada PR es un viaje考古 arqueológico."

---

## 2. Core Workflows (User Journeys)

### Workflow A — "Entender una función desconocida"

**Actor:** Lucía (Persona 1)

```text
1. ⌘K abre Spotter
2. Escribe "build_overview" → ve 2 resultados
3. Click en primer resultado → abre pane "Overview"
4. Lee metadata (signature, file location)
5. Click en tab "Call graph" → abre GraphViewRenderer
6. Ve 4 nodos: build_overview (centro) + 3 callers/callees
7. Click en nodo "fan_in" → abre NUEVO pane (Pane Stack)
8. Pane stack: [build_overview | Call graph] → [fan_in | Call graph]
9. Lee metadata de fan_in
10. Click en tab "Source" → ve código de fan_in
11. Click en tab "Call graph" → ve callers de fan_in
12. Vuelve a tab "build_overview" → pane anterior intacto
```

**Tiempo objetivo:** < 2 minutos  
**Estados UI que atraviesa:** Spotter → Overview → Call Graph → Pane Stack → Source → Pane Stack navigation  

---

### Workflow B — "Debugging: test que rompe"

**Actor:** Carlos (Persona 2)

```text
1. CI reporta: test "test_ingest_pipeline" FAILED
2. ⌘K → "test_ingest_pipeline"
3. Ve el test en resultados → click
4. Pane: Overview → ve ubicación del test
5. Tab "Call graph" → ve qué llama al test setup
6. Click en nodo "MockBackend" → nuevo pane
7. Tab "Source" → lee implementación del mock
8. Identifica el bug: mock no maneja workspace vacío
9. Vuelve a tab "test_ingest_pipeline" (Pane Stack preservado)
10. Click "Save exploration" → guarda snapshot con 2 panes
11. Mañana abre "Saved Explorations" → restaura ambos panes
12. Ve el mock pane todavía en el scroll/zoom que dejó
```

**Tiempo objetivo:** < 5 minutos (incluyendo save/restore)  
**Estados UI que atraviesa:** Spotter → Inspector → Call Graph → Pane Stack → Save/Restore flow  

---

### Workflow C — "Code review: impacto de un cambio"

**Actor:** Marta (Persona 3)

```text
1. PR cambia función `validate_workspace`
2. ⌘K → "validate_workspace"
3. Tab "Call graph" → ve 12 callers
4. Click en cada caller → abre panes en stack
5. Pane stack: [validate_workspace | Call graph] → [ingest_pipeline | Call graph] → [cli_parser | Call graph] → [api_handler | Call graph]
6. Cada pane muestra callers específicos
7. Identifica que api_handler no tiene tests
8. Click "Save exploration" → nombre: "PR-1234 impact analysis"
9. Comarte URL del exploration con el PR author
10. Author restaura la exploration → ve el mismo pane stack
```

**Tiempo objetivo:** < 10 minutos  
**Estados UI:** Multi-pane navigation + Sharing flow

---

## 3. UI States — Por Componente

### 3.1 Shell (Top-level container)

```text
┌─────────────────────────────────────────────────────────┐
│ CogniCode Explorer  ● ws-prod-01  [Scan][Graph][C4]    │ ← Top bar
│                                          [Share] [🔍⌘K]│
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌─────────────────────┐  ┌─────────────────────────┐   │
│  │   InteractiveGraph  │  │  PaneStack              │   │
│  │   (left zone)       │  │  (right zone)           │   │
│  │                     │  │                         │   │
│  │                     │  │  [tab1] [tab2] [tab3]   │   │
│  │                     │  │  ─────────────────      │   │
│  │                     │  │                         │   │
│  │                     │  │  <pane content>         │   │
│  └─────────────────────┘  └─────────────────────────┘   │
│                                                          │
└─────────────────────────────────────────────────────────┘

States:
- Loading: Top bar shows "●" gray dot (HealthProbe)
- Error: Top bar shows "●" red dot + error message in main
- Empty (no workspace): GraphLanding component shows hero
- Small viewport: Bottom sheet replaces right zone
```

**testIds:** `shell`, `shell[data-viewport]`, `spotter-trigger`

---

### 3.2 Spotter (Global Search)

```text
States:
- HIDDEN: Default state (no overlay)
- LOADING: User types, results pending
- RESULTS: 0..N items visible
- EMPTY: Query has 0 results
- ERROR: MSW/network failure

Interaction:
- ⌘K (Mac) / Ctrl+K (Linux/Windows) → OPEN
- ESC → CLOSE
- ArrowUp/Down → navigate results
- Enter → SELECT first/highlighted result
- Click on result → SELECT

testIds: spotter, spotter-input, spotter-results, spotter-item-{n}
```

**Visual:**

```text
┌────────────────────────────────────┐
│  🔍  build_overview_        ESC    │  ← Input
├────────────────────────────────────┤
│ ▸ build_overview · function        │  ← Selected (default)
│   lib.rs:16                        │
│                                      │
│   build_callgraph · function        │
│   lib.rs:42                         │
└────────────────────────────────────┘
```

---

### 3.3 PaneStack (Right zone — desktop)

```text
States:
- EMPTY: No panes (initial state)
- SINGLE: 1 pane (most common)
- MULTI: 2..N panes (after drilling)
- DUPLICATE_PREVENTED: Selecting existing object activates tab (not new pane)

Interaction:
- Click on tab → activate pane
- Click ✕ on active tab → close pane (returns to previous)
- If last pane closed → EMPTY state
- Selecting existing objectId → DUPLICATE_PREVENTED

testIds: pane-stack, pane-tab-{n}, pane-close, pane-stack-empty
```

**Visual — Empty:**

```text
┌────────────────────────────────────┐
│                                     │
│         No panes open              │  ← Empty state
│                                      │
│         Press ⌘K to search         │
│                                     │
└────────────────────────────────────┘
```

**Visual — Single pane:**

```text
┌────────────────────────────────────┐
│ [build_overview ×]                 │  ← Tab strip
├────────────────────────────────────┤
│                                     │
│  Object Inspector                  │
│                                     │
│  function: build_overview          │
│  lib.rs:16                         │
│                                     │
│  ── View tabs ──                   │
│  [Overview] [Call graph] [Source]  │
│                                     │
│  <blocks rendering>                │
│                                     │
└────────────────────────────────────┘
```

**Visual — Multi-pane (Pane Stack):**

```text
┌────────────────────────────────────┐
│ [build_overview ×][fan_in ×][+]   │  ← Tabs
├────────────────────────────────────┤
│                                     │
│  Object Inspector (active: fan_in) │
│                                     │
│  function: fan_in                  │
│  lib.rs:42                         │
│                                     │
│  [Overview] [Call graph] [Source]  │
│                                     │
│  <fan_in content>                  │
│                                     │
└────────────────────────────────────┘
```

---

### 3.4 Object Inspector — Overview (default view)

```text
States:
- LOADING: First fetch
- READY: All blocks rendered
- ERROR: Object not found
- VALIDATING: Background refresh (silent)

Visual — Overview:
┌────────────────────────────────────┐
│ build_overview · function          │  ← Title
│ crates/.../lib.rs:16               │  ← Location
├────────────────────────────────────┤
│ Signature                          │
│ fn build_overview(...)             │  ← signature block
├────────────────────────────────────┤
│ Call metrics                       │
│ Fan in: 3  Fan out: 4              │  ← call_metrics block
├────────────────────────────────────┤
│ Callers (3)                        │
│ • explore                          │  ← callers block
│ • ingest_setup                     │
│ • cli_main                         │
├────────────────────────────────────┤
│ Callees (4)                        │
│ • fan_in                           │  ← callees block
│ • fan_out                          │
│ • …                                │
└────────────────────────────────────┘

testIds: object-inspector, object-inspector-body, view-tab-overview
```

---

### 3.5 Object Inspector — Call Graph (GraphViewRenderer)

**THIS IS THE FIX — the core of the new architecture.**

```text
States:
- LOADING: Layout calculating
- READY: SVG rendered with nodes
- EMPTY: No callers/callees (show empty state)
- ZOOMING: User wheel-zooming
- DRAGGING: User panning
- NAVIGATING: Click triggered new pane

Visual — Ready (the fix):
┌────────────────────────────────────┐
│ Call graph: build_overview         │  ← Title (from view.title)
│ 4 nodes · 3 edges                  │  ← Stats (auto-calculated)
├────────────────────────────────────┤
│                                     │
│         ┌──────┐                   │
│         │ build│ ← selected        │  ← Root node (center)
│         └──┬───┘                   │
│       ┌────┼────┐                  │
│       │    │    │                  │
│   ┌──▼─┐┌─▼──┐┌▼──┐               │
│   │exp ││fan ││ing│                │  ← Connected nodes
│   │lore││_in ││est│                │
│   └────┘└────┘└───┘                │
│                                     │
│                          ┌──┐      │
│                          │+ │      │  ← Zoom controls
│                          │- │      │     (top-right)
│                          │⟲ │      │
│                          └──┘      │
└────────────────────────────────────┘

testIds (NEW): graph-view-renderer, svg-graph-canvas, graph-node-{id}, graph-edge-{from-to}
```

**Visual — Empty (no callers/callees):**

```text
┌────────────────────────────────────┐
│ Call graph: isolated_function      │
├────────────────────────────────────┤
│                                     │
│         No call relationships      │  ← Empty state
│                                      │
│   This object has no incoming      │
│   or outgoing calls.                │
│                                      │
│   Try a different view like         │
│   "Overview" or "Source".          │
│                                     │
└────────────────────────────────────┘

testIds: graph-empty-state
```

**Visual — Loading:**

```text
┌────────────────────────────────────┐
│ Call graph: build_overview         │
├────────────────────────────────────┤
│                                     │
│           ⏳ Computing layout...   │  ← Skeleton
│                                     │
└────────────────────────────────────┘

testIds: graph-loading
```

---

### 3.6 InteractiveGraph (Left zone)

```text
States:
- LANDING: No root object (welcome screen)
- READY: Graph rendered with root + neighbors
- LOADING: Initial fetch
- ERROR: Workspace or graph failure

Visual — Landing:
┌────────────────────────────────────┐
│                                     │
│       👋 Welcome to CogniCode      │
│                                     │
│   Press ⌘K to search for a         │
│   symbol, file, or command.        │
│                                      │
│   Or browse recent explorations.   │
│                                     │
│   [List of recent explorations]    │
│                                     │
└────────────────────────────────────┘

testIds: interactive-graph-loading, graph-landing
```

---

## 4. Moldable Navigation Patterns

### 4.1 Pane Stack (Opción A — elegida)

```text
[Entry Point] → [Symbol A] → [Caller B] → [Callee C]
                  tab1          tab2         tab3
                  ◄── active tab is C, A and B preserved as tabs
```

**Reglas:**

1. Click en objeto relacionado → **abre nuevo pane** (no reemplaza)
2. Cada pane mantiene: `{objectId, viewId, scrollY, viewport}`
3. Deduplicación: si `objectId` ya existe → activa ese pane (no duplica)
4. Cerrar pane (✕) → vuelve al pane anterior
5. Cerrar último pane → empty state

---

### 4.2 Click-to-Explore (Graph View)

```text
┌─────────────────────────────────┐
│  Graph: build_overview          │
│                                  │
│      [build] ← root              │
│         │                        │
│      [fan_in] ← click target    │
│                                  │
└─────────────────────────────────┘

User clicks [fan_in] →
┌─────────────────────────────────┐
│  Graph: build_overview | fan_in │  ← Pane stack grows
│                                  │
│  Active tab: fan_in              │
│                                  │
└─────────────────────────────────┘

User clicks ✕ on fan_in →
┌─────────────────────────────────┐
│  Graph: build_overview          │  ← Returns to single pane
│                                  │
└─────────────────────────────────┘
```

---

## 5. Exploration Snapshot Flow

### 5.1 Save Flow

```text
1. User clicks "Share" button in top bar → testid="share-exploration"
2. Modal appears: "Save current exploration"
3. User enters name: "PR-1234 impact analysis"
4. User clicks "Save" → POST /api/explorations/session
5. Backend persists: { id, name, panes[], created_at }
6. localStorage cache: cognicode.exploration.snapshot.{workspace}.{session}
7. UI shows: "Saved! Share URL: /explore/{sessionId}"
8. User copies URL or shares with team
```

### 5.2 Load Flow

```text
1. User navigates to /explore/{sessionId}
2. Frontend: GET /api/explorations/session/{id}
3. Backend returns: { panes: [{paneId, objectId, viewId, scrollY, viewport}] }
4. Frontend: hydrate PaneStack with each pane
5. For each pane: fetch ContextualView for (objectId, viewId)
6. UI: render each pane in order, last one active
7. localStorage also updated for offline access
```

### 5.3 Visual — Save Modal

```text
┌────────────────────────────────────┐
│ Save exploration snapshot        ✕ │
├────────────────────────────────────┤
│                                     │
│ Name:                               │
│ ┌─────────────────────────────────┐│
│ │ PR-1234 impact analysis         ││
│ └─────────────────────────────────┘│
│                                     │
│ Will save:                          │
│ ✓ 3 panes                           │
│ ✓ Current viewport in Call graph    │
│                                     │
│              [Cancel] [Save]        │
└────────────────────────────────────┘

testIds: save-exploration-modal, save-exploration-name, save-exploration-cancel, save-exploration-save
```

---

## 6. Empty States — Catálogo Completo

| Componente | Empty State | Acción sugerida |
|------------|-------------|-----------------|
| Shell (no workspace) | "No workspace selected" | Open settings |
| Spotter (no results) | "No matches for 'xyz'" | Try broader query |
| PaneStack | "No panes open" | Press ⌘K to search |
| Call Graph (no callers/callees) | "No call relationships" | Try Overview or Source |
| InteractiveGraph (landing) | "Welcome — press ⌘K" | Open spotter |
| SavedExplorations (none) | "No saved explorations yet" | Browse workspace |
| Overview (no metadata) | "No metadata available" | Try Call graph |

---

## 7. Loading States

| Componente | Loading Visual | Duración objetivo |
|------------|----------------|-------------------|
| Initial app boot | Top bar health probe grays | < 500ms |
| Spotter search | Results fade-in skeleton | < 200ms (cached) |
| Object fetch | Inspector skeleton blocks | < 1s |
| Call Graph layout | "Computing layout..." skeleton | < 300ms |
| Save exploration | Modal spinner → success toast | < 2s |
| Load exploration | Full-screen skeleton | < 3s |

**Estrategia:** Skeleton blocks, nunca spinners genéricos. Cada skeleton refleja el layout del componente final.

---

## 8. Error States

| Error | Visual | Recuperación |
|-------|--------|--------------|
| MSW not loaded | "Mock mode unavailable" | Reload page |
| Workspace not found | "Workspace 'xyz' missing" | Choose another |
| Object not found | "Object 'abc' not in graph" | Back to spotter |
| Layout calculation failed | "Could not compute graph layout" | Try simpler view |
| Save failed (network) | Toast: "Save failed — retrying..." | Auto-retry 3x |
| Load failed (network) | "Could not load exploration" | Try cached version |
| localStorage quota exceeded | "Snapshot cache full" | Delete old snapshots |

---

## 9. Keyboard Shortcuts (Mac / Linux/Windows)

### Global

| Shortcut | Action | Context |
|----------|--------|---------|
| ⌘K / Ctrl+K | Open Spotter | Anywhere |
| ESC | Close modal/Spotter | When modal open |
| / | Focus search input | When not in input |

### Pane Stack

| Shortcut | Action | Context |
|----------|--------|---------|
| ⌘1..9 / Ctrl+1..9 | Switch to tab N | When pane stack active |
| ⌘W / Ctrl+W | Close active pane | When pane stack active |
| ⌘[ / ⌘] | Previous/next pane | When pane stack active |

### View Tabs

| Shortcut | Action | Context |
|----------|--------|---------|
| ← / → | Previous/next view | When view tabs focused |
| Enter / Space | Activate focused view | When view tab focused |

### Call Graph

| Shortcut | Action | Context |
|----------|--------|---------|
| Mouse wheel | Zoom in/out | When graph canvas focused |
| Drag | Pan | When graph canvas focused |
| Click on node | Navigate (open pane) | When graph canvas focused |
| Double-click on node | Navigate AND activate | When graph canvas focused |
| 0 | Reset zoom to 1:1 | When graph canvas focused |
| F | Fit graph to screen | When graph canvas focused |

---

## 10. Accessibility (a11y)

### Screen Reader Flows

**Workflow A — con screen reader:**

1. `aria-label="Spotter search"` → activate → Spotter opens
2. "Spotter, dialog, search input" → user types
3. "5 results found, listbox" → arrow keys to navigate
4. "build_overview, function, option 1 of 5, selected" → Enter
5. "Pane inspector: build_overview, region" → focus moves to inspector
6. "View tabs: Overview, Call graph, Source" → arrow keys
7. "Call graph, tab" → Enter activates
8. "Graph with 4 nodes, complementary" → graph content
9. Screen reader fallback table (sr-only) lists nodes:
   - "build_overview, symbol, position x=200, y=200"
   - "fan_in, symbol, position x=400, y=200"
10. Tab to node "fan_in" → "fan_in, button" → Enter → opens new pane
11. "Pane 2 of 2: fan_in, tab, selected" → confirms navigation

### Focus Management

- Spotter open → focus moves to input
- Spotter close → focus returns to trigger
- Pane switch → focus moves to active pane's body
- Modal close → focus returns to trigger button
- View tab switch → focus stays on tab strip

### Color Contrast

- All text: WCAG AA (4.5:1 minimum)
- Graph edges: distinguishable in grayscale (use shape/label, not just color)
- Selected node: bold border + different fill color
- Hover: lighter background, not only color change

---

## 11. Visual Regression Tests — Mapping

Each state above should have a golden image:

```typescript
// apps/explorer-ui/e2e/visual-regression.spec.ts

// 3.1 Shell states
test("shell-desktop", ...)
test("shell-tablet", ...)
test("shell-small", ...)

// 3.2 Spotter states
test("spotter-results", ...)
test("spotter-empty", ...)
test("spotter-loading", ...)

// 3.3 PaneStack states
test("pane-stack-empty", ...)
test("pane-stack-single", ...)
test("pane-stack-multi", ...)
test("pane-stack-deduplicated", ...)

// 3.4 Overview
test("overview-ready", ...)
test("overview-loading", ...)
test("overview-error", ...)

// 3.5 Call Graph (THE FIX)
test("call-graph-ready", ...)       // ← validates the fix
test("call-graph-empty", ...)       // ← new empty state
test("call-graph-loading", ...)     // ← new loading state
test("call-graph-zoomed", ...)      // ← viewport interaction
test("call-graph-node-clicked", ...) // ← pane navigation

// 5 Exploration Snapshot
test("save-modal", ...)
test("save-success", ...)
test("load-exploration-3panes", ...)
```

---

## 12. Component Test Map (Vitest)

```typescript
// apps/explorer-ui/src/components/ObjectInspector/PaneInspector.test.tsx

- renders shell when display is null
- renders Blocks for non-graph viewKind
- renders GraphViewRenderer for call_graph kind  ← NEW
- renders GraphViewRenderer for dependency_graph kind ← NEW
- early-return happens BEFORE LoadingTier
- passes objectId to GraphViewRenderer
- passes onClose when present

// apps/explorer-ui/src/components/GraphView/GraphViewRenderer.test.tsx ← NEW

- calls layoutFromContextualView with view
- uses useMemo with [object_id, blocks]
- shows empty state when nodes.length <= 1
- renders SvgGraph with layout
- dispatches SELECT_OBJECT on node click
- includes viewId in payload (Pane Stack)
```

---

## 13. Roadmap Cross-Reference

Cada elemento de UX mapeado a tarea del roadmap:

| UX Element | Tarea roadmap |
|------------|---------------|
| GraphViewRenderer empty state | 1.1 (Tarea 1.1) |
| PaneInspector early-return | 1.2 (Tarea 1.2) |
| Call graph golden image | 1.3 (Tarea 1.3) |
| Save modal | 2.4 (Tarea 2.4) |
| Load exploration flow | 2.4 + 2.5 (Tareas 2.4, 2.5) |
| Keyboard shortcuts | Spec implícito en cada componente |
| Empty state catalog | Implemented per-component |

---

**Owner:** Test-Pyramid-Builder Agent  
**Last Reviewer:** n/a  
**Next Review Date:** 2026-06-23