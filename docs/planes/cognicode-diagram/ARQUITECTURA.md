# CogniCode Diagram — Arquitectura

## Posicion en el Workspace

```
CogniCode/
├── crates/
│   ├── cognicode/          # CLI binary
│   ├── cognicode-core/     # Motor de inteligencia de codigo (CallGraph, Symbol, tree-sitter)
│   ├── cognicode-db/       # Persistencia SQLite
│   ├── cognicode-axiom/    # Motor de reglas + code smells
│   ├── cognicode-quality/  # Analisis de calidad (gates, linting)
│   ├── cognicode-mcp/      # MCP server (expone tools)
│   ├── cognicode-diagram/  # ← NUEVO: Diagramacion inferida + C4 Model
│   └── ...
```

## Principios de Diseno

1. **Separacion total de core** — `cognicode-diagram` depende de `cognicode-core` via su API publica, nunca accede a internals
2. **Pipeline de 3 fases** — Inference → Layout → Render, cada fase independiente y testeable
3. **Inferencia lazy** — Solo calcula cuando se pide un diagrama, no en build time
4. **Multi-formato** — El modelo C4 es agnostico del formato de output
5. **Incremental** — Reutiliza FileManifest de core para saber que re-calcular

## Estructura del Crate

```
cognicode-diagram/
├── Cargo.toml
├── src/
│   ├── lib.rs                    # Re-exports publicos
│   │
│   ├── model/                    # Tipos del modelo C4 (agnostico de formato)
│   │   ├── mod.rs
│   │   ├── c4_types.rs           # Person, SoftwareSystem, Container, Component, CodeElement
│   │   ├── relationships.rs      # C4Relationship, C4RelationshipKind
│   │   ├── workspace.rs          # C4Workspace = model + views + styles
│   │   ├── views.rs              # SystemContextView, ContainerView, ComponentView, DynamicView
│   │   └── styles.rs             # ElementStyle, Theme
│   │
│   ├── inference/                # Fase 1: Inferencia desde codigo → modelo C4
│   │   ├── mod.rs
│   │   ├── engine.rs             # InferenceEngine: orquesta las 4 capas
│   │   ├── code_inference.rs     # L4: CallGraph → CodeElements + UML relations
│   │   ├── component_inference.rs # L3: CallGraph modules → Components
│   │   ├── container_inference.rs # L2: Cargo.toml/package.json → Containers
│   │   ├── context_inference.rs  # L1: Deps + heuristics → Persons + External Systems
│   │   ├── uml_rules.rs          # Motor de reglas UML (composicion, herencia, etc.)
│   │   └── config_parsers/
│   │       ├── mod.rs
│   │       ├── cargo.rs          # Parsea Cargo.toml
│   │       ├── nodejs.rs         # Parsea package.json
│   │       └── python.rs         # Parsea pyproject.toml / setup.py
│   │
│   ├── layout/                   # Fase 2: Computa posiciones de nodos
│   │   ├── mod.rs
│   │   ├── sugiyama.rs           # Wrapper de rust-sugiyama con extensiones
│   │   ├── port_assigner.rs      # Asigna puertos a nodos por tipo de relacion
│   │   ├── compound.rs           # Nodos compuestos (parent-children)
│   │   └── types.rs              # LayoutedNode, LayoutedEdge, Point, Port
│   │
│   ├── render/                   # Fase 3: Modelo C4 (+ layout) → formato de output
│   │   ├── mod.rs
│   │   ├── mermaid.rs            # Mermaid C4 (flowchart con estilos C4)
│   │   ├── mermaid_c4.rs         # Mermaid con macros C4 especificas
│   │   ├── plantuml.rs           # PlantUML con macros C4
│   │   ├── structurizr_dsl.rs    # Structurizr DSL (.dsl)
│   │   ├── svg.rs                # SVG nativo (usa layout coordinates)
│   │   └── d2.rs                 # D2 lang
│   │
│   └── mcp/                      # Integration con cognicode-mcp
│       ├── mod.rs
│       └── tools.rs              # Handler functions llamadas desde cognicode-mcp
│
└── tests/
    ├── fixtures/                 # Proyectos de ejemplo para tests
    │   ├── rust_project/
    │   ├── python_project/
    │   └── mixed_project/
    ├── inference_tests.rs
    ├── render_tests.rs
    └── integration_tests.rs
```

