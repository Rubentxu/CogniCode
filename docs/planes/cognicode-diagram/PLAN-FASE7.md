# Plan Fase 7 — Dashboard Integration

## Objetivo

Integrar `cognicode-diagram` en `cognicode-dashboard`, añadiendo una nueva sección de visualización de diagramas que permite generar, visualizar, comparar y compartir diagramas C4, sequence, state machine, y activity directamente desde el dashboard web.

## Duracion Estimada: 3 semanas

## Pre-requisitos

- Fase 5 completada (cognicode-diagram production-ready)
- Fase 6 en progreso (T6.1-T6.7 avanzado)
- `cognicode-dashboard` funcional

## Arquitectura de Integracion

### 11.1 Stack Tecnico

| Capa | Tecnologia Actual | Cambio |
|------|------------------|--------|
| Frontend | Leptos 0.8 CSR | + DiagramViewer components |
| Server | Axum 0.7 | + Diagram API endpoints |
| Diagram Engine | cognicode-diagram | Exponer como library |
| Database | cognicode.db | + diagram_snapshots table |

### 11.2 Modelo de Integracion

```
┌─────────────────────────────────────────────────────────────────────┐
│                     cognicode-dashboard                               │
│                                                                      │
│  ┌──────────────────┐      ┌───────────────────────────────────┐   │
│  │  Frontend WASM    │      │  Server (Axum)                     │   │
│  │  (Leptos 0.8)    │◄────►│  Puerto 3000                       │   │
│  │                    │ HTTP │  API REST + Static Files           │   │
│  │  + DiagramViewer  │      │  + Diagram Server Functions        │   │
│  │  + DiagramSelector│      │  + reverse_engineer_c4 wrapper      │   │
│  │  + ThemeSelector  │      │  + sequence_diagram wrapper         │   │
│  │  + DiagramDiff    │      │  + diff_diagrams wrapper            │   │
│  └──────────────────┘      └───────────┬───────────────────────┘   │
│                                          │                           │
│                    ┌─────────────────────┼───────────────────────┐   │
│                    │  cognicode-diagram  │                         │   │
│                    │  (called in-process)│                         │   │
│                    └──────────┬──────────┴────────────────────────┘   │
│                               │                                       │
└───────────────────────────────│───────────────────────────────────────┘
                                │
┌───────────────────────────────│───────────────────────────────────────┐
│                    cognicode-diagram                                  │
│                                                                       │
│  ┌─────────────────┐    ┌──────────────┐    ┌────────────────┐       │
│  │   INFERENCE      │    │    LAYOUT     │    │    RENDER      │       │
│  │                  │    │              │    │                │       │
│  │ CallGraph ─────►│───►│ C4Workspace  │───►│ Mermaid code   │       │
│  │ Cargo.toml ─────►│    │ + positions  │    │ SVG            │       │
│  │ package.json ───►│    │              │    │ D2             │       │
│  │                  │    │              │    │                │       │
│  └─────────────────┘    └──────────────┘    └────────────────┘       │
└───────────────────────────────────────────────────────────────────────┘
```

---

## Tasks

### T7.1: Diagram API Endpoints (3 dias)

**Descripcion**: Crear endpoints REST en el dashboard server para invocar las funciones de cognicode-diagram.

**Pasos**:
- [ ] `crates/cognicode-dashboard/src/api/diagrams.rs` — Server functions
- [ ] Endpoints:
  - [ ] `POST /api/diagrams/c4` — Generar diagrama C4
    - Body: `{project_path, level, format, theme}`
    - Returns: `{mermaid, svg?, d2?, diagram_id}`
  - [ ] `GET /api/diagrams/{id}` — Obtener diagrama por ID
  - [ ] `GET /api/diagrams/{id}/rendered` — Obtener como SVG/Mermaid
  - [ ] `POST /api/diagrams/sequence` — Generar sequence diagram
    - Body: `{project_path, entry_symbol, format}`
  - [ ] `POST /api/diagrams/state-machine` — Generar state machine
    - Body: `{project_path, symbol_name, format}`
  - [ ] `POST /api/diagrams/activity` — Generar activity diagram
    - Body: `{project_path, symbol_name, format}`
  - [ ] `POST /api/diagrams/diff` — Comparar dos diagramas
    - Body: `{diagram_id_a, diagram_id_b, format}`
  - [ ] `GET /api/diagrams/history` — Historial de diagramas generados
