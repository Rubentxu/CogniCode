# Roadmap: Moldable View Routing + Pane State Persistence

**Fecha:** 2026-06-20
**Estado:** Planeación post-grill session (7 preguntas resueltas)
**Prioridad:** ALTA — desbloquea bug crítico del Call Graph (SVG en blanco)

---

## Origen: Grill-with-Docs Session

Detectado bug crítico: la view `call_graph` muestra el SVG en blanco. Investigación reveló que `ViewBlock.tsx` no sabe cómo renderizar un graph — solo renderiza blocks individuales.

Resultado del grill: 10 decisiones arquitectónicas resueltas en sesión colaborativa con el usuario. Este roadmap traduce esas decisiones en tareas de implementación.

---

## Decisiones Resueltas (Grill Session 2026-06-20)

| # | Decisión | Justificación |
|---|----------|---------------|
| 1 | Routing en `PaneInspector` con early-return | Único punto que conoce `display = view ?? _activeView` |
| 2 | Ubicación: después de `display = view ?? _activeView` | Mantiene la lógica de routing visible y centralizada |
| 3 | `GraphViewRenderer` recibe `ContextualView` completo | Forward-compat, consistencia con `Blocks`, `layoutFromContextualView` ya lo espera |
| 4 | Componente genérico `GraphViewRenderer` (no específico) | Reusar para `call_graph`, `dependency_graph`, etc. |
| 5 | Navegación Moldable: **Opción A — Pane Stack** | Preserva exploración (GtPager), `pane-stack.spec.ts` ya implementa dedup |
| 6 | Edge labels: solo en highlight/hover | Mantiene graph limpio, alineado con `GraphEdge` actual |
| 7 | Empty state específico para graphs | Mensaje claro cuando no hay callers/callees |
| 8 | `useMemo` con `[view.object_id, view.blocks]` | No recalcula por `isValidating` |
| 9 | Pane State Snapshot: **Opción B** | Persistir scroll, zoom, pan por pane (no nodos hovered) |
| 10 | `PaneSnapshot` con `ViewportState {x, y, scale}` | Schema explícito, sin defaults |
| 11 | Trigger híbrido (manual + localStorage) | localStorage cache + manual save a servidor |
| 12 | Naming: "Exploration Snapshot" | Consistente con dominio `ExplorationSession` |
| 13 | **NO backward compatibility** — forzar arquitectura nueva | Sesiones antiguas inválidas; no se restauran |

---

## Épica 1: GraphViewRenderer (Fix Bug Call Graph)

**Objetivo:** Eliminar el bug crítico donde la view `call_graph` muestra SVG en blanco.

### Tarea 1.1 — Crear componente `GraphViewRenderer`

**Archivos:**
- `apps/explorer-ui/src/components/GraphView/GraphViewRenderer.tsx` (nuevo)
- `apps/explorer-ui/src/components/GraphView/index.ts` (nuevo)

**Implementación:**

```typescript
// apps/explorer-ui/src/components/GraphView/GraphViewRenderer.tsx
interface GraphViewRendererProps {
  view: ContextualView;
  objectId: string;
  onClose?: () => void;
}

export function GraphViewRenderer({ view, objectId, onClose }: GraphViewRendererProps) {
  const dispatch = useAppDispatch();
  
  // Memoización: solo recalcular cuando cambian los datos estructurales
  const layout = useMemo(() => {
    if (view.kind === "dependency_graph") {
      return layoutFromDependencyGraph(view); // futuro
    }
    return layoutFromContextualView(view);
  }, [view.object_id, view.blocks]);
  
  // Empty state: objeto sin callers ni callees
  if (layout.nodes.length <= 1) {
    return (
      <div data-testid="graph-empty-state" className="p-4">
        <h3>No call relationships</h3>
        <p>This object has no incoming or outgoing calls.</p>
        <p>Try a different view like "Overview" or "Source".</p>
      </div>
    );
  }
  
  return (
    <div className="flex h-full flex-col" data-testid="graph-view-renderer">
      <PaneHeader objectId={objectId} onClose={onClose} />
      <SvgGraph
        layout={layout}
        selectedId={objectId}
        onSelectObject={(nodeId) => {
          dispatch({
            type: "SELECT_OBJECT",
            payload: { objectId: nodeId, viewId: view.id },
          });
        }}
      />
    </div>
  );
}
```

**Criterios de aceptación:**
- [ ] `layout` solo recalcula cuando `view.object_id` o `view.blocks` cambian
- [ ] Empty state aparece cuando `nodes.length <= 1`
- [ ] Click en nodo despacha `SELECT_OBJECT` con el `nodeId`
- [ ] Componente testeable aisladamente con `@testing-library/react`

