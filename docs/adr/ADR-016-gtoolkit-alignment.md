# ADR-016: Alineación con gtoolkit — Brechas y Plan

**Fecha:** 2026-06-15
**Estado:** PROPOSED
**Decisión:** Cerrar las brechas de navegación (pane-stack) y persistencia semántica (ExplorationEvent) para alinear CogniCode Explorer con el modelo de gtoolkit.
**Fuente:** Revisión arquitectónica jun-15 contra gtoolkit; análisis del estado real del código.

---

## Context

CogniCode Explorer está inspirado en gtoolkit — el vocabulario (`ViewSpec`, `ViewKind`, `RendererKind`, `Inspector`, `Lens`), la arquitectura de vistas (ISP-segregated `ViewDescriptor`/`ViewExecutor`), la separación de responsabilidades (backend resuelve, frontend renderiza). Sin embargo, la auditoría de jun-15 reveló divergencias concretas que limitan la alineación declarada en `CONTEXT.md`.

### Brechas con gtoolkit (estado real a jun-15)

| Aspecto | gtoolkit | CogniCode Explorer | Estado |
|---------|----------|-------------------|--------|
| Navegación entre objetos | Pane stack (GtPager) | Column-based (Miller) | Brecha activa |
| Historial de exploración | GtPager navigation history | `HistoryEntry` de Ask (preguntas) | Brecha activa |
| Persistencia semántica | Sí, primera clase | Solo sesiones efímeras con TTL | Brecha activa |
| Moldable views runtime | Sí, maduro | Sí, v1 (sin remote renderers) | ✅ Parcialmente alineado |
| Mocks | Hand-written | Hand-written | ✅ Alineado |
| Spotter | Sí | Sí | ✅ Alineado |
| Lepiter (notebook) | Sí | Diseñado, no v1 | Diferido |

### Detalle de las brechas activas

**1. Navegación column-based vs pane-stack**

`CONTEXT.md` (jun-12) describe la navegación pane-stack como visión:

> "Clicking a related object opens a new pane to the right instead of replacing the current object. This preserves the exploration narrative:
> `[Entry Point] → [Symbol] → [Repository] → [Decision] → [Test]`"

**Realidad del código (jun-15):**
- `apps/explorer-ui/src/state/context.ts:23-44` — `AppState` tiene `columns: ExplorationColumn[]` (lineal) y `activeObjectId: string | null` (un solo foco).
- `context.ts:109-136` — `SELECT_OBJECT` "collapses trailing columns and pushes a new one" → comportamiento de **reemplazo**, no de **apilamiento lateral**.
- `context.ts:138-149` — `SET_ACTIVE_VIEW` solo actualiza el leaf (`columns[columns.length-1]`).
- No existe tipo `InspectorPane`, `panes: InspectorPane[]`, ni acción `PUSH_PANE`/`CLOSE_PANE`/`ACTIVATE_PANE`.

**Impacto:** para drill-down vertical (file → symbol → callee) funciona bien. Para exploración amplia (comparar 3 implementaciones de un trait) hay que navegar来回 entre columnas Miller. La metáfora Miller Columns + 1 inspector central es **diferente** del pane-stack de gtoolkit.

**2. Historial de exploración**

`CONTEXT.md` describe `ExplorationSession`:

> "Backend-owned semantic state — ordered navigation events `{ object_id, view_id, query, timestamp }` for restore, sharing, and MCP"

**Realidad del código:**
- `crates/cognicode-explorer/src/session/state.rs:28-36` — `HistoryEntry` es `{ question, answer_summary, pattern_id, ts }`. Es historial de **preguntas del Ask**, no de navegación entre objetos.
- `apps/explorer-ui/src/state/context.ts:62` — acción `ADD_EXPLORATION` definida pero **no usada** en el código (búsqueda confirma: solo aparece en tests y fixtures de mocks).
- No existe `ExplorationEvent` con `{ object_id, view_id, query, timestamp }`.

**Impacto:** el "shareable exploration" prometido no es posible. Un usuario no puede guardar y compartir el camino exacto que recorrió.

**3. Persistencia semántica**

`CONTEXT.md` declara persistencia primera clase con restore/sharing/MCP.

**Realidad del código:**
- `crates/cognicode-explorer/src/facades/persistence.rs:26` — `ExplorationPathStore` es un `Mutex<HashMap>` en memoria. No hay Postgres backing.
- `crates/cognicode-explorer/src/dto.rs:334-346` — `ExplorationPath { columns, objects, lens, created_at }` es UNA lista de columnas. No modela pane-stack.
- `save_exploration` valida que `columns` no esté vacío — funciona, pero es lineal.

**Impacto:** las exploraciones viven en RAM del proceso, se pierden al reiniciar. No hay URL compartible, no hay "resume from link".