## Flujo de Datos

```
                        cognicode-core
                    ┌─────────────────────┐
                    │   CallGraph          │
                    │   Symbol (21 kinds)  │
                    │   DependencyType (8) │
                    │   tree-sitter        │
                    └─────────┬───────────┘
                              │ referencia (Arc/clone)
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                     cognicode-diagram                            │
│                                                                  │
│  ┌─────────────────┐    ┌──────────────┐    ┌────────────────┐  │
│  │   INFERENCE      │    │    LAYOUT     │    │    RENDER      │  │
│  │                  │    │              │    │                │  │
│  │ CallGraph ──────►│───►│ C4Workspace  │───►│ Mermaid code   │──┼──► String
│  │ Cargo.toml ─────►│    │ + positions  │    │ PlantUML .puml │──┼──► String
│  │ package.json ───►│    │              │    │ Structurizr DSL│──┼──► .dsl file
│  │                  │    │ rust-sugiyama│    │ SVG            │──┼──► SVG bytes
│  │ L4: code         │    │ port_assign  │    │ D2             │──┼──► String
│  │ L3: component    │    │ compound     │    │                │  │
│  │ L2: container    │    │              │    │                │  │
│  │ L1: context      │    │              │    │                │  │
│  └─────────────────┘    └──────────────┘    └────────────────┘  │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────────┐
                    │   cognicode-mcp     │
                    │   (registra tools)  │
                    └─────────────────────┘
```

## Tipos Core del Modelo C4

```rust
// model/c4_types.rs

pub enum C4Element {
    Person(Person),
    SoftwareSystem(SoftwareSystem),
    Container(Container),
    Component(Component),
    CodeElement(CodeElement),
}

pub struct Person {
    pub id: ElementId,
    pub name: String,
    pub description: String,
    pub location: PersonLocation, // Internal, External
}

pub struct SoftwareSystem {
    pub id: ElementId,
    pub name: String,
    pub description: String,
    pub location: ElementLocation, // Internal, External
    pub containers: Vec<Container>,
}

pub struct Container {
    pub id: ElementId,
    pub name: String,
    pub container_type: ContainerType, // Service, Library, DataStore, Executable
    pub technology: String,
    pub description: String,
    pub path: Option<PathBuf>,
    pub components: Vec<Component>,
}

pub struct Component {
    pub id: ElementId,
    pub name: String,
    pub component_type: ComponentType, // Module, Interface, Controller, Repository
    pub technology: String,
    pub description: String,
    pub path: Option<PathBuf>,
    pub code_elements: Vec<CodeElement>,
}

pub struct CodeElement {
    pub id: ElementId,
    pub name: String,
    pub kind: CodeElementKind, // Class, Struct, Enum, Trait, Function, Method
    pub visibility: Visibility, // Public, Private, Protected
    pub attributes: Vec<Attribute>,
    pub methods: Vec<Method>,
    pub relationships: Vec<UmlRelationship>,
}

pub enum ContainerType {
    Service,
    Library,
    DataStore,
    Executable,
    Queue,
}

pub enum C4RelationshipKind {
    Uses,
    Calls,
    DependsOn,
    SendsTo,
    ReadsFrom,
    WritesTo,
    Inherits,
    Implements,
    Composes,
    Aggregates,
}
```

## Inferencia — Reglas UML

