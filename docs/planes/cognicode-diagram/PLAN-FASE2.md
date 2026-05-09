# Plan Fase 2 — L3 Component + L2 Container

## Objetivo

Implementar inferencia de diagramas de Componentes (L3) y Containers (L2) del C4 Model, detectando modulos internos, crates/binarios, y sus relaciones.

## Duracion Estimada: 2 semanas

## Pre-requisitos

- Fase 1 completada (model types, inference engine, Mermaid renderer)
- `cognicode-core` con `CallGraph.find_module_dependencies()` operativo

## Tasks

### T2.1: Component Inference (L3) (4 dias)

**Descripcion**: Inferir componentes internos de un container a partir de la estructura de directorios y el CallGraph.

**Pasos**:
- [ ] `inference/component_inference.rs` — `infer_components(CallGraph, container_path) -> Vec<Component>`
- [ ] Agrupar simbolos por directorio padre (`CallGraph::module_from_file` logic)
- [ ] Clasificar directorios por convencion: `domain/` → Domain, `infrastructure/` → Infrastructure, `interface/` → Interface, `application/` → Application
- [ ] Detectar interfaces publicas: simbolos `pub` en `mod.rs` o archivo principal del directorio
- [ ] Inferir relaciones inter-componente desde edges del CallGraph que cruzan directorios
- [ ] Calcular coupling score entre componentes (reutilizar `find_module_dependencies`)
- [ ] Detectar patrones comunes: Repository pattern, Service pattern, Handler pattern

**Criterio de aceptacion**: Para `cognicode-core`, infiere al menos 5 componentes (domain, infrastructure, interface, application, sandbox_core) con sus relaciones de dependencia.

---

### T2.2: Container Inference (L2) — Cargo.toml Parser (3 dias)

**Descripcion**: Inferir containers (bins, libs, services) desde la configuracion del build system.

**Pasos**:
- [ ] `inference/config_parsers/cargo.rs` — Parser de Cargo.toml
  - [ ] Parsear `workspace.members` → lista de crates
  - [ ] Parsear `[[bin]]` targets → ContainerType::Executable/Service
  - [ ] Parsear `[lib]` target → ContainerType::Library
  - [ ] Parsear `[dependencies]` → inter-container relationships
  - [ ] Parsear `name`, `description`, `version`
- [ ] `inference/container_inference.rs` — `infer_containers(project_dir) -> Vec<Container>`
- [ ] Detectar tecnologia: `tokio` → "Async Runtime", `rmcp` → "MCP Protocol", `clap` → "CLI"
- [ ] Inferir descripciones desde `description` field o `main.rs` doc comments
- [ ] Detectar data stores: `rusqlite` → DataStore, `redis` → DataStore
- [ ] Construir grafo de dependencias inter-container

**Criterio de aceptacion**: Para el workspace CogniCode, infiere 13 containers con tipos correctos (2 executables, 11 libraries) y sus dependencias.

---

### T2.3: Container Inference — Multi-lenguaje (2 dias)

**Descripcion**: Extender la inferencia L2 a Node.js y Python.

**Pasos**:
- [ ] `inference/config_parsers/nodejs.rs` — Parser de `package.json`
  - [ ] `scripts.start` → Service container
  - [ ] `main` field → Library/CLI container
  - [ ] `dependencies` → relationships
- [ ] `inference/config_parsers/python.rs` — Parser de `pyproject.toml` / `setup.py`
  - [ ] `[project.scripts]` → Executable containers
  - [ ] `[tool.poetry]` dependencies → relationships
- [ ] `inference/config_parsers/mod.rs` — Auto-detectar build system por archivos presentes

**Criterio de aceptacion**: Para un proyecto Node.js con `package.json`, infiere containers correctamente.

---

### T2.4: Mermaid Renderer — Component y Container (2 dias)

**Descripcion**: Extender el renderer Mermaid para diagramas de Component y Container.

**Pasos**:
- [ ] `render/mermaid_c4.rs` — Renderer especializado C4
- [ ] `render_component_diagram(components, options) -> String`
  - [ ] Usar `flowchart TB` con subgraphs para agrupar por container
  - [ ] Shapes por tipo: cylindro para DB, rounded box para service, box para library
  - [ ] Labels en edges con dependency count
- [ ] `render_container_diagram(containers, options) -> String`
  - [ ] C4-style: containers dentro del boundary del sistema
  - [ ] Actors externos fuera del boundary
  - [ ] Data stores con shape cylindro
- [ ] Opciones: `direction` (TB/LR), `show_coupling_score`, `theme`

**Criterio de aceptacion**: Diagrama de containers de CogniCode muestra 13 crates, sus dependencias, y 2 data stores (SQLite + Graph cache) en Mermaid valido.

---

### T2.5: MCP Tools (2 dias)

**Descripcion**: Exponer L2 y L3 como tools MCP.

**Pasos**:
- [ ] `generate_c4_components` — Input: `directory`, `container_name`, `format`, `detail_level`
- [ ] `generate_c4_containers` — Input: `directory`, `format`, `show_coupling`
- [ ] Registrar en `cognicode-mcp` rmcp_adapter
- [ ] Tests de integracion con proyecto CogniCode como fixture

**Criterio de aceptacion**: `generate_c4_containers` para CogniCode produce diagrama con 13 containers y sus relaciones.

---

### T2.6: Tests (1 dia)

**Descripcion**: Tests para L2 y L3.

- [ ] Test fixtures: workspace Rust con 3 crates (1 bin, 2 libs)
- [ ] Test: `infer_containers` detecta bin + 2 libs + dependencias
- [ ] Test: `infer_components` detecta modulos internos
- [ ] Test: Mermaid output es valido para L2 y L3
- [ ] Test: Proyecto CogniCode como caso de integracion

## Dependencias

```
T2.1 (L3 Component) ──→ T2.4 (Mermaid L3) ──→ T2.5 (MCP Tools)
T2.2 (L2 Cargo.toml) ─→ T2.4 (Mermaid L2) ──→ T2.5
T2.3 (L2 Multi-lang) ─→ T2.4
                                            T2.6 (Tests)
```

## Milestone M2 + M3

**M2**: `generate_c4_components` produce diagrama de componentes de `cognicode-core` mostrando domain/, infrastructure/, interface/, application/ como cajas con relaciones.

**M3**: `generate_c4_containers` produce diagrama de containers del workspace CogniCode mostrando 13 crates con tipos (Service/Library/DataStore) y sus dependencias inter-crate.