- [ ] Integracion con cognicode-diagram:
  - [ ] Llamadas in-process (mismo proceso Rust)
  - [ ] Reutilizar CallGraph cacheado
  - [ ] Timeout configurable para diagramas complejos

**Criterio de aceptacion**: `POST /api/diagrams/c4` con `{project_path, level: "container"}` devuelve Mermaid valido en <2s.

---

### T7.2: DiagramViewer Component (4 dias)

**Descripcion**: Componente Leptos para visualizar diagramas en el dashboard.

**Pasos**:
- [ ] `crates/cognicode-dashboard/src/components/diagrams/mod.rs`
- [ ] `DiagramViewer` component props:
  - `diagram_type`: C4 | Sequence | StateMachine | Activity
  - `mermaid_code`: String
  - `theme`: light | dark
  - `zoomable`: bool
  - `panable`: bool
- [ ] Funcionalidades:
  - [ ] Render Mermaid via `mermaid.js` WASM o iframe embed
  - [ ] Zoom in/out con botones y scroll
  - [ ] Pan/drag para diagramas grandes
  - [ ] Descargar como SVG/PNG
  - [ ] Copiar codigo Mermaid al portapapeles
  - [ ] Fullscreen mode
- [ ] Theme support:
  - [ ] Light: fondo blanco, texto negro
  - [ ] Dark: fondo oscuro, colores invertidos
  - [ ] Toggle entre themes

**Criterio de aceptacion**: DiagramViewer renderiza un C4 container diagram de 10+ nodos con zoom/pan funcional.

---

### T7.3: Diagrams Page (`/diagrams`) (4 dias)

**Descripcion**: Nueva pagina en el dashboard para generar y explorar diagramas.

**Pasos**:
- [ ] `crates/cognicode-dashboard/src/pages/diagrams.rs` — Pagina completa
- [ ] Layout:
  - [ ] Header: titulo, boton "Generate New"
  - [ ] Sidebar: lista de diagramas guardados
  - [ ] Main: DiagramViewer + controls
  - [ ] Footer: metadata (generated at, project, level)
- [ ] Selector de tipo de diagrama:
  - [ ] C4 Level: Context | Container | Component | Code
  - [ ] UML Type: Sequence | State Machine | Activity
- [ ] Controls:
  - [ ] Format: Mermaid | SVG | PlantUML | D2
  - [ ] Theme: Light | Dark | Auto
  - [ ] Entry point (para sequence/activity)
- [ ] Boton Generate con loading state
- [ ] Historial de diagramas generados (guardados en localStorage)

**Criterio de aceptacion**: Pagina `/diagrams` permite seleccionar C4 Container, hacer click en Generate, y ver el diagrama renderizado en <3s.

---

### T7.4: Diagram Comparison View (3 dias)

**Descripcion**: Vista para comparar dos versiones de un diagrama.

**Pasos**:
- [ ] `crates/cognicode-dashboard/src/pages/diagram_diff.rs`
- [ ] Layout side-by-side:
  - [ ] Panel A: diagrama anterior
  - [ ] Panel B: diagrama nuevo
  - [ ] Panel diff: changeset visual
- [ ] Controles:
  - [ ] Selector de runs para comparar
  - [ ] Toggle sync zoom entre paneles
  - [ ] Highlight de cambios (verde=añadido, rojo=eliminado)
