# Plan Fase 6 — Advanced Diagram Features

## Objetivo

Extender `cognicode-diagram` con features avanzadas de diagramación: diagramas de secuencia, state machines, activity diagrams, inferencia para más lenguajes, y capacidades de IA para generar resúmenes y explicaciones de los diagramas.

## Duracion Estimada: 3-4 semanas

## Pre-requisitos

- Fase 5 completada (D2 export, ER diagrams, deployment, benchmarking)
- `cognicode-diagram` production-ready con los 4 niveles C4

## Tasks

### T6.1: Sequence Diagrams (5 dias)

**Descripcion**: Generar diagramas de secuencia UML desde el call graph traversal.

**Pasos**:
- [ ] `inference/sequence_inference.rs` — `infer_sequence(call_graph, entry_point) -> SequenceModel`
- [ ] Deteccion de:
  - [ ] Actors (personas/sistemas que inician interacciones)
  - [ ] Lifelines (lineas de tiempo por objeto)
  - [ ] Messages (llamadas sincronas, asíncronas, responses)
  - [ ] Activation boxes (tiempo que un objeto está activo)
  - [ ] Return messages (respuestas con línea punteada)
  - [ ] Self-calls (llamadas de un objeto a sí mismo)
  - [ ] Async messages (con flecha abierta)
  - [ ] Loop/alt/opt fragments (desde control flow)
- [ ] Render en formatos:
  - [ ] Mermaid sequenceDiagram
  - [ ] PlantUML sequence diagram
  - [ ] SVG con layout horizontal (tiempo → eje X)
- [ ] MCP tool: `generate_sequence_diagram` — Input: `directory`, `entry_symbol`, `format`

**Criterio de aceptacion**: `generate_sequence_diagram(entry_symbol="main")` genera un sequence diagram correcto mostrando las llamadas desde main hasta 3 niveles de profundidad.

---

### T6.2: State Machine Diagrams (4 dias)

**Descripcion**: Detectar state machines desde el código y renderizar diagramas de estados.

**Pasos**:
- [ ] `inference/state_machine_inference.rs` — `infer_state_machine(symbol) -> StateMachineModel`
- [ ] Deteccion de estados desde:
  - [ ] Enums con varianteschtml `state_` prefix
  - [ ] Structs con campo `state: StateEnum`
  - [ ] Pattern matching exhaustivo sobre estados
  - [ ] Transiciones: funciones que llaman `transition_to()`, `set_state()`
  - [ ] Actions de entrada/salida ( `on_enter_*`, `on_exit_*`)
  - [ ] Guards (condiciones en if/while dentro de transiciones)
- [ ] Modelo:
  - [ ] States: initial, final, regular, choice
  - [ ] Transitions: from, to, trigger, guard, action
  - [ ] Events que disparan transiciones
- [ ] Render:
  - [ ] Mermaid stateDiagram-v2
  - [ ] PlantUML state diagram
- [ ] MCP tool: `generate_state_machine` — Input: `symbol_name`, `format`

**Criterio de aceptacion**: Para un enum con 5 estados y transiciones detectadas, genera un state diagram que muestra todos los estados y sus transiciones.

---

### T6.3: Activity Diagrams (3 dias)

**Descripcion**: Generar diagramas de actividad (flujo de control) desde funciones.

**Pasos**:
- [ ] `inference/activity_inference.rs` — `infer_activity(function) -> ActivityModel`
- [ ] Deteccion de:
  - [ ] Start/End nodes
  - [ ] Action nodes (sentencias executable)
  - [ ] Decision nodes (if/else, match)
  - [ ] Merge nodes (reunión de branches)
  - [ ] Fork/Join nodes (paralel execution)
  - [ ] Loop nodes (for/while)
- [ ] Render:
  - [ ] Mermaid flowchart (TB/LR)
  - [ ] PlantUML activity diagram
- [ ] MCP tool: `generate_activity_diagram` — Input: `symbol_name`, `format`

**Criterio de aceptacion**: Para una función con if/else y un loop, genera un activity diagram que muestra el flujo de control completo.

---

### T6.4: TypeScript/JavaScript Inference (4 dias)

**Descripcion**: Extender la inferencia C4 para TypeScript y JavaScript.

**Pasos**:
- [ ] `config_parsers/nodejs.rs` — Parsear package.json para L2
  - [ ] Detectar bins, scripts, dependencies
  - [ ] Identificar tipo de proyecto (CLI, library, service)
- [ ] `inference/ts_inference.rs` — Inferencia L1-L3 para TypeScript
  - [ ] L1: Detectar actors desde imports (react, express, next)
  - [ ] L2: Containers desde package.json
  - [ ] L3: Components desde tsconfig.json paths o estructura de directorios
- [ ] Soporte para:
  - [ ] TypeScript .ts/.tsx files
  - [ ] JavaScript .js/.jsx files
  - [ ] tsconfig.json path aliases
  - [ ] Next.js app directory structure
  - [ ] React component patterns
