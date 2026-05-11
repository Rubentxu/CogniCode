# Plan Fase 5 — Polish + Deploy + Extras

## Objetivo

Implementar diagramas de deployment y ER, export D2, integracion con dashboard, benchmarking y documentacion API para hacer `cognicode-diagram` production-ready.

## Duracion Estimada: 2 semanas

## Pre-requisitos

- Fase 1 completada (model types, inference engine, Mermaid renderer)
- Fase 3 completada (reverse_engineer_c4 meta-tool)
- Fase 4 completada (layout engine, SVG renderer)
- `cognicode-dashboard` accesible para integracion

## Tasks

### T5.1: D2 Export (2 dias)

**Descripcion**: Implementar export al lenguaje D2 (de Terrastruct) para diagramas C4.

**Pasos**:
- [ ] `render/d2.rs` — `render_d2(C4Workspace, view_type) -> String`
- [ ] Sintaxis D2 para containers, edges, shapes, styles
- [ ] Soporte para los 4 niveles C4:
  - [ ] L1: actors, software systems, external systems
  - [ ] L2: containers dentro de system boundaries
  - [ ] L3: components dentro de container boundaries
  - [ ] L4: code elements (classes, functions)
- [ ] Shapes D2: cylindo para databases, rectangle para servicios, diamond para decisions
- [ ] Estilos: themes compatibles con D2 (dark/light)
- [ ] MCP tool: actualizar `reverse_engineer_c4` para soportar `format="d2"`

**Criterio de aceptacion**: `reverse_engineer_c4` con `format="d2"` genera output D2 valido que Terrastruct puede renderizar para los 4 niveles C4.

---

### T5.2: Deployment Diagrams (3 dias)

**Descripcion**: Inferir y renderizar diagramas de deployment desde Dockerfile y docker-compose.yml.

**Pasos**:
- [ ] `inference/deployment_inference.rs` — `infer_deployment(project_dir) -> DeploymentModel`
- [ ] Parsear Dockerfile:
  - [ ] Detectar imagen base, puerto EXPOSE, CMD/ENTRYPOINT
  - [ ] Detectar variables de entorno (ENV) y labels
  - [ ] Detectar multiples servicios FROM (multi-stage build)
- [ ] Parsear docker-compose.yml/docker-compose.yaml:
  - [ ] Servicios → deployment nodes
  - [ ] Networks → network labels
  - [ ] Volumes → persistent storage
  - [ ] Port mappings → exposed ports
  - [ ] Dependencies (depends_on) → relationships
- [ ] Mapear servicios → C4 deployment nodes con tecnologia
- [ ] Render en formatos:
  - [ ] Mermaid: deployment diagram con nodos rect, database, queue
  - [ ] PlantUML: deployment diagram con artefactos
  - [ ] D2: shapes de nodos y conexiones de red
  - [ ] SVG: usando el renderer de F4
- [ ] MCP tool: `generate_c4_deployment` — Input: `directory`, `format`, `theme`

**Criterio de aceptacion**: Para un proyecto con docker-compose.yml de 5 servicios, genera diagrama de deployment mostrando todos los servicios, sus puertos, redes y volumenes.

---

### T5.3: ER Diagrams (3 dias)

**Descripcion**: Inferir y renderizar diagramas entidad-relacion para proyectos con schemas de base de datos.

**Pasos**:
- [ ] `inference/er_inference.rs` — `infer_er_diagram(project_dir) -> ErModel`
- [ ] Deteccion de DB schemas desde:
  - [ ] Archivos SQL de migraciones (migrate/, db/migrations/)
  - [ ] Archivos .sql con CREATE TABLE
  - [ ] Modelos ORM (SQLx, Diesel, SeaORM, TypeORM, Prisma, Django models)
  - [ ] Configuracion de database URL (DATABASE_URL parsing)
- [ ] Parsear SQL CREATE TABLE:
  - [ ] Extraer nombre de tabla, columnas, tipos
  - [ ] Primary keys, foreign keys
  - [ ] Constraints (NOT NULL, UNIQUE, CHECK)
- [ ] Detectar relaciones:
  - [ ] Foreign keys → relaciones entre entidades
  - [ ] Cardinalidad (1:1, 1:N, N:N)
  - [ ] Relaciones implicitas por convencion de nombres
- [ ] Render como Mermaid erDiagram
- [ ] MCP tool: `generate_er_diagram` — Input: `directory`, `format`, `include_relationships`