**Esfuerzo:** 2-3 horas

---

### Tarea 1.2 — Routing en PaneInspector

**Archivos:**
- `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx`

**Cambio:**

```typescript
// Después de calcular `display`
const display = view ?? _activeView;

// --- Moldable Dev: Graph views get dedicated renderer ---
if (display && isGraphViewKind(display.kind)) {
  return (
    <GraphViewRenderer
      view={display}
      objectId={objectId}
      onClose={onClose}
    />
  );
}
// ---------------------------------------------------------

function isGraphViewKind(kind: string | undefined): boolean {
  return kind === "call_graph" || 
         kind === "dependency_graph" || 
         kind === "data_flow" ||
         kind === "impact_radius" ||
         kind === "seam_map";
}
```

**Criterios de aceptación:**
- [ ] Early-return ocurre ANTES de `<LoadingTier>` para evitar loading flicker
- [ ] Funciona con MSW fixtures (mock mode)
- [ ] Funciona con backend real (no mock mode)
- [ ] El bug del SVG en blanco se resuelve

**Esfuerzo:** 30 min

---

### Tarea 1.3 — Visual Regression test para Call Graph

**Archivos:**
- `apps/explorer-ui/e2e/visual-regression.spec.ts` (extender)

**Test:**

```typescript
test("Call Graph view renders SVG with nodes (not blank)", async ({ page }) => {
  await page.goto("/");
  
  // Abrir el spotter y seleccionar objeto
  await page.keyboard.press("Meta+k");
  await page.getByTestId("spotter-input").fill("build");
  await page.getByTestId("spotter-results").getByTestId(/^spotter-item-/).first().click();
  
  // Click en tab "Call graph"
  const callGraphTab = page.getByTestId("view-tab-call-graph");
  await callGraphTab.click();
  
  // Verificar que el graph NO está en blanco
  const svgGraph = page.getByTestId("svg-graph-canvas");
  await expect(svgGraph).toBeVisible();
  
  // Verificar que tiene nodos renderizados
  const nodes = page.locator("[data-testid^='graph-node-']");
  await expect(nodes.first()).toBeVisible({ timeout: 3_000 });
  expect(await nodes.count()).toBeGreaterThan(1);
  
  // Captura visual
  await expect(page.getByTestId("graph-view-renderer")).toHaveScreenshot(
    "call-graph-rendered.png",
    { animations: "disabled", fullPage: true }
  );
});

test("Clicking a node opens a new pane (Pane Stack navigation)", async ({ page }) => {
  await page.goto("/");
  
  // Navegar al call graph
  await page.keyboard.press("Meta+k");
  await page.getByTestId("spotter-input").fill("build");
  await page.getByTestId("spotter-results").getByTestId(/^spotter-item-/).first().click();
  await page.getByTestId("view-tab-call-graph").click();
  
  // Estado inicial: 1 pane
  const tabs = page.locator("[data-testid^='pane-tab-']");
  await expect(tabs).toHaveCount(1);
  
  // Click en un nodo
  const firstNode = page.locator("[data-testid^='graph-node-']").first();
  await firstNode.click();
  
  // Debe abrir un nuevo pane (Pane Stack navigation)
  await expect(tabs).toHaveCount(2);
  
  // El nuevo pane debe estar activo
  await expect(tabs.last()).toHaveAttribute("aria-selected", "true");
});

test("Empty graph state shows helpful message", async ({ page }) => {
  // ... test para objeto sin callers ni callees
});
```

**Criterios de aceptación:**
- [ ] Golden image `call-graph-rendered.png` muestra el graph NO en blanco
- [ ] El test `Pane Stack navigation` valida que se abre un nuevo pane
- [ ] El test `Empty graph state` valida el empty state

**Esfuerzo:** 1-2 horas

---

### Tarea 1.4 — Documentación

**Archivos:**
- `docs/adr/ADR-040-graph-view-renderer.md` (nuevo ADR)

**Contenido del ADR:**