- [ ] Tests con proyectos TypeScript reales

**Criterio de aceptacion**: `reverse_engineer_c4` genera diagramas correctos para un proyecto Next.js con 5 componentes React.

---

### T6.5: Multi-Language Workspace Support (3 dias)

**Descripcion**: Soporte para proyectos con múltiples lenguajes (Rust + TypeScript + Python).

**Pasos**:
- [ ] `inference/multi_lang_engine.rs` — Orquestar inferencia por lenguaje
- [ ] Merge de modelos C4 desde:
  - [ ] Rust (Cargo.toml, mod structure)
  - [ ] TypeScript (package.json, tsconfig)
  - [ ] Python (pyproject.toml, setup.py)
  - [ ] Go (go.mod)
- [ ] Deteccion de relaciones inter-lenguaje:
  - [ ] FFI calls (Rust → C, Python → Rust via pyo3)
  - [ ] gRPC/HTTP entre servicios
  - [ ] Shared databases
- [ ] Contenedor único con múltiples tecnologías

**Criterio de aceptacion**: Para un workspace Rust + TypeScript, genera un C4 container diagram mostrando ambos como containers separados con sus dependencias.

---

### T6.6: AI Diagram Summarization (3 dias)

**Descripcion**: Usar IA para generar explicaciones en lenguaje natural de los diagramas.

**Pasos**:
- [ ] `summarization/mod.rs` — Pipeline de summarization
- [ ] Integracion con LLM (via cognicode-core LLM trait):
  - [ ] Resumen del sistema en texto
  - [ ] Explicacion de relaciones clave
  - [ ] Identificacion de puntos de falla potenciales
  - [ ] Sugerencias de arquitectura
- [ ] Templates de summary:
  - [ ] Executive summary (1 parrafo)
  - [ ] Technical overview (para developers)
  - [ ] Risk assessment
- [ ] MCP tool: `summarize_diagram` — Input: `diagram_mermaid`, `style` (executive/technical)

**Criterio de aceptacion**: `summarize_diagram(diagram_mermaid, style="technical")` devuelve un resumen de 3-5 párrafos explicando el sistema.

---

### T6.7: Diagram Diff & Versioning (3 dias)

**Descripcion**: Comparar versiones de diagramas entre analysis runs.

**Pasos**:
- [ ] `diff/mod.rs` — Calcular diff entre dos C4Workspace
- [ ] Deteccion de cambios:
  - [ ] Elementos nuevos/eliminados
  - [ ] Relaciones nuevas/eliminadas
  - [ ] Cambios en atributos (description, technology)
- [ ] Render del diff:
  - [ ] Mermaid con estilos highlight (verde=nuevo, rojo=eliminado)
  - [ ] JSON con changeset estructurado
- [ ] Integracion con cognicode-db:
  - [ ] Guardar diagram snapshots en analysis_runs
  - [ ] API para obtener diff entre runs
- [ ] MCP tool: `diff_diagrams` — Input: `run_id_a`, `run_id_b`

**Criterio de aceptacion**: `diff_diagrams(run_a, run_b)` muestra los cambios en formato Mermaid con colores diferenciando añadidos/eliminados.

---

## Dependencias

```
T6.1 (Sequence) ← F5 (D2, reverse_engineer_c4)
T6.2 (State Machine) ← F5
T6.3 (Activity) ← F5
T6.4 (TS/JS) ← F3 (context inference)
T6.5 (Multi-lang) ← T6.4
T6.6 (AI Sum.) ← T6.1-T6.5 (necesita diagramas completos)
T6.7 (Diff) ← F5 (cognicode-db integration)
```

## Milestones

| Milestone | Task | Criterio de aceptacion |
|---|---|---|
| M9: Sequence | T6.1 | generate_sequence_diagram produce Mermaid válido |
| M10: State Machine | T6.2 | Detecta estados desde enum + transiciones |
| M11: Activity | T6.3 | Activity diagram con fork/join para paralelismo |
| M12: TypeScript | T6.4 |reverse_engineer_c4 funciona con proyecto Next.js |
| M13: Multi-lang | T6.5 | Workspace Rust+TS genera container diagram unificado |
| M14: AI Summary | T6.6 | LLM genera resumen comprensible del diagrama |
| M15: Diff | T6.7 | diff_diagrams muestra changeset visual |

## Recursos

- Mermaid sequenceDiagram: https://mermaid.js.org/syntax/sequenceDiagram.html
- Mermaid stateDiagram: https://mermaid.js.org/syntax/stateDiagram.html
- Mermaid flowchart: https://mermaid.js.org/syntax/flowchart.html
- PlantUML sequence: https://plantuml.com/sequence-diagram
- PlantUML state: https://plantuml.com/state-diagram