**Criterio de aceptacion**: Para un proyecto Rust con migraciones SQLx, genera ER diagram con todas las tablas, columnas y relaciones correctamente detectadas.

---

### T5.4: Performance Benchmarking (1 dia)

**Descripcion**: Establecer benchmarks de rendimiento para validar que el sistema cumple las metricas objetivo.

**Pasos**:
- [ ] `benches/layout_benchmark.rs` — Criterion benchmark para layout
  - [ ] Layout de 20 nodos, 50 nodos, 100 nodos
  - [ ] Comparar Sugiyama vs no-op layout
- [ ] `benches/inference_benchmark.rs` — Benchmark para inference
  - [ ] Inferencia L1, L2, L3 desde cero
  - [ ] Inference desde cache (caso caliente)
- [ ] `benches/render_benchmark.rs` — Benchmark para rendering
  - [ ] Mermaid render, PlantUML render, SVG render, D2 render
- [ ] Targets de rendimiento:
  - [ ] <2s para generar C4 completo de CogniCode
  - [ ] <500ms para diagrama de container
  - [ ] <1s para layout + SVG de 50 nodos
- [ ] Integrar con CI (GitHub Actions)

**Criterio de aceptacion**: Todos los benchmarks pasan en CI con los targets establecidos.

---

### T5.5: Dashboard Integration (2 dias)

**Descripcion**: Exponer las herramientas de diagramacion via la API de `cognicode-dashboard`.

**Pasos**:
- [ ] Endpoints API en `cognicode-dashboard`:
  - [ ] `POST /api/diagrams/c4` — Generar diagrama C4
  - [ ] `GET /api/diagrams/{id}/svg` — Obtener diagrama como SVG
  - [ ] `GET /api/diagrams/{id}/mermaid` — Obtener como Mermaid
  - [ ] `GET /api/diagrams/{id}/d2` — Obtener como D2
- [ ] Renderizado SVG en paginas del dashboard:
  - [ ] Componente `DiagramViewer` para mostrar SVG
  - [ ] Selector de tema (light/dark)
  - [ ] Selector de nivel C4
- [ ] Auto-refresh on code changes:
  - [ ] WebSocket para notificar cambios en el proyecto
  - [ ] Regeneracion automatica del diagrama
  - [ ] Diff visual de cambios en el diagrama
- [ ] Integracion con `reverse_engineer_c4` MCP tool

**Criterio de aceptacion**: Dashboard muestra diagrama C4 SVG de CogniCode que se actualiza automaticamente al modificar codigo.

---

### T5.6: API Documentation (1 dia)

**Descripcion**: Documentar toda la API publica del crate.

**Pasos**:
- [ ] Rustdoc para todos los modulos:
  - [ ] `inference/` — Modulo de inferencia con ejemplos
  - [ ] `render/` — Modulo de renderizado con ejemplos
  - [ ] `layout/` — Motor de layout con ejemplos
  - [ ] `model/` — Tipos C4 con ejemplos
- [ ] Documentacion de funciones publicas:
  - [ ] `infer_containers()`, `infer_components()`, `infer_er_diagram()`
  - [ ] `render_mermaid()`, `render_plantuml()`, `render_svg()`, `render_d2()`
  - [ ] `compute_layout()`, `assign_ports()`
- [ ] Module-level docs explicando el flujo de datos
- [ ] Ejemplos en doc comments para cada func publica
- [ ] Generar HTML docs con `cargo doc`
- [ ] Publicar en docs.rs

**Criterio de aceptacion**: `cargo doc --open` genera documentacion completa sin warnings, con ejemplos ejecutables.

---

## Dependencias

```
T5.1 (D2 Export) ───────────────────────────────────→ Milestone M8
T5.2 (Deployment) ──────────────────────────────────→ Milestone M8
T5.3 (ER Diagrams) ────────────────────────────────→ Milestone M8
T5.4 (Benchmarking) ← T5.1, T5.2, T5.3
T5.5 (Dashboard) ← T5.1, T5.2, T5.3, T5.4
T5.6 (API Docs) ← T5.1, T5.2, T5.3
```

## Milestone M8

**Criterio**: Todas las tools MCP funcionan con proyectos Rust, Python, Go, TypeScript. El diagrama de deployment, ER, y C4 se generan correctamente para todos los lenguajes soportados.

(End of file - total 166 lines)