### Por qué importa cerrar las brechas

- **`CONTEXT.md` miente softly.** El doc describe pane-stack, persistencia, sharing — pero el código implementa otra cosa. Un nuevo contributor que lea `CONTEXT.md` espera GtPager; encuentra Miller.
- **Costo de "falso gtoolkit".** Cuando un usuario familiarizado con gtoolkit intenta CogniCode, espera pane-stack y se encuentra con reemplazo. Fricción de onboarding.
- **Limitación real para el caso de uso "exploración amplia".** El vertical slice tracing (drill-down) está bien, pero el "compare implementations across crates" no lo soporta bien.

### Por qué NO se cerró antes

- **Complejidad.** Pane-stack no es solo un state distinto: cada pane necesita su propio `activeViewId`, `activeLensId`, scroll local, y posiblemente filtros locales. Multiplica el state.
- **Decisión consciente.** El código eligió column-based porque (a) cabe en pantallas pequeñas, (b) es más simple, (c) la metáfora Miller da "sensación de camino" sin apilamiento.
- **No había presión.** El proyecto es v1; el "vertical slice tracing" es el caso primario, y column-based lo cubre.

## Decision

Cerrar las dos brechas principales (navegación pane-stack + persistencia semántica) con un diseño **configurable y opt-in**, sin romper el flujo actual.

### Plan de implementación

**Fase 1 — Refactor del reducer con `NavigationAdapter` (3-4 días)**

Introducir una interfaz polimórfica para que `column-based` y `pane-stack` coexistan:

```ts
interface NavigationAdapter {
  onSelectObject(objectId: string, viewId?: string, kind?: string): void;
  onActivateView(viewId: string): void;
  onPop(): void;
  getActiveFocus(): { objectId: string | null; viewId: string | null; lensId: string | null };
  renderPanels(): ReactNode;  // el Shell llama esto
}
```

- `ColumnNavigation` (refactor mecánico del reducer actual) — `default`, sin cambios visibles.
- `PaneStackNavigation` (implementación nueva) — opt-in vía settings.
- `AppState` guarda `navigationMode: "column" | "pane-stack"` + el adapter correspondiente.
- Persistir la elección en `localStorage`.

**Fase 2 — Pane-stack end-to-end (1-2 semanas)**

- `InspectorPane { id, objectId, activeViewId, activeLensId, scroll?, localFilters? }` con state per-pane.
- Acciones: `PUSH_PANE`, `POP_PANE`, `CLOSE_PANE`, `ACTIVATE_PANE`, `REORDER_PANE` (drag).
- `Shell.tsx`: cuando `navigationMode === "pane-stack"`, ObjectInspector se vuelve un carrusel horizontal con tabs (estilo GtPager), con botones de cierre por pane.
- Viewport handling: pane-stack se degrada a column-based automáticamente en `small` viewport.
- Tests: viewport edge cases, navigation invariant, close+reopen.

**Fase 3 — `ExplorationEvent` + persistencia semántica (1 semana)**

- Nuevo tipo backend `ExplorationEvent { id, object_id, view_id, query?, ts }` con append-only log.
- `ExplorationSession { events: Vec<ExplorationEvent>, mode: "column" | "pane-stack" }`.
- `save_exploration` extendido: acepta mode, serializa pane-stack como `panes: Vec<Vec<ExplorationColumn>>`.
- Backward compat: `serde(default)` para que paths viejos deserialicen.
- URL sharing: encoding compacto de `ExplorationSession` en querystring.

**Fase 4 — Sharing y restore (3-5 días)**

- Endpoint `GET /api/explorations/{id}` que retorna `ExplorationSession` serializado.
- Botón "Share" en UI → copia URL con querystring.
- En `apps/explorer-ui/src/App.tsx`: al montar, parsear querystring, dispatchar `RESTORE_EXPLORATION`.

### Estimación total

| Fase | LOC | Archivos | Tests | Tiempo |
|------|-----|----------|-------|--------|
| 1. NavigationAdapter | 200-300 | 1-2 | 8-12 | 3-4 días |
| 2. Pane-stack | 600-800 | 2-3 | 15-20 | 1-2 semanas |
| 3. ExplorationEvent | 150-250 | 3-4 | 5-8 | 1 semana |
| 4. Sharing | 100-150 | 2-3 | 3-5 | 3-5 días |
| **Total** | **~1050-1500** | **~10** | **~30-45** | **3-4 semanas** |

### Riesgos