```markdown
# ADR-040: GraphViewRenderer para ViewKinds estructurales

## Contexto
La view `call_graph` mostraba SVG en blanco porque `ViewBlock.tsx` solo
renderiza blocks individuales, no un graph interactivo.

## Decisión
Crear `GraphViewRenderer` que:
- Recibe el ContextualView completo
- Calcula el layout con `layoutFromContextualView`
- Renderiza `SvgGraph` con el layout

El routing ocurre en `PaneInspector` con un early-return cuando
`view.kind` es un ViewKind estructural (`call_graph`, `dependency_graph`,
`data_flow`, `impact_radius`, `seam_map`).

## Consecuencias
Positivas:
- Fix del bug crítico del Call Graph
- Componente genérico reusado para 5 ViewKinds
- Navegación Moldable con Pane Stack (preserva exploración)

Negativas:
- Añade componente nuevo al bundle
- Routing logic distribuida entre `PaneInspector` y `ViewBlock`

## Alternativas consideradas
- Añadir caso `case "call_graph":` en `ViewBlock.tsx`: rechazado porque
  ViewBlock renderiza blocks, no views completas.
- Conditional rendering inline en `PaneInspector`: rechazado por legibilidad.
```

**Esfuerzo:** 30 min

---

## Épica 2: Pane State Persistence

**Objetivo:** Persistir el estado visual de cada pane (scroll, zoom, pan) para poder restaurar la exploración completa.

### Tarea 2.1 — Extender `Pane` con `ViewportState`

**Archivos:**
- `apps/explorer-ui/src/state/navigation/types.ts`

**Cambio:**

```typescript
// Añadir a la interface `Pane`
export interface Pane {
  id: string;
  objectId: string;
  viewId: string | null;
  // ... campos existentes
  viewport?: ViewportState;  // ← NUEVO (solo para graph views)
}

export interface ViewportState {
  x: number;
  y: number;
  scale: number;
}
```

**Esfuerzo:** 15 min

---

### Tarea 2.2 — Capturar viewport en `SvgGraph`

**Archivos:**
- `apps/explorer-ui/src/components/SvgGraph/SvgGraph.tsx`

**Cambio:**

```typescript
// Cuando el usuario interactúa (pan/zoom), despachar acción
const dispatchViewportChange = useCallback(
  (viewport: ViewportState) => {
    dispatch({
      type: "UPDATE_PANE_VIEWPORT",
      payload: { paneId, viewport },
    });
  },
  [dispatch, paneId]
);
```

**Esfuerzo:** 1 hora

---

### Tarea 2.3 — Backend: extender `ExplorationSession` con `panes`

**Archivos:**
- `crates/cognicode-explorer/src/facades/persistence.rs`
- `crates/cognicode-explorer/src/api/types.rs` (o similar)

**Cambio:**

```rust
#[derive(Serialize, Deserialize)]
pub struct PaneSnapshot {
    pub pane_id: String,
    pub object_id: String,
    pub view_id: String,
    pub scroll_y: f32,
    pub viewport: Option<ViewportState>,
}

#[derive(Serialize, Deserialize)]
pub struct ViewportState {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

pub struct ExplorationSession {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub events: Vec<NavigationEvent>,
    pub panes: Vec<PaneSnapshot>,  // ← NUEVO
    pub created_at: DateTime<Utc>,
}
```

**Esfuerzo:** 2-3 horas

---

### Tarea 2.4 — Frontend: capturar snapshot al guardar

**Archivos:**
- `apps/explorer-ui/src/hooks/useExplorations.ts`

**Cambio:**

```typescript
export async function saveExploration(
  request: unknown,
  currentPanes: Pane[],
): Promise<ExplorationSession> {
  const parsedRequest = saveExplorationSessionRequestSchema.parse(request);
  
  // Capturar snapshot de cada pane
  const panes: PaneSnapshot[] = currentPanes.map(pane => ({
    pane_id: pane.id,
    object_id: pane.objectId,
    view_id: pane.viewId ?? "overview",
    scroll_y: pane.scrollY ?? 0,
    viewport: pane.viewport,
  }));
  
  const session = await apiPost(
    "/explorations/session",
    { ...parsedRequest, panes },
    explorationSessionSchema,
  );
  return session;
}
```

**Esfuerzo:** 2 horas

---

### Tarea 2.5 — Tests E2E para persistencia

**Archivos:**
- `apps/explorer-ui/e2e/exploration-persistence.spec.ts` (nuevo)

**Tests:**

```typescript
test("saving exploration persists pane state", async ({ page }) => {
  // 1. Crear pane con call graph
  // 2. Hacer zoom/pan
  // 3. Guardar exploration
  // 4. Verificar que el backend recibió el viewport
});

test("loading exploration restores all panes with their viewports", async ({ page }) => {
  // 1. Cargar exploration guardada
  // 2. Verificar que cada pane tiene su viewport
});
```

**Esfuerzo:** 2 horas

---

## Estimación Total

