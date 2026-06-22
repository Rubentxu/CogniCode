# ADR-040: GraphViewRenderer para ViewKinds estructurales

## Estado

**Implementado** — 2026-06-20

## Contexto

La view `call_graph` en el Explorer UI mostraba el SVG en blanco, a pesar de que los metadatos (fan in: 3, fan out: 4, signature) sí se renderizaban correctamente. La investigación reveló que:

1. `call_graph` es un **ViewKind** (intención semántica), NO un block id
2. `ViewBlock.tsx` tiene un switch que renderiza blocks individuales: `identity`, `call_metrics`, `callers`, `callees`, etc.
3. `ViewBlock.tsx` NO tiene un caso `case "call_graph":` → cae en `default` → renderiza `<UnknownBlockView>` → SVG nunca se instancia

Además, queremos llevar el CogniCode Explorer a una experiencia de exploración estilo **Moldable Development** (filosofía de GToolkit/GtPager), donde:

- Las vistas no son páginas estáticas sino artefactos navegables
- Click en un objeto relacionado abre un nuevo pane (no reemplaza el actual)
- El pane stack preserva la exploración del usuario
- Cada pane guarda su estado visual (scroll, zoom, pan)

## Decisión

### 1. Routing

Crear un componente genérico **`GraphViewRenderer`** que consume el `ContextualView` completo y renderiza un `SvgGraph` interactivo. El routing ocurre en `PaneInspector` via `resolveRenderStrategy`:

```typescript
const display = view ?? _activeView;

// All rendering now routes through the registry
const strategy = resolveRenderStrategy(display);
return <strategy.renderer view={display} objectId={objectId} onClose={onClose} />;
```

`resolveRenderStrategy` dispatchs a `RendererEntry` for: `call_graph`, `dependency_graph`, `data_flow`, `impact_radius`, `seam_map`, and all other ViewKinds.

### 2. Layout

Usar `useMemo` con dependencia `[view.object_id, view.blocks]` para evitar recálculo en re-renders por `isValidating`:

```typescript
const layout = useMemo(() => {
  if (view.kind === "dependency_graph") {
    return layoutFromDependencyGraph(view); // futuro
  }
  return layoutFromContextualView(view);
}, [view.object_id, view.blocks]);
```

### 3. Empty State

Cuando `layout.nodes.length <= 1` (objeto sin callers ni callees), mostrar un empty state específico para graphs:

```typescript
if (layout.nodes.length <= 1) {
  return <GraphEmptyState />;
}
```

### 4. Navegación Moldable

Click en un nodo del graph despacha `SELECT_OBJECT` con el `nodeId`, reusando el `viewId` actual:

```typescript
<SvgGraph
  onSelectObject={(nodeId) => {
    dispatch({
      type: "SELECT_OBJECT",
      payload: { objectId: nodeId, viewId: view.id },
    });
  }}
/>
```

Esto abre un **nuevo pane** en el stack (Pane Stack navigation), preservando la exploración del usuario. La deduplicación (seleccionar mismo `objectId` reusa pane existente) ya está implementada en `pane-stack.spec.ts` P3.2 variant.

### 5. Edge Labels

Los labels de edges (`"calls"`, `"called by"`) se muestran **solo cuando el edge está highlighted** (hovered o selected). El graph se mantiene limpio por defecto.

### 6. Persistencia: Exploration Snapshot

La feature se llama **"Exploration Snapshot"**. Persiste el estado visual de cada pane:

```typescript
export interface PaneSnapshot {
  pane_id: string;
  object_id: string;
  view_id: string;
  scroll_y: number;
  viewport?: ViewportState;  // { x, y, scale } para graph views
}

export interface ExplorationSession {
  // ... campos existentes
  panes: Vec<PaneSnapshot>;  // REQUERIDO, sin default
}
```

### 7. Trigger Híbrido

Persistencia en dos niveles:

- **localStorage**: cache inmediato de cada cambio (instantáneo, sin red)
- **Servidor**: save manual via botón "Save exploration snapshot" (durable)

LocalStorage key: `cognicode.exploration.snapshot.${workspaceId}.${sessionId}`

### 8. NO Backward Compatibility

Las sesiones `ExplorationSession` antiguas (sin campo `panes`) son **inválidas** después del deploy. La deserialización falla con error 422. El usuario debe re-guardar explorations con la nueva arquitectura.

**NO usar `#[serde(default)]`** en el campo `panes`. Si falta, error explícito.

## Consecuencias

### Positivas

- ✅ **Fix bug crítico**: Call Graph renderiza SVG correctamente
- ✅ **Componente genérico**: reusado para 5 ViewKinds estructurales
- ✅ **Moldable Dev**: navegación con pane stack preserva exploración
- ✅ **Persistencia duradera**: usuario puede restaurar explorations completas
- ✅ **Performance**: `useMemo` evita recálculos innecesarios
- ✅ **Zero deuda técnica**: sin backward compat ni fallbacks

### Negativas

- ❌ **Breaking change**: sesiones antiguas inválidas
- ❌ **Bundle size**: nuevo componente (mitigado con React.lazy)
- ❌ **Complejidad**: routing distribuida entre `PaneInspector` y `ViewBlock`

### Neutras

- 🔄 LocalStorage vs servidor: ambos coexisten, cada uno con rol distinto

## Alternativas Consideradas

### A) Añadir caso `case "call_graph":` en `ViewBlock.tsx`

**Rechazada** porque:
- `ViewBlock` renderiza blocks individuales, no views completas
- El graph necesita acceso al `ContextualView` completo, no solo a un block
- Rompe el contrato `ViewBlock = renderer de un block`

### B) Conditional rendering inline en `PaneInspector`

**Rechazada** porque:
- Componente dedicado es más testeable
- Permite lazy loading del bundle del graph
- Separa concerns: `PaneInspector` orquesta, `GraphViewRenderer` renderiza

### C) Componente específico `CallGraphView` (no genérico)

**Rechazada** porque:
- `dependency_graph`, `data_flow`, `impact_radius`, `seam_map` también son graphs
- `SvgGraph` ya es genérico, solo cambia el layout
- `GraphViewRenderer` con `useMemo` puede despachar a diferentes layouts

### D) Trigger automático cada N segundos

**Rechazada** porque:
- Ruido en backend
- El usuario debe tener control explícito sobre qué se persiste

### E) Backward compatibility con `#[serde(default)]`

**Rechazada por el usuario:**
> "forzamos todo a las nuevas arquitecturas, no mantenemos nada ni soportamos funcionalidades antiguas"

## Referencias

- Grill session: 2026-06-20 (13 preguntas resueltas)
- Roadmap: `docs/roadmap/MOLDABLE-VIEW-PANE-STATE-2026.md`
- Bug original: SVG en blanco en view `call_graph`
- Concepto Moldable Dev: `CONTEXT.md` §Navigation §Inspector Pane Stack
- Concepto existente: `CONTEXT.md` §ExplorationSession, §Shareable Exploration

## Términos del Glossary (CONTEXT.md)

Esta ADR formaliza los siguientes términos añadidos a `CONTEXT.md`:

- **GraphViewRenderer**: Componente genérico que renderiza `ContextualView` con ViewKind estructural como `SvgGraph` interactivo.
- **Moldable Navigation**: Patrón donde click en objeto relacionado abre nuevo pane (Pane Stack), preservando exploración.
- **Exploration Snapshot**: Feature que persiste el estado visual completo de cada pane (scroll, zoom, pan) en `ExplorationSession.panes`.