- [ ] Resumen de cambios:
  - [ ] "+N elementos nuevos"
  - [ ] "-N elementos eliminados"
  - [ ] "~N elementos modificados"
- [ ] Navegacion: click en elemento cambiado → scroll a su ubicacion en cada panel

**Criterio de aceptacion**: Comparar diagramas de dos analysis runs muestra diff visual con elementos resaltados en color.

---

### T7.5: Real-time Diagram Updates (3 dias)

**Descripcion**: Auto-regenerar diagramas cuando cambia el codigo.

**Pasos**:
- [ ] File watcher integration:
  - [ ] Usar `notify` crate para detectar cambios en archivos
  - [ ] Debounce: esperar 2s sin cambios antes de regenerar
  - [ ] Solo regenerar si el archivo afecta al diagrama actual
- [ ] WebSocket para updates:
  - [ ] `GET /ws/diagrams/live` — WebSocket connection
  - [ ] Server -> Client: `{type: "diagram_updated", diagram_id, changes}`
  - [ ] Client: actualiza DiagramViewer automaticamente
- [ ] Indicador UI:
  - [ ] Badge "Live" cuando watching está activo
  - [ ] Toast notification cuando diagrama se actualiza
  - [ ] Boton "Pause updates" / "Resume"
- [ ] Toggle en Settings para enable/disable auto-update

**Criterio de aceptacion**: Modificar un archivo .rs activa regeneration y el diagrama se actualiza en <5s sin recargar la pagina.

---

### T7.6: Diagram Sharing & Export (2 dias)

**Descripcion**: Exportar y compartir diagramas.

**Pasos**:
- [ ] Exportar como:
  - [ ] SVG (vector, editable)
  - [ ] PNG (raster, 2x resolution)
  - [ ] PDF (para documentacion)
  - [ ] Mermaid code (.mmd file)
- [ ] Share:
  - [ ] Generar URL con diagrama embebido (base64 encoded)
  - [ ] Copy link to clipboard
  - [ ] Share button para Twitter/LinkedIn (Open Graph tags)
- [ ] Embed:
  - [ ] Iframe code para incrustar en README.md
  - [ ] Markdown con el diagrama (para docs)
- [ ] Print:
  - [ ] CSS print styles para diagramas
  - [ ] A4/Letter size option

**Criterio de aceptacion**: Boton "Share" genera URL que al abrirla muestra el mismo diagrama en otro browser.

---

### T7.7: Dashboard Navigation Integration (2 dias)

**Descripcion**: Integrar diagramas en la navegacion existente del dashboard.

**Pasos**:
- [ ] Sidebar: anadir item "Diagrams"
  - [ ] Icono: diagram-icon (mermaid-like)
  - [ ] Badge con count de diagramas guardados
- [ ] Breadcrumb integration:
  - [ ] Dashboard > Diagrams > {diagram_name}
- [ ] Linking desde otras paginas:
  - [ ] Project card: anadir boton "View Diagram"
  - [ ] Issues: link "View in Diagram" lleva al diagrama filtrado
- [ ] Deep linking:
  - [ ] URL params para restaurar estado: `/diagrams?type=c4&level=container&project=/path/to/proj`

**Criterio de aceptacion**: Navegando a Dashboard > Diagrams abre la pagina de diagramas con el sidebar item activo.

---

## Dependencias

```
T7.1 (API) ← F5 (cognicode-diagram) + F6 (T6.1-T6.3)
T7.2 (Viewer) ← T7.1
T7.3 (Page) ← T7.1 + T7.2
T7.4 (Compare) ← T7.1 + T7.2 + T6.7 (Diff)
T7.5 (Live) ← T7.3
T7.6 (Export) ← T7.3
T7.7 (Nav) ← T7.3
```

## Milestones