```rust
// inference/uml_rules.rs

pub enum UmlRelationKind {
    Inheritance,      // struct Foo extends Bar
    Realization,      // impl Trait for Struct
    Composition,      // struct owns Vec<Inner> (lifetime tied)
    Aggregation,      // struct holds &Inner (borrowed)
    Association,      // fn uses Type in params
    Dependency,       // fn creates temporary instance
}

pub struct UmlRule {
    pub name: &'static str,
    pub condition: fn(&Symbol, &CallGraph) -> Option<UmlRelationKind>,
    pub confidence: f64, // 0.0 - 1.0
}

// Reglas de ejemplo:
// 1. Si un struct tiene un campo Vec<OtroStruct> → Composition (confidence: 0.9)
// 2. Si un struct tiene un campo Option<Box<dyn Trait>> → Realization (confidence: 0.85)
// 3. Si un struct tiene un campo &Other → Aggregation (confidence: 0.7)
// 4. Si un impl block implementa un trait → Realization (confidence: 1.0)
// 5. Si una funcion tiene un parametro de tipo otro struct → Association (confidence: 0.6)
// 6. Si DependencyType::Inherits → Inheritance (confidence: 1.0)
// 7. Si DependencyType::Contains → Composition (confidence: 0.8)
```

## Inferencia — Heuristicas por Nivel C4

### L1: Context

| Senal en el codigo | Actor/External inferido | Confidence |
|---|---|---|
| `clap` + `fn main()` | Person: "Developer (CLI)" | 0.9 |
| `rmcp` + `transport-io` | Person: "AI Agent (MCP)" | 0.95 |
| `reqwest`/`hyper` imports | External: "HTTP API" | 0.7 |
| `rusqlite` imports | External: "SQLite Database" | 0.85 |
| `opentelemetry-otlp` imports | External: "OpenTelemetry Collector" | 0.8 |
| `tokio::net::TcpListener` | External: "TCP Clients" | 0.6 |
| `std::io::stdin()` | Person: "User (stdin)" | 0.7 |

### L2: Container

| Senal en Cargo.toml | Container inferido |
|---|---|
| `[[bin]]` target | ContainerType::Executable o Service |
| `[lib]` present | ContainerType::Library |
| `name = "foo-mcp"` | Service + technology: "MCP Protocol" |
| `name = "foo-cli"` | Executable + technology: "CLI" |
| `name = "foo-db"` | Library + technology: "Data Layer" |
| `rusqlite` in deps | DataStore: "SQLite" |
| `tokio` + `[bin]` | Service + technology: "Tokio async runtime" |

### L3: Component

| Senal en estructura | Component inferido |
|---|---|
| Directorio `domain/` | "Domain Layer" |
| Directorio `infrastructure/` | "Infrastructure Layer" |
| Directorio `interface/` o `api/` | "Interface Layer" |
| Directorio `application/` | "Application Layer" |
| Archivo `mod.rs` con pub exports | "Module Interface" |
| Archivo `traits.rs` | "Abstractions" |
| Archivo `handlers.rs` | "Request Handlers" |
| Archivo `models.rs` o `dto.rs` | "Data Transfer Objects" |

## Dependencias

```toml
[dependencies]
# Internal
cognicode-core = { path = "../cognicode-core" }

# Graph layout
rust-sugiyama = "0.6"
petgraph = { workspace = true }

# Parsing
toml = "0.8"               # Cargo.toml parsing
serde = { workspace = true }
serde_json = { workspace = true }

# Async
tokio = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Parallelism
rayon = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

## Integracion con cognicode-mcp

El crate `cognicode-mcp` registra las tools de diagramacion en su `rmcp_adapter.rs`:

```rust
// En cognicode-mcp/src/main.rs o rmcp_adapter.rs
// Se anaden las tools de cognicode_diagram::mcp al server

// Las tools se registran como cualquier otra MCP tool:
Tool::new("generate_c4_code", "Generate C4 Code-level diagram", ...),
Tool::new("generate_c4_components", "Generate C4 Component diagram", ...),
Tool::new("generate_c4_containers", "Generate C4 Container diagram", ...),
Tool::new("generate_c4_context", "Generate C4 System Context diagram", ...),
Tool::new("reverse_engineer_c4", "Full C4 reverse engineering pipeline", ...),
```

Los handlers delegan a `cognicode-diagram::mcp::tools` pasando el `CallGraph` del `HandlerContext`.
