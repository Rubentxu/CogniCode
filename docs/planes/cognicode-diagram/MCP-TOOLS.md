# MCP Tools — Especificacion

## Tools de Diagramacion C4

Todas las tools son registradas en `cognicode-mcp` via `rmcp_adapter.rs` y delegan a `cognicode-diagram::mcp::tools`.

---

### 1. `generate_c4_code` (L4)

**Descripcion**: Genera diagrama de clases UML (nivel Code del C4 Model) a partir del analisis de un archivo o modulo.

```json
{
  "input": {
    "path": "crates/cognicode-core/src/domain/aggregates",
    "format": "code | svg",
    "depth": 2,
    "module_filter": null,
    "visibility": "public | all",
    "theme": "dracula",
    "include_external_deps": false
  },
  "output": {
    "mermaid_code": "classDiagram\n  class CallGraph { ... }",
    "svg": "<svg>...</svg>",
    "element_count": 15,
    "relationship_count": 23,
    "format": "code"
  }
}
```

---

### 2. `generate_c4_components` (L3)

**Descripcion**: Genera diagrama de componentes internos de un container especifico.

```json
{
  "input": {
    "directory": ".",
    "container_name": "cognicode-core",
    "format": "code | svg",
    "detail_level": "summary | detailed",
    "show_coupling_score": true,
    "theme": "tokyo-night"
  },
  "output": {
    "mermaid_code": "flowchart TB\n  subgraph cognicode-core ...",
    "svg": "<svg>...</svg>",
    "components": [
      { "name": "domain", "type": "Module", "symbol_count": 45 },
      { "name": "infrastructure", "type": "Module", "symbol_count": 120 }
    ],
    "relationships": [
      { "from": "interface/mcp", "to": "application/services", "type": "uses", "edge_count": 15 }
    ]
  }
}
```

---

### 3. `generate_c4_containers` (L2)

**Descripcion**: Genera diagrama de containers (crates, binarios, librerias, DBs) del sistema.

```json
{
  "input": {
    "directory": ".",
    "format": "code | svg | dsl",
    "show_coupling": true,
    "theme": "catppuccin-mocha"
  },
  "output": {
    "mermaid_code": "flowchart TB\n  subgraph CogniCode ...",
    "dsl": "workspace \"CogniCode\" { model { ... } }",
    "containers": [
      { "name": "cognicode-mcp", "type": "Service", "technology": "Rust/Tokio" },
      { "name": "cognicode-core", "type": "Library", "technology": "Rust" }
    ],
    "data_stores": [
      { "name": "SQLite Graph Cache", "technology": "rusqlite" }
    ]
  }
}
```

---

### 4. `generate_c4_context` (L1)

**Descripcion**: Genera diagrama de contexto del sistema mostrando actores y sistemas externos.

```json
{
  "input": {
    "directory": ".",
    "format": "code | svg | dsl | plantuml",
    "theme": "nord"
  },
  "output": {
    "mermaid_code": "flowchart TB\n  AI_Agent --> CogniCode ...",
    "dsl": "workspace { model { person ai_agent ... } }",
    "plantuml": "@startuml ... @enduml",
    "system_name": "CogniCode",
    "persons": [
      { "name": "AI Agent", "description": "AI agents consuming MCP tools" },
      { "name": "Developer", "description": "CLI user" }
    ],
    "external_systems": [
      { "name": "SQLite", "description": "Local graph persistence" },
      { "name": "OpenTelemetry Collector", "description": "Metrics export" }
    ]
  }
}
```

---

### 5. `generate_c4_dynamic`

**Descripcion**: Genera diagrama de secuencia (dynamic view) a partir de un entry point del call graph.

```json
{
  "input": {
    "entry_point": "call_tool",
    "max_depth": 5,
    "format": "code",
    "group_by": "module | component"
  },
  "output": {
    "mermaid_code": "sequenceDiagram\n  participant MCP ...",
    "participant_count": 6,
    "interaction_count": 18
  }
}
```

---

### 6. `reverse_engineer_c4` (Meta-tool)

**Descripcion**: Ejecuta el pipeline completo de reverse engineering C4 en un solo llamado.

```json
{
  "input": {
    "directory": ".",
    "levels": ["L1", "L2", "L3"],
    "format": "all | mermaid | plantuml | dsl | svg",
    "output_dir": "./docs/architecture",
    "theme": "dracula",
    "detail_level": "summary | detailed"
  },
  "output": {
    "generated_files": [
      "docs/architecture/context.mmd",
      "docs/architecture/containers.mmd",
      "docs/architecture/components-cognicode-core.mmd",
      "docs/architecture/workspace.dsl",
      "docs/architecture/context.puml"
    ],
    "stats": {
      "persons": 2,
      "external_systems": 3,
      "containers": 13,
      "components": 8,
      "total_elements": 26,
      "total_relationships": 34
    },
    "elapsed_ms": 3200
  }
}
```

## Registro en cognicode-mcp

```rust
// En rmcp_adapter.rs, dentro de la lista de tools:

Tool::new(
    "generate_c4_code",
    "Generate C4 Code-level class diagram from source analysis. Shows classes, structs, traits, and their UML relationships.",
    Arc::new(json!({ "type": "object", "properties": { ... } }))
),
Tool::new(
    "generate_c4_components",
    "Generate C4 Component diagram showing internal modules and their dependencies within a container.",
    Arc::new(json!({ "type": "object", "properties": { ... } }))
),
// ... etc
```

## Handler Pattern

```rust
// cognicode-diagram/src/mcp/tools.rs

pub async fn handle_generate_c4_code(
    graph: &CallGraph,
    input: GenerateC4CodeInput,
) -> Result<GenerateC4CodeOutput, DiagramError> {
    let engine = InferenceEngine::new(graph);
    let code_elements = engine.infer_code_elements(&input.path, input.depth)?;
    let mermaid = render_class_diagram(&code_elements, &input.into())?;
    let svg = if input.format == "svg" {
        Some(render_svg(&code_elements, &input.theme)?)
    } else {
        None
    };
    Ok(GenerateC4CodeOutput { mermaid_code: mermaid, svg, ... })
}
```