| Milestone | Task | Criterio de aceptacion |
|---|---|---|
| M16: API | T7.1 | POST /api/diagrams/c4 devuelve Mermaid valido |
| M17: Viewer | T7.2 | DiagramViewer renderiza con zoom/pan |
| M18: Page | T7.3 | Pagina /diagrams funcional |
| M19: Compare | T7.4 | Side-by-side diff con highlighting |
| M20: Live | T7.5 | Auto-update en <5s al cambiar codigo |
| M21: Export | T7.6 | Export SVG/PNG funcional |
| M22: Nav | T7.7 | Sidebar integration completa |

## Base de Datos

### Nueva tabla: `diagram_snapshots`

```sql
CREATE TABLE diagram_snapshots (
    id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    diagram_type TEXT NOT NULL, -- 'c4', 'sequence', 'state_machine', 'activity'
    level TEXT, -- 'context', 'container', 'component', 'code'
    mermaid_code TEXT NOT NULL,
    svg_code TEXT,
    metadata TEXT, -- JSON: {entry_symbol, format, theme}
    created_at TEXT NOT NULL,
    run_id TEXT, -- FK a analysis_runs
    FOREIGN KEY (run_id) REFERENCES analysis_runs(id)
);

CREATE INDEX idx_diagram_project ON diagram_snapshots(project_path);
CREATE INDEX idx_diagram_run ON diagram_snapshots(run_id);
```

---

## Ejemplo de Uso

### Flujo 1: Generar diagrama C4

```
1. Usuario abre /diagrams
2. Selecciona "C4" > "Container"
3. Click "Generate"
4. Server: reverse_engineer_c4(project_path, level="container")
5. Server: render_mermaid(c4_workspace, format="mermaid")
6. Client: DiagramViewer renderiza el Mermaid
7. Usuario puede zoom/pan/exportar
```

### Flujo 2: Comparar versiones

```
1. Usuario genera diagrama C4 Container
2. Tiempo despues, modifica codigo (anade nuevo container)
3. Click "Compare" > selecciona "vs Previous Run"
4. Server: diff_diagrams(run_a, run_b)
5. Client: DiagramDiffView muestra side-by-side con highlights
6. Usuario ve "+1 container (auth-service)"
```

### Flujo 3: Live updates

```
1. Usuario abre diagrama C4 en modo "Live"
2. WebSocket conectado a /ws/diagrams/live
3. Usuario modifica src/main.rs en editor externo
4. File watcher detecta cambio
5. Server regenera diagrama
6. WebSocket envia {type: "diagram_updated"}
7. Client actualiza DiagramViewer automaticamente
8. Toast: "Diagram updated (1 change detected)"
```

---

## API Reference

### POST /api/diagrams/c4

```json
// Request
{
  "project_path": "/path/to/project",
  "level": "container",
  "format": "mermaid",
  "theme": "dark"
}

// Response 200
{
  "id": "diag_abc123",
  "mermaid_code": "graph TB\n  A[Auth Service] --> B[Database]",
  "svg_code": "<svg>...</svg>",
  "metadata": {
    "level": "container",
    "element_count": 12,
    "relationship_count": 15,
    "generated_at": "2026-05-11T12:00:00Z"
  }
}
```

### POST /api/diagrams/diff

```json
// Request
{
  "diagram_id_a": "diag_abc123",
  "diagram_id_b": "diag_def456"
}

// Response 200
{
  "diff": {
    "added": [
      {"type": "container", "id": "auth-service", "name": "Auth Service"}
    ],
    "removed": [],
    "modified": []
  },
  "mermaid_a": "graph TB\n  A[API] --> B[DB]",
  "mermaid_b": "graph TB\n  A[API] --> B[DB]\n  A --> C[Auth Service]"
}
```

---

## Recursos

- Mermaid.js: https://mermaid.js.org/
- Leptos: https://leptos-rs.github.io/
- Notify (file watcher): https://docs.rs/notify/
- Axum WebSocket: https://docs.rs/axum/latest/axum/websocket/
