# Plan Fase 1 — Foundation + L4 Code Diagram

## Objetivo

Crear el crate `cognicode-diagram` con la estructura base y generar diagramas de clases UML (nivel L4 Code del C4 Model) a partir del `CallGraph` existente.

## Duracion Estimada: 2 semanas

## Tasks

### T1.1: Crate Skeleton (1 dia)

**Descripcion**: Crear el crate con la estructura de directorios y `Cargo.toml`.

**Pasos**:
- [ ] `cargo new --lib crates/cognicode-diagram`
- [ ] Configurar `Cargo.toml` con dependencias
- [ ] Anadir al workspace `Cargo.toml` principal
- [ ] Crear estructura de directorios: `model/`, `inference/`, `layout/`, `render/`, `mcp/`
- [ ] `lib.rs` con re-exports publicos

**Criterio de aceptacion**: `cargo build -p cognicode-diagram` compila sin errores.

---

### T1.2: C4 Model Types (2 dias)

**Descripcion**: Definir los tipos del modelo C4 que seran agnosticos del formato de output.

**Pasos**:
- [ ] `model/c4_types.rs` — `ElementId`, `Person`, `SoftwareSystem`, `Container`, `Component`, `CodeElement`
- [ ] `model/relationships.rs` — `C4Relationship`, `C4RelationshipKind` (11 tipos)
- [ ] `model/workspace.rs` — `C4Workspace` (agrupa model + views + styles)
- [ ] `model/views.rs` — `View`, `SystemContextView`, `ContainerView`, `ComponentView`, `CodeView`
- [ ] `model/styles.rs` — `ElementStyle`, `Theme`, `Shape` (Box, RoundedBox, Cylinder, Person, etc.)
- [ ] Derive `Serialize`, `Deserialize`, `Clone` para todos los tipos

**Criterio de aceptacion**: Tipos compilan, se pueden serializar a JSON, crear un workspace vacio.

---

### T1.3: UML Rules Engine (3 dias)

**Descripcion**: Motor de reglas que infiere relaciones UML desde el CallGraph.

**Pasos**:
- [ ] `inference/uml_rules.rs` — `UmlRule`, `UmlRelationKind` (7 tipos)
- [ ] Regla: `Inheritance` — `DependencyType::Inherits` → herencia (confidence: 1.0)
- [ ] Regla: `Realization` — `impl Trait for Struct` detectado via AST (confidence: 1.0)
- [ ] Regla: `Composition` — struct tiene campo `Vec<T>` / `Box<T>` / `T` owned (confidence: 0.9)
- [ ] Regla: `Aggregation` — struct tiene campo `&T` / `Option<&T>` (confidence: 0.7)
- [ ] Regla: `Association` — funcion recibe tipo como parametro (confidence: 0.6)
- [ ] Regla: `Dependency` — funcion crea instancia temporal (confidence: 0.5)
- [ ] Regla: `Contains` — `DependencyType::Contains` → composicion (confidence: 0.8)
- [ ] Sistema de confidence scoring — cada regla produce (kind, confidence)
- [ ] Resolucion de conflictos cuando multiples reglas aplican al mismo par

**Criterio de aceptacion**: Dado un CallGraph con structs Rust que usan herencia, composicion y agregacion, el motor infiere correctamente >80% de las relaciones UML.

---

### T1.4: L4 Code Inference (3 dias)

**Descripcion**: Inferir elementos de codigo (L4) desde el CallGraph.

**Pasos**:
- [ ] `inference/code_inference.rs` — `infer_code_elements(CallGraph) -> Vec<CodeElement>`
- [ ] Convertir `SymbolKind::Class`/`Struct`/`Enum` → `CodeElementKind::Class`
- [ ] Convertir `SymbolKind::Trait` → `CodeElementKind::Interface`
- [ ] Convertir `SymbolKind::Method`/`Function` → `CodeElementKind::Method` con visibilidad
- [ ] Extraer atributos desde `FunctionSignature.parameters`
- [ ] Agrupar metodos dentro de sus structs/classes padre
- [ ] Aplicar `UmlRules` para inferir relaciones entre CodeElements
- [ ] `inference/engine.rs` — `InferenceEngine` como facade

