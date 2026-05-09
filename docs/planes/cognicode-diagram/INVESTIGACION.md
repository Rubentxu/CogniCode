# Investigacion — Crates, Referencias y Hallazgos

## Crates Rust Relevantes

### Layout / Graph

| Crate | Version | Descripcion | Uso en cognicode-diagram |
|---|---|---|---|
| `rust-sugiyama` | 0.6 | Implementacion del algoritmo de Sugiyama sobre petgraph | Layout base para L2-L4 |
| `petgraph` | 0.6 | Estructuras de grafos dirigidos (ya en workspace) | Base compartida con cognicode-core |
| `structurizr-rs` | - (GitHub) | Implementacion Rust de Structurizr Lite con DSL parser, SVG render, export | Referencia para DSL syntax; potencialmente fork/dependency |

### Parsing

| Crate | Version | Descripcion | Uso |
|---|---|---|---|
| `toml` | 0.8 | Parser de TOML | Cargo.toml → L2 container inference |
| `serde_json` | 1.0 | JSON serialization | package.json, pyproject.toml parsing |
| `serde_yaml` | 0.9 | YAML serialization | docker-compose.yml parsing (L2 deploy) |

### Rendering

| Crate | Version | Descripcion | Uso |
|---|---|---|---|
| `mermaid-rs-renderer` | 0.2 | Render Mermaid → SVG (ya en workspace) | Render Mermaid a SVG con temas |
| `svg` | 0.18 | Generacion de SVG programatico | SVG nativo en Fase 4 |

## Proyectos de Referencia

### structurizr-rs
- **URL**: https://github.com/Helms-AI/structurizr-rs
- **Lenguaje**: Rust
- **Lo que aporta**: Parser DSL completo, render SVG, export Mermaid/PlantUML/D2/DOT, web server
- **Estructura de crates**: `structurizr-core`, `structurizr-dsl`, `structurizr-render`, `structurizr-export`, `structurizr-web`
- **Como lo usamos**: Como referencia para la spec DSL. Potencialmente como dependencia si publican crates.
- **Limitaciones**: No tiene inferencia automatica — requiere DSL manual. Solo renderiza lo que le das.

### Pyreverse (Python)
- **URL**: Parte de Pylint
- **Lo que aporta**: AST → diagrama de clases via `astroid`
- **Como inspira**: Patron de inferencia de relaciones: heuristica de tipos (coleccion → composicion)

### srcUML
- **Lo que aporta**: srcML (XML del codigo) → reglas XSLT → diagrama UML
- **Como inspira**: Pipeline de transformacion: codigo → representacion intermedia → reglas → diagrama

### PlantUML + ELK
- **Lo que aporta**: Layout de diagramas complejos con puertos y nodos compuestos
- **Como inspira**: ELK como referencia para layout con puertos (implementar en Fase 4)

## Algoritmos Clave

### Sugiyama (5 fases)
Implementado en `rust-sugiyama`:
1. **Ciclo elimination** — invertir aristas para hacer el grafo aciclico
2. **Layer assignment** — asignar cada nodo a una capa (eje Y)
3. **Crossing reduction** — reordenar nodos dentro de capas (baricentro)
4. **Node positioning** — asignar coordenadas X (Brandes-Kopf)
5. **Edge routing** — dibujar aristas (splines/orthogonal)

### Extensiones ELK sobre Sugiyama
Para implementar en Fase 4:
- **Port-aware crossing reduction** — baricentro calculado sobre puertos, no centros
- **Compound node layout** — layout recursivo de dentro hacia fuera
- **Orthogonal edge routing** — enrutado con angulos rectos evitando obstaculos

## Mapeo de DependencyType → UML/C4

| DependencyType (core) | UML Relation | C4 Relation | Regla de inferencia |
|---|---|---|---|
| `Calls` | Dependency | Uses | Directa |
| `Imports` | Dependency | DependsOn | Directa |
| `Inherits` | Inheritance | — | Directa |
| `UsesGeneric` | Realization | — | Si el generic es un trait |
| `References` | Association | Uses | Directa |
| `Defines` | Composition | Contains | Si parent es tipo de definicion |
| `AnnotatedBy` | Dependency | — | Indirecta |
| `Contains` | Composition | Contains | Directa |

## Mapeo de SymbolKind → C4 Code Element

| SymbolKind | C4 Code Element Kind | Notas |
|---|---|---|
| `Class`, `Struct` | Class | Agrupa fields + methods |
| `Enum` | Enum | Lista de variantes |
| `Trait`, `Interface` | Interface | Lista de firmas |
| `Function` | Function | Standalone |
| `Method` | Method | Pertenece a Class |
| `Constructor` | Constructor | Pertenece a Class |
| `Field`, `Property` | Field | Pertenece a Class |
| `Constant` | Constant | Standalone o pertenece a Class |
| `Module` | Package | Agrupa elementos |
| `Variable`, `Parameter` | (omitir) | Ruido para diagramas |

## Temas Disponibles (heredados de core)

14 temas configurados en `cognicode-core/infrastructure/mermaid/mod.rs`:
`catppuccin-mocha`, `catppuccin-latte`, `dracula`, `tokyo-night`, `tokyo-night-light`, `tokyo-night-storm`, `nord`, `nord-light`, `github-light`, `github-dark`, `solarized-light`, `solarized-dark`, `one-dark`, `zinc-dark`

## Formatos de Output

| Formato | Extension | Complejidad | Fase |
|---|---|---|---|
| Mermaid code | `.mmd` | Baja (generar texto) | F1 |
| Mermaid SVG | `.svg` | Baja (via mermaid-rs-renderer) | F1 |
| Structurizr DSL | `.dsl` | Media (spec completa) | F3 |
| PlantUML | `.puml` | Media (macros C4) | F3 |
| D2 | `.d2` | Baja | F5 |
| SVG nativo | `.svg` | Alta (layout propio) | F4 |

## Benchmarks de Referencia

| Operacion | Target | Limite |
|---|---|---|
| Inferencia L4 (100 simbolos) | <50ms | 100ms |
| Inferencia L2+L3 (workspace 13 crates) | <500ms | 1s |
| Pipeline completo L1-L4 (CogniCode) | <3s | 5s |
| Layout Sugiyama (50 nodos) | <100ms | 500ms |
| Render SVG (50 nodos) | <50ms | 200ms |
| Render Mermaid texto (50 nodos) | <10ms | 50ms |