| Riesgo | Severidad | Mitigación |
|--------|-----------|------------|
| Pane-stack rompe viewport `small` | Alta | Degradar automáticamente a column-based en < 768px |
| Backward compat de `ExplorationPath` | Media | `#[serde(default)]` en nuevos campos; test golden de paths viejos |
| Tests existentes asumen modelo viejo | Media | Refactor de `MillerColumns.test.tsx`, `ObjectInspector` tests con shim |
| State por pane (scroll, lens) infla memoria | Baja | Cap en `panes.length` (8 panes máx), limpieza de panes cerrados |
| Sincronización frontend↔backend pane-stack | Media | Empezar con `panes` solo en frontend; backend recibe `panes: Vec<Vec<...>>` serializado |

## Rationale

- **Por qué pane-stack y no quedarse con column:** el `CONTEXT.md` lo declara, y la metáfora Miller + 1 inspector es confusa para usuarios de gtoolkit. No cerrar la brecha es admitir que `CONTEXT.md` miente.
- **Por qué opt-in y no default:** el flujo vertical slice tracing funciona bien con column; pane-stack es estrictamente más complejo. Forzar pane-stack a todos los usuarios es regresión de UX para el caso primario.
- **Por qué `NavigationAdapter` y no flag binario:** permite agregar un tercer modo (ej. "split-view" estilo VSCode) sin tocar el Shell. Cuesta ~30% más LOC que un flag, pero es la última vez que se diseña.
- **Por qué persistencia semántica con `ExplorationEvent`:** es el bloque mínimo para sharing/restore. Sin esto, pane-stack es local-only; con esto, es compartible (killer feature de gtoolkit).

## Alternatives Considered

- **No hacer nada (quedarse con column-based):** rechazado — admite que `CONTEXT.md` describe una愿景 que no vamos a construir. Peor que documentarlo explícitamente.
- **Hacer pane-stack el único modo, rompiendo column-based:** rechazado — regresión para el caso de uso primario (vertical slice). 80% de los flujos se resienten.
- **Pane-stack sin persistencia (solo state local):** rechazado — pane-stack sin sharing es pane-stack a medias. Sharing es lo que justifica la complejidad.
- **Flag binario en vez de `NavigationAdapter`:** rechazado — cuesta menos al inicio pero impide evolución (no hay tercer modo sin rewrite). El polimorfismo es el techo correcto.

## Consequences

### Positivas
- `CONTEXT.md` deja de mentir sobre navegación.
- Onboarding de usuarios de gtoolkit se vuelve directo.
- Sharing/restore de explorations (feature diferenciador vs Spotter de VSCode).
- Arquitectura abierta para agregar modos de navegación sin rewrite.

### Negativas
- +3-4 semanas de trabajo que podría ir a features más visibles (MCP tools nuevos, más views, calidad).
- Doble code path para mantener (`ColumnNavigation` y `PaneStackNavigation` comparten parte del state).
- Tests más complejos: invariantes de navegación, viewport handling, backward compat.

### Neutras
- Documentación del adapter en `state/navigation/README.md`.
- Settings UI tiene un nuevo toggle ("Use GtPager navigation").
- El backend `ExplorationPath` tiene un campo `mode` que envejece naturalmente con el tiempo.

## Validation

### Fase 1
- [ ] `NavigationAdapter` interface existe y está documentada
- [ ] `ColumnNavigation` es refactor del reducer actual con cero regresión de tests
- [ ] Tests cubren todos los actions (`PUSH_COLUMN`, `POP_COLUMN`, `SELECT_OBJECT`, etc.)

### Fase 2
- [ ] Pane-stack funcional: abrir 3 panes, cerrar el del medio, foco va al adyacente
- [ ] Viewport `small` degrada a column automáticamente
- [ ] Drag-to-reorder panes funciona
- [ ] Cap de 8 panes enforced

### Fase 3
- [ ] `ExplorationEvent` serializa correctamente
- [ ] `ExplorationPath` con `mode: "pane-stack"` round-trips por serde
- [ ] Paths viejos (`mode: "column"` implícito) deserializan sin error
- [ ] Golden test de paths pre-pane-stack

### Fase 4
- [ ] Botón "Share" copia URL con querystring
- [ ] Abrir URL restaura `ExplorationSession` con pane-stack correcto
- [ ] `GET /api/explorations/{id}` funciona para IDs válidos y retorna 404 para inválidos

## References

- `CONTEXT.md` líneas sobre "Pane stack", "Explorer Inspection", "ExplorationSession"
- `apps/explorer-ui/src/state/context.ts` — reducer actual
- `apps/explorer-ui/src/components/Shell.tsx` — viewport rendering
- `crates/cognicode-explorer/src/dto.rs:334-352` — `ExplorationPath`/`ExplorationColumn`
- `crates/cognicode-explorer/src/facades/persistence.rs:26-98` — `ExplorationPathStore` y `save_exploration`
- `crates/cognicode-explorer/src/session/state.rs:28-36` — `HistoryEntry` (Ask history, no navigation)
- ADR-009 — Hybrid Explorer Navigation (jun-12, base conceptual)