**Criterio de aceptacion**: `infer_code_elements` para `cognicode-core/src/domain/aggregates/call_graph.rs` produce CodeElements para `CallGraph`, `SymbolId`, `Symbol` con sus relaciones.

---

### T1.5: Mermaid Class Diagram Renderer (2 dias)

**Descripcion**: Renderizar CodeElements como diagrama de clases Mermaid.

**Pasos**:
- [ ] `render/mermaid.rs` — `render_class_diagram(Vec<CodeElement>, options) -> String`
- [ ] Generar bloques de clase con secciones: `class Foo { +method() -field }`
- [ ] Renderizar relaciones: `Foo <|-- Bar` (herencia), `Foo *-- Baz` (composicion), etc.
- [ ] Soporte para `classDiagram` syntax de Mermaid
- [ ] Labels en relaciones con confianza
- [ ] Opciones: `max_depth`, `module_filter`, `visibility_filter`

**Criterio de aceptacion**: El output Mermaid es valido (parseable por `mermaid-rs-renderer`), muestra al menos 3 tipos de relaciones UML correctamente.

---

### T1.6: MCP Tool `generate_c4_code` (1 dia)

**Descripcion**: Exponer la generacion de diagramas L4 como tool MCP.

**Pasos**:
- [ ] `mcp/tools.rs` — `handle_generate_c4_code(HandlerContext, input) -> Result<Output>`
- [ ] Input schema: `path`, `depth`, `format` (code/svg), `module_filter`, `visibility`
- [ ] Output schema: `mermaid_code`, `svg` (opcional), `element_count`, `relationship_count`
- [ ] Registrar tool en `cognicode-mcp` rmcp_adapter

**Criterio de aceptacion**: Desde un agente MCP, `generate_c4_code` con `path="crates/cognicode-core/src/domain"` produce un diagrama Mermaid valido.

---

### T1.7: Tests (2 dias)

**Descripcion**: Tests unitarios y de integracion.

**Pasos**:
- [ ] Test fixtures: proyecto Rust simple con herencia, composicion, traits
- [ ] Tests unitarios de `UmlRules` — cada regla con entrada/salida esperada
- [ ] Tests de `code_inference` — verificar elementos inferidos
- [ ] Tests de `mermaid_renderer` — verificar output valido
- [ ] Test de integracion: CallGraph completo → diagrama Mermaid
- [ ] Test de rendimiento: <100ms para inferencia + render de 100 simbolos

**Criterio de aceptacion**: `cargo test -p cognicode-diagram` pasa con >90% coverage en `inference/` y `render/`.

## Dependencias de Fase

```
T1.1 (Skeleton) ──→ T1.2 (Types) ──→ T1.3 (UML Rules) ──→ T1.4 (L4 Inference)
                                                                     │
                                                     T1.5 (Mermaid) ←─┘
                                                          │
                                                     T1.6 (MCP Tool)
                                                          │
                                                     T1.7 (Tests)
```

## Riesgos de Fase

| Riesgo | Mitigacion |
|---|---|
| `rust-sugiyama` no instalable | Layout se pospone a Fase 4, F1 solo genera texto |
| UML rules producen muchos falsos positivos | Empezar con 3 reglas de alta confianza, iterar |
| Mermaid `classDiagram` syntax limitada | Usar `flowchart` como fallback |

## Milestone M1

**Criterio**: `generate_c4_code` produce un diagrama de clases Mermaid correcto para `crates/cognicode-core/src/domain/aggregates/call_graph.rs` mostrando `CallGraph`, `Symbol`, `SymbolId` con sus relaciones de composicion y dependencia.
