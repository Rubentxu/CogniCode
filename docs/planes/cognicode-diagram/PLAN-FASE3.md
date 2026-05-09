# Plan Fase 3 — L1 Context + Structurizr DSL + Full C4

## Objetivo

Completar los 4 niveles del C4 Model con la capa de Context (L1), generar Structurizr DSL como formato de intercambio estandar, y crear la meta-tool de reverse engineering completo.

## Duracion Estimada: 2 semanas

## Pre-requisitos

- Fase 2 completada (L2 Container + L3 Component inference)
- Fase 1 completada (L4 Code + model types)

## Tasks

### T3.1: Context Inference (L1) (3 dias)

**Descripcion**: Inferir el diagrama de contexto del sistema: actores, sistemas externos, y el sistema principal.

**Pasos**:
- [ ] `inference/context_inference.rs` — `infer_context(project_dir, CallGraph) -> SoftwareSystem`
- [ ] Detectar nombre del sistema desde `Cargo.toml` workspace o `package.json` name
- [ ] Inferir personas/actores:
  - [ ] `clap` + `fn main()` → Person "Developer" location=External
  - [ ] `rmcp` + stdio transport → Person "AI Agent" location=External
  - [ ] `actix-web`/`axum`/`rocket` → Person "End User" location=External
  - [ ] `std::io::stdin()` → Person "User (stdin)" location=External
- [ ] Inferir sistemas externos:
  - [ ] `rusqlite` → SoftwareSystem "SQLite" location=External
  - [ ] `opentelemetry-otlp` → SoftwareSystem "OpenTelemetry Collector" location=External
  - [ ] `reqwest`/`hyper` → SoftwareSystem "External HTTP API" location=External
  - [ ] `lsp-types` → SoftwareSystem "LSP Client" location=External
  - [ ] `redis` crate → SoftwareSystem "Redis" location=External
  - [ ] `postgres`/`sqlx` → SoftwareSystem "PostgreSQL" location=External
- [ ] Inferir relaciones contexto:
  - [ ] Actor → Sistema: "Uses" (desde tipo de interfaz CLI/MCP/HTTP)
  - [ ] Sistema → External: "Reads/Writes" (desde tipo de dep: DB, API, Queue)
- [ ] Confidence scoring: cada deteccion tiene un score, solo se incluyen las >0.5

**Criterio de aceptacion**: Para CogniCode, detecta al menos "AI Agent", "Developer" como personas y "SQLite", "OTel Collector" como sistemas externos.

---

### T3.2: Structurizr DSL Generator (4 dias)

**Descripcion**: Generar Structurizr DSL (.dsl) como formato de intercambio estandar consumible por structurizr-rs y Structurizr Cloud.

**Pasos**:
- [ ] `render/structurizr_dsl.rs` — `render_structurizr_dsl(C4Workspace) -> String`
- [ ] Generar bloque `workspace` con nombre y descripcion
- [ ] Generar bloque `model`:
  - [ ] `person` declarations
  - [ ] `softwareSystem` con `container` hijos
  - [ ] `component` dentro de containers
  - [ ] `->` relationships con labels y technology
- [ ] Generar bloque `views`:
  - [ ] `systemContext` view
  - [ ] `container` view
  - [ ] `component` view por container
  - [ ] `dynamic` view (opcional)
  - [ ] `autoLayout` en todas las vistas
- [ ] Generar bloque `styles`:
  - [ ] Colors por tipo de elemento (azul systems, verde containers, etc.)
  - [ ] Shapes (Person, Cylinder para DB, RoundedBox para services)
- [ ] Soporte para temas remotos (`!theme` directive)
- [ ] Test: DSL generado es parseable por structurizr-rs sin errores

**Criterio de aceptacion**: El DSL generado para CogniCode es parseable por `structurizr-rs validate` sin errores y produce diagramas correctos.

---

### T3.3: PlantUML C4 Renderer (2 dias)

**Descripcion**: Renderizar modelo C4 como PlantUML con macros C4.

**Pasos**:
- [ ] `render/plantuml.rs` — `render_plantuml_c4(C4Workspace, view_type) -> String`
- [ ] Usar macros `!include https://raw.githubusercontent.com/.../C4_Context.puml`
- [ ] `System_Context` view: `System(...)` + `Person(...)` + `Rel(...)`
- [ ] `Container` view: `Container(...)` + `ContainerDb(...)` + `Rel(...)`
- [ ] `Component` view: `Component(...)` + `Rel(...)`
- [ ] Styling con `LAYOUT_WITH_LEGEND()`

**Criterio de aceptacion**: PlantUML output para CogniCode context view es renderizable por PlantUML server.

---

### T3.4: Meta-Tool `reverse_engineer_c4` (2 dias)

**Descripcion**: Tool MCP que ejecuta el pipeline completo de reverse engineering en un solo llamado.

**Pasos**:
- [ ] `mcp/tools.rs` — `handle_reverse_engineer_c4(HandlerContext, input) -> Result<Output>`
- [ ] Input: `directory`, `levels: ["L1","L2","L3","L4"]`, `format` (mermaid/plantuml/dsl/all), `output_dir`
- [ ] Pipeline:
  1. `ensure_graph_built()` (reutiliza logica de core)
  2. `infer_context()` si L1 solicitado
  3. `infer_containers()` si L2 solicitado
  4. `infer_components()` si L3 solicitado
  5. `infer_code_elements()` si L4 solicitado
  6. Render en formato(s) solicitado(s)
  7. Si `output_dir`, escribir archivos
- [ ] Output: resumen con archivos generados, conteo de elementos por nivel
- [ ] Registrar tool en `cognicode-mcp`

**Criterio de aceptacion**: `reverse_engineer_c4` con `levels=["L1","L2","L3"]` y `format="all"` genera Mermaid + PlantUML + DSL para los 3 niveles en <5s.

---

### T3.5: Dynamic View (Sequence Diagrams) (2 dias)

**Descripcion**: Generar diagramas de secuencia a partir del traversal del CallGraph.

**Pasos**:
- [ ] `render/mermaid.rs` — `render_sequence_diagram(CallGraph, entry_point, max_depth) -> String`
- [ ] Usar `sequenceDiagram` syntax de Mermaid
- [ ] BFS desde entry point: cada llamada → `Caller ->> Callee: method_name()`
- [ ] Agrupar por lifeline (una por module/component)
- [ ] Detectar loops (retrocesos en BFS) → `loop` blocks
- [ ] Detectar condicionales (if/else en AST) → `alt` blocks (mejor esfuerzo)
- [ ] MCP tool: `generate_c4_dynamic` con input: `entry_point`, `max_depth`, `format`

**Criterio de aceptacion**: Para `CogniCodeHandler::call_tool` como entry point, genera diagrama de secuencia con al menos 5 interacciones.

---

### T3.6: Tests (1 dia)

- [ ] Test: `infer_context` detecta actores y externals para CogniCode
- [ ] Test: DSL generado es valido (parseable por structurizr-rs)
- [ ] Test: PlantUML output es renderizable
- [ ] Test: `reverse_engineer_c4` pipeline completo
- [ ] Test: Dynamic view genera sequence diagram con entry point conocido
- [ ] Test de rendimiento: pipeline completo <5s para CogniCode (~500 simbolos)

## Milestones

**M4**: `generate_c4_context` detecta "AI Agent", "Developer", "SQLite", "OTel Collector" para CogniCode.

**M5**: Structurizr DSL generado es parseable por structurizr-rs sin errores.

**M6**: `reverse_engineer_c4` genera las vistas L1+L2+L3 para CogniCode en <5s en formatos Mermaid, PlantUML y DSL.