| Épica | Tareas | Esfuerzo | Prioridad |
|-------|--------|----------|-----------|
| 1 — GraphViewRenderer | 4 | 4-6 horas | ALTA (fix bug) |
| 2 — Pane State Persistence | 5 | 7-8 horas | MEDIA |

**Total:** ~11-14 horas (~2 días)

---

## Orden de Implementación Recomendado

1. **Tarea 1.1** — Crear `GraphViewRenderer` (núcleo del fix)
2. **Tarea 1.2** — Routing en `PaneInspector` (desbloquea el fix)
3. **Tarea 1.3** — Visual regression test (valida el fix)
4. **Tarea 1.4** — ADR-040 (documenta la decisión)
5. **Tarea 2.1** — Extender `Pane` con `ViewportState` (núcleo de persistencia)
6. **Tarea 2.2** — Capturar viewport en `SvgGraph` (alimenta persistencia)
7. **Tarea 2.3** — Backend `ExplorationSession.panes` (schema)
8. **Tarea 2.4** — Frontend captura snapshot (al guardar)
9. **Tarea 2.5** — Tests E2E (validación)

---

## Riesgos y Mitigaciones

| Riesgo | Mitigación |
|--------|------------|
| Bundle size aumenta con `GraphViewRenderer` | Lazy load del componente (React.lazy) |
| Layout changes cause flicker al navegar | Mantener `useMemo` estricto |
| Viewport state no migra correctamente | Tests de round-trip save/load |
| Performance: snapshot grande con muchos panes | Límite de panes (MAX_PANES = 10) |

---

## Preguntas Resueltas (Grill Session — 13/13)

Todas las preguntas del grill session fueron resueltas con el usuario:

| # | Pregunta | Resolución |
|---|----------|------------|
| 1 | ¿Dónde decidir routing graph vs blocks? | `PaneInspector` con early-return |
| 2 | ¿Punto exacto del early-return? | Después de `display = view ?? _activeView` |
| 3 | ¿Qué recibe `GraphViewRenderer`? | `ContextualView` completo |
| 4 | ¿Genérico o específico? | Genérico (`GraphViewRenderer`) |
| 5 | ¿Cómo navega Moldable? | Opción A — Pane Stack |
| 6 | ¿Edge labels permanentes? | Solo en highlight/hover |
| 7 | ¿Empty state? | Específico para graphs |
| 8 | ¿Memoización del layout? | `useMemo([view.object_id, view.blocks])` |
| 9 | ¿Qué persistir por pane? | Opción B — scroll + zoom + pan |
| 10 | ¿Estructura del snapshot? | `PaneSnapshot { object_id, view_id, scroll_y, viewport }` |
| 11 | ¿Trigger de persistencia? | Opción C — Híbrido (manual + localStorage) |
| 12 | ¿Nombre de la feature? | "Exploration Snapshot" |
| 13 | ¿Backward compatibility? | **NO** — forzar arquitectura nueva, sesiones antiguas inválidas |

---

## Decisión de Migración (Resolución 13)

**Decisión:** NO mantener backward compatibility. Las sesiones `ExplorationSession` antiguas (sin campo `panes`) son **inválidas** después del deploy.

**Implementación:**

```rust
// NO usar #[serde(default)] — deserialización falla si falta `panes`
pub struct ExplorationSession {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub events: Vec<NavigationEvent>,
    pub panes: Vec<PaneSnapshot>,  // REQUERIDO — sin default
    pub created_at: DateTime<Utc>,
}
```

**Comportamiento:**
- ❌ Sesiones antiguas (sin `panes`) → error 422 al deserializar
- ❌ localStorage antiguo → se ignora silenciosamente
- ❌ "Load exploration" → solo permite cargar sesiones nuevas
- ✅ Usuario debe re-guardar explorations manualmente con la nueva arquitectura

**Justificación del usuario:**
> "forzamos todo a las nuevas arquitecturas, no mantenemos nada ni soportamos funcionalidades antiguas"

Alineado con el principio del proyecto: NO mantener código legacy, romper explícitamente para forzar la nueva arquitectura.

**Consecuencias:**
- Positivo: cero deuda técnica de migración
- Positivo: código limpio, sin fallbacks
- Negativo: usuarios con explorations guardadas pierden acceso
- Negativo: requiere comunicación explícita del breaking change

---

**Owner:** Test-Pyramid-Builder Agent  
**Fecha de inicio:** 2026-06-20 (post-grill)
**Fecha objetivo de cierre:** 2026-06-27 (1 sprint)
**Last Reviewer:** n/a  
**Next Review Date:** 2026-06-23