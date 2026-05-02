# Arquitectura de Servidores MCP: cognicode-mcp + cognicode-quality

## Resumen Ejecutivo

Este documento describe la arquitectura de los **dos servidores MCP** que componen el workspace de CogniCode: `cognicode-mcp` (inteligencia de código) y `cognicode-quality` (análisis de calidad). Ambos comparten la librería `cognicode-core` como base, y utilizan un cache compartido basado en Redb para persistir el call graph y los parse trees.

La decisión de dividir en dos servidores independientes en lugar de uno solo responde a principios de **aislamiento de dominio**, **evolución independiente**, y **simplicidad del servidor core**:

- **Aislamiento de dominio**: quality analysis y code intelligence son dominios distintos con ciclos de cambio independientes
- **Evolución independiente**: cognicode-quality puede añadir reglas, smell detectors, y gates sin tocar el servidor de inteligencia de código
- **Servidor core más ligero**: cognicode-mcp permanece dedicado a análisis de código puro, sin la complejidad de quality gates y ratings

---

## 1. Visión General

### 1.1 Dos Servidores, Un Workspace

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         COGNICODE WORKSPACE                              │
│                                                                          │
│  ┌────────────────────────────┐    ┌───────────────────────────────┐  │
│  │    cognicode-mcp            │    │    cognicode-quality           │  │
│  │    Puerto: 8000             │    │    Puerto: 8001                │  │
│  │    32 herramientas         │    │    ~15 herramientas           │  │
│  │                             │    │                               │  │
│  │  • analyze_*               │    │  • check_quality              │  │
│  │  • get_*                   │    │  • quality_delta              │  │
│  │  • find_*                  │    │  • check_boundaries           │  │
│  │  • refactor_*              │    │  • detect_duplications        │  │
│  │  • export_*                │    │  • evaluate_gate              │  │
│  │  • list_*                  │    │  • list_rules                  │  │
│  │                            │    │  • test_rule                  │  │
│  │                            │    │  • get_profile                 │  │
│  │                            │    │  • check_lint                  │  │
│  │                            │    │  • list_smells                 │  │
│  │                            │    │  • list_duplications          │  │
│  │                            │    │  • compute_debt                │  │
│  │                            │    │  • rate_project                │  │
│  │                            │    │  • load_adrs                   │  │
│  └────────────┬───────────────┘    └───────────────┬───────────────┘  │
│               │                                      │                   │
│               └──────────────────┬───────────────────┘                   │
│                                  ▼                                       │
│  ┌───────────────────────────────────────────────────────────────────┐   │
│  │                      cognicode-core (lib)                          │   │
│  │                                                                       │   │
│  │  tree-sitter  │  CallGraph  │  ComplexityCalculator  │  Redb       │   │
│  │  ImpactAnalyzer  │  CycleDetector  │  SymbolIndex  │  Cache      │   │
│  └───────────────────────────────────────────────────────────────────┘   │
│                                  │                                        │
│                                  ▼                                        │
│  ┌───────────────────────────────────────────────────────────────────┐   │
│  │                  ~/.cognicode/cache/ (Redb on-disk)                 │   │
│  │                                                                       │   │
│  │  parse_tree_cache.redb  │  call_graph_cache.redb  │  symbols.redb  │   │
│  └───────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

### 1.2 ¿Por qué dos servidores en lugar de uno?

| Criterio | Servidor único | Dos servidores |
|----------|---------------|---------------|
| **Acoplamiento** | Quality y code intelligence acoplados | Dominios independientes |
| **Velocidad de evolución** | Cambios en quality pueden afectar code intelligence | Evolución independiente |
| **Consumo de memoria** | Todo en un proceso | Puede aislarse si es necesario |
| **Configuración MCP** | Un servidor, una configuración | Dos servidores, dos puertos |
| **Debugging** | Más complejo | Más claro, cada servidor tiene su dominio |
| **Dependencias** | Un binary | quality puede añadir dependencias de linting sin afectar core |

La separación permite que `cognicode-mcp` permanezca estable y ligero, mientras `cognicode-quality` evoluciona rápidamente con nuevas reglas, smells, y gates.

### 1.3 Posición en el Workspace

```
cognicode-workspace/
├── Cargo.toml                      # Workspace root
├── crates/
│   ├── cognicode-core/             # Librería compartida (lib)
│   │   └── src/
│   │       ├── domain/
│   │       │   ├── aggregates/
│   │       │   │   ├── call_graph.rs    # CallGraph aggregate
│   │       │   │   └── symbol.rs        # Symbol aggregate
│   │       │   └── services/
│   │       │       ├── complexity.rs       # ComplexityCalculator
│   │       │       ├── impact_analyzer.rs  # ImpactAnalyzer
│   │       │       └── cycle_detector.rs   # CycleDetector (Tarjan SCC)
│   │       ├── application/
│   │       │   └── analysis_service.rs
│   │       └── infrastructure/
│   │           ├── graph/
│   │           │   └── pet_graph_store.rs  # PetGraph + Redb persistence
│   │           └── cache/
│   │               └── redb_cache.rs       # Shared cache layer
│   │
│   ├── cognicode-mcp/             # Servidor MCP (code intelligence)
│   │   └── src/main.rs             # Puerto 8000
│   │
│   ├── cognicode-quality/          # NUEVO: Servidor MCP (quality analysis)
│   │   └── src/main.rs             # Puerto 8001
│   │
│   └── cognicode-axiom/            # Crate interno (NO es servidor)
│       └── src/
│           ├── lib.rs              # Facade: axiom::analyze()
│           ├── rules/              # Rule management + declare_rule! macro
│           ├── quality/            # SOLID, connascence, LCOM, boundaries, delta
│           ├── smells/             # Code smell detectors
│           ├── gates/              # Quality gates evaluation
│           ├── debt/               # Technical debt computation
│           ├── ratings/            # Project ratings (A-F)
│           └── linters/            # clippy, eslint, semgrep wrappers
```

---

## 2. cognicode-mcp

### 2.1 Descripción

`servidor MCP` dedicado a **inteligencia de código**: análisis estático, búsqueda de símbolos, refactoring, y exportación de grafos. Es el servidor original del workspace y permanece sin cambios en su dominio funcional.

### 2.2 Estado Actual

| Atributo | Valor |
|----------|-------|
| **Puerto** | 8000 |
| **Herramientas** | 32 |
| **Dependencias** | Solo `cognicode-core` |
| **Cache** | Comparado con quality via Redb |

### 2.3 Herramientas Principales

```rust
// Dominio: code intelligence
tools: [
    "analyze_file",           // Análisis estático de un archivo
    "analyze_symbol",        // Análisis de un símbolo específico
    "analyze_call_hierarchy",// Call hierarchy completa
    "get_symbols",           // Lista símbolos en un archivo
    "get_outline",           // Outline de un archivo
    "find_usages",           // Find all usages
    "find_incoming",         // Quién llama a este símbolo
    "find_outgoing",         // Qué llama este símbolo
    "find_dead_code",        // Código no utilizado
    "find_cycles",           // Dependencias cíclicas
    "get_complexity",        // Métricas de complejidad
    "get_impact",            // Análisis de impacto
    "refactor_rename",       // Rename symbol
    "refactor_extract",      // Extract function/method
    "refactor_inline",       // Inline function
    "export_call_graph",     // Exportar call graph
    "export_dependencies",   // Exportar matrix de dependencias
    "build_lightweight_index",// Build symbol index
    "query_symbol_index",    // Query symbol index
    "semantic_search",       // Búsqueda semántica
    // ... y otras hasta 32
]
```

### 2.4 Dependencias

```
cognicode-mcp
    └── cognicode-core (lib)
```

---

## 3. cognicode-quality

### 3.1 Descripción

**Nuevo servidor MCP** dedicado al análisis de calidad de código: reglas, smells, gates, technical debt, ratings, y linting. Es un binary separado que expone ~15 herramientas MCP.

### 3.2 Estado Actual

| Atributo | Valor |
|----------|-------|
| **Puerto** | 8001 |
| **Herramientas** | ~15 |
| **Dependencias** | `cognicode-core` + `cognicode-axiom` |
| **Cache** | Compartido con cognicode-mcp via Redb |

### 3.3 Herramientas Principales

```rust
// Dominio: quality analysis
tools: [
    "check_quality",         // Análisis completo de calidad
    "quality_delta",         // Compara calidad antes/después de cambios
    "check_boundaries",      // Valida DDD/hexagonal boundaries
    "detect_duplications",   // Detecta duplicación de código
    "evaluate_gate",         // Evalúa si se cumple un quality gate
    "list_rules",            // Lista reglas de calidad definidas
    "test_rule",             // Prueba una regla contra código
    "get_profile",           // Obtiene profile de calidad configurado
    "check_lint",            // Ejecuta linter externo (clippy/eslint/semgrep)
    "list_smells",           // Lista code smells detectados
    "list_duplications",     // Lista duplicaciones encontradas
    "compute_debt",          // Calcula technical debt en horas
    "rate_project",          // Califica proyecto (A-F)
    "load_adrs",             // Carga ADRs y extrae reglas implícitas
]
```

### 3.4 Diagrama de Arquitectura

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    cognicode-quality (binary)                            │
│                         Puerto 8001                                      │
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                    QualityHandler                                 │    │
│  │            (rmcp Handler — entry point)                         │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                  │                                       │
│                                  ▼                                       │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                         axiom-lib                                │    │
│  │              (Facade — pure quality analysis)                    │    │
│  │                                                                       │    │
│  │   axiom::analyze() → QualityReport                               │    │
│  │   axiom::evaluate_gate() → GateResult                            │    │
│  │   axiom::compute_debt() → DebtReport                             │    │
│  │   axiom::rate_project() → Rating                                 │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                  │                                       │
│                                  ▼                                       │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                      cognicode-axiom (lib)                       │    │
│  │                  Pure quality analysis — NO governance          │    │
│  │                                                                       │    │
│  │   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐      │    │
│  │   │  rules/  │  │ quality/ │  │ smells/  │  │  gates/  │      │    │
│  │   │          │  │          │  │          │  │          │      │    │
│  │   │ declare_ │  │ SOLID   │  │ long_    │  │ evaluate_ │      │    │
│  │   │ rule!    │  │ connasce│  │ function │  │ gate     │      │    │
│  │   │ store    │  │ LCOM    │  │ god_     │  │ threshold │      │    │
│  │   │ validate │  │ delta   │  │ class    │  │          │      │    │
│  │   └──────────┘  └──────────┘  └──────────┘  └──────────┘      │    │
│  │                                                                       │    │
│  │   ┌──────────┐  ┌──────────┐  ┌──────────┐                       │    │
│  │   │   debt/  │  │ ratings/ │  │ linters/ │                       │    │
│  │   │          │  │          │  │          │                       │    │
│  │   │ compute  │  │ rate    │  │ clippy   │                       │    │
│  │   │ estimate │  │ project  │  │ eslint   │                       │    │
│  │   │          │  │          │  │ semgrep  │                       │    │
│  │   └──────────┘  └──────────┘  └──────────┘                       │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                  │                                       │
│                                  ▼                                       │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                     cognicode-core (lib)                          │    │
│  │                   Shared infrastructure                           │    │
│  │                                                                       │    │
│  │   CallGraph  │  ComplexityCalculator  │  ImpactAnalyzer          │    │
│  │   CycleDetector  │  SymbolIndex  │  tree-sitter parser           │    │
│  │   Redb cache layer                                              │    │
│  └─────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 4. cognicode-axiom (crate interno)

### 4.1 Descripción

`cognicode-axiom` es una **librería interna** (no un servidor ni binary) que proporciona análisis de calidad de código puro. **NO contiene governance, policies, audit, ni reflection**. Su única responsabilidad es quality analysis.

### 4.2 Módulos

```
cognicode-axiom/src/
├── lib.rs              # Facade: axiom::analyze(), axiom::evaluate_gate()
├── rules/
│   ├── mod.rs
│   ├── store.rs        # RuleStore: CRUD de reglas
│   ├── validator.rs    # Validación de expresiones de reglas
│   └── macro.rs        # declare_rule! macro para definir reglas
├── quality/
│   ├── mod.rs
│   ├── solid.rs        # Análisis de principios SOLID
│   ├── connascence.rs   # Análisis de connascence
│   ├── lcom.rs         # Lack of Cohesion of Methods
│   ├── delta.rs        # Quality before/after comparison
│   └── boundaries.rs   # DDD/Hexagonal boundary validation
├── smells/
│   ├── mod.rs
│   ├── detector.rs     # Code smell detector framework
│   ├── long_function.rs
│   ├── god_class.rs
│   ├── feature_envy.rs
│   └── data_clump.rs
├── gates/
│   ├── mod.rs
│   ├── evaluator.rs    # Gate evaluation engine
│   └── threshold.rs    # Threshold configuration
├── debt/
│   ├── mod.rs
│   └── calculator.rs   # Technical debt computation
├── ratings/
│   ├── mod.rs
│   └── scorer.rs       # Project rating (A-F)
└── linters/
    ├── mod.rs
    ├── clippy.rs       # Rust clippy wrapper
    ├── eslint.rs       # JS/TS eslint wrapper
    └── semgrep.rs      # Semgrep wrapper
```

### 4.3 API Principal (lib.rs)

```rust
// crates/cognicode-axiom/src/lib.rs

pub mod rules;
pub mod quality;
pub mod smells;
pub mod gates;
pub mod debt;
pub mod ratings;
pub mod linters;

use crate::quality::QualityReport;
use crate::gates::{GateResult, QualityGate};
use crate::debt::DebtReport;
use crate::ratings::ProjectRating;

/// Análisis completo de calidad sobre un conjunto de paths
pub fn analyze(
    paths: &[std::path::PathBuf],
    profile: &QualityProfile,
) -> QualityReport {
    // 1. Construir/reutilizar call graph desde cache
    // 2. Ejecutar reglas en paralelo via rayon
    // 3. Detectar smells
    // 4. Calcular complejidad
    // 5. Generar QualityReport
}

/// Evalúa si se cumple un quality gate
pub fn evaluate_gate(
    graph: &CallGraph,
    gate: &QualityGate,
) -> GateResult {
    // Compara métricas contra thresholds del gate
}

/// Calcula technical debt en horas
pub fn compute_debt(
    graph: &CallGraph,
    smells: &[CodeSmell],
) -> DebtReport {
    // Suma deuda por smell × factor de complejidad
}

/// Califica proyecto (A-F)
pub fn rate_project(report: &QualityReport) -> ProjectRating {
    // Basado en score ponderado de métricas
}
```

### 4.4 declare_rule! Macro

```rust
// crates/cognicode-axiom/src/rules/macro.rs

/// Macro para declarar reglas de calidad de forma declarativa
/// Uso:
/// ```ignore
/// declare_rule!(
///     "no-naked-pointers",
///     RuleKind::Security,
///     "No se permiten punteros crudos en código nuevo",
///     r#"ast.match(
///         .*PointerType,
///         "Uso de puntero crudo detectado"
///     )"#
/// );
/// ```
#[macro_export]
macro_rules! declare_rule {
    ($id:expr, $kind:expr, $description:expr, $condition:expr $(,)?) => {
        $crate::rules::Rule {
            id: $id.to_string(),
            kind: $kind,
            description: $description.to_string(),
            condition: $condition.to_string(),
            severity: $crate::rules::Severity::Error,
            enabled: true,
        }
    };
}
```

---

## 5. cognicode-core (compartido)

### 5.1 Descripción

Librería compartida que provee la **infraestructura de análisis de código**: parsing, call graphs, métricas de complejidad, y cache.

### 5.2 Componentes Reutilizados

| Componente | Descripción | Usado por |
|------------|-------------|-----------|
| **tree-sitter** | Parser multi-lenguaje | Ambos servidores |
| **CallGraph** | Grafo de llamadas con Tarjan SCC | Ambos servidores |
| **ComplexityCalculator** | Ciclomática, cognitiva, anidamiento | Ambos servidores |
| **ImpactAnalyzer** | Análisis de impacto de cambios | Ambos servidores |
| **CycleDetector** | Detección de ciclos | Ambos servidores |
| **SymbolIndex** | Índice de símbolos | Ambos servidores |
| **Redb cache** | Persistencia en disco | Ambos servidores |

### 5.3 Cache Compartido

```
~/.cognicode/cache/
├── parse_tree_cache.redb    # Parse trees de tree-sitter
├── call_graph_cache.redb    # Call graphs construidos
├── symbol_index.redb        # Índice de símbolos
└── complexity_cache.redb   # Métricas de complejidad
```

Ambos servidores acceden al mismo directorio de cache. El cache usa timestamps de archivo para invalidación automática.

---

## 6. Flujo de una Request Quality

```
Agent (Claude Code)
    │
    │  MCP tool call: check_quality
    │  {
    │    "paths": ["src/**/*.rs"],
    │    "profile": "default"
    │  }
    ▼
cognicode-quality (puerto 8001)
    │
    │  QualityHandler::execute_tool("check_quality", ...)
    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       axiom::analyze()                               │
│                                                                       │
│  1. Obtener/reconstruir CallGraph desde cache                        │
│     └─► cognicode_core::CallGraph::build()                          │
│         └─► Redb cache check → parse if miss                        │
│                                                                       │
│  2. Ejecutar reglas en paralelo via rayon                            │
│     └─► rule_store.iter_enabled()                                   │
│         └─► rayon::parallel::parallel_map()                           │
│             ├─► rule_1.evaluate(graph)                              │
│             ├─► rule_2.evaluate(graph)                              │
│             └─► rule_N.evaluate(graph)                              │
│                                                                       │
│  3. Detectar code smells                                             │
│     └─► smells::Detector::scan(graph)                                │
│                                                                       │
│  4. Calcular métricas de complejidad                                 │
│     └─► ComplexityCalculator::analyze(graph)                         │
│                                                                       │
│  5. Generar QualityReport                                            │
│     └─► QualityReport { issues, score, debt_hours, rating }         │
└─────────────────────────────────────────────────────────────────────┘
    │
    │  QualityReport
    ▼
Agent recibe: {
  "score": 0.73,
  "grade": "C",
  "issues": [
    {
      "type": "SOLIDViolation",
      "rule": "single-responsibility",
      "symbol": "UserService",
      "severity": "warning"
    },
    {
      "type": "CodeSmell",
      "smell": "GodClass",
      "symbol": "PaymentProcessor"
    }
  ],
  "debt_hours": 42.5,
  "rating": "C"
}
```

---

## 7. Dependencias entre Crates

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          cognicode-mcp                                  │
│                          (binary, puerto 8000)                          │
│                                                                          │
│  dependencies:                                                          │
│    └── cognicode-core (lib)                                             │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
                                 │
                                 │ depends on
                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        cognicode-core (lib)                              │
│                                                                          │
│  purpose: Shared infrastructure for code analysis                        │
│                                                                          │
│  provides:                                                             │
│    • CallGraph (Tarjan SCC, path finding, cycle detection)              │
│    • ComplexityCalculator (cyclomatic, cognitive, nesting)              │
│    • ImpactAnalyzer (fan-in, fan-out, ripple effect)                    │
│    • CycleDetector                                                      │
│    • SymbolIndex + tree-sitter parsing                                  │
│    • Redb cache layer                                                  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
                                 ▲
                                 │ depends on
                                 │
┌─────────────────────────────────────────────────────────────────────────┐
│                       cognicode-axiom (lib)                              │
│                                                                          │
│  purpose: Pure quality analysis — NO governance                          │
│                                                                          │
│  dependencies:                                                          │
│    └── cognicode-core (lib)                                             │
│                                                                          │
│  modules:                                                              │
│    • rules/      (store, validator, declare_rule! macro)                │
│    • quality/    (SOLID, connascence, LCOM, delta, boundaries)         │
│    • smells/     (long_function, god_class, feature_envy, data_clump)   │
│    • gates/      (evaluator, threshold)                                │
│    • debt/       (calculator)                                          │
│    • ratings/    (scorer)                                              │
│    • linters/    (clippy, eslint, semgrep)                             │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
                                 │
                                 │ depends on
                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                     cognicode-quality (binary)                           │
│                     (puerto 8001, ~15 tools)                            │
│                                                                          │
│  dependencies:                                                          │
│    ├── cognicode-core (lib)                                             │
│    └── cognicode-axiom (lib)                                            │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Ejemplo de main.rs para cognicode-quality

```rust
// crates/cognicode-quality/src/main.rs

use cognicode_quality::axiom;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::cache::redb_cache::RedbCache;
use rmcp::server::{McpServer, ServerHandler};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
pub struct QualityHandler {
    cache: RedbCache,
    profile: axiom::QualityProfile,
}

impl QualityHandler {
    pub fn new(cache_dir: std::path::PathBuf) -> Result<Self> {
        let cache = RedbCache::open(cache_dir)?;
        let profile = axiom::QualityProfile::default();

        Ok(Self { cache, profile })
    }

    async fn check_quality(
        &self,
        paths: Vec<std::path::PathBuf>,
        profile_name: Option<String>,
    ) -> Result<axiom::QualityReport> {
        let profile = profile_name
            .map(|n| axiom::QualityProfile::load(&n))
            .unwrap_or_else(|| self.profile.clone());

        // Obtener o construir call graph
        let graph = self.cache.get_or_build_call_graph(&paths)?;

        // Ejecutar análisis
        let report = axiom::analyze(&paths, &profile);

        Ok(report)
    }

    async fn evaluate_gate(
        &self,
        gate_name: String,
    ) -> Result<axiom::GateResult> {
        let gate = axiom::QualityGate::load(&gate_name)?;
        let graph = self.cache.get_or_build_call_graph(&[])?;

        Ok(axiom::evaluate_gate(&graph, &gate))
    }

    async fn compute_debt(
        &self,
        paths: Vec<std::path::PathBuf>,
    ) -> Result<axiom::DebtReport> {
        let graph = self.cache.get_or_build_call_graph(&paths)?;
        let smells = axiom::smells::detect_all(&graph);

        Ok(axiom::compute_debt(&graph, &smells))
    }
}

#[rmcp::async_trait]
impl ServerHandler for QualityHandler {
    async fn handle_tool_call(
        &self,
        tool: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, rmcp::Error> {
        match tool {
            "check_quality" => {
                let paths: Vec<std::path::PathBuf> =
                    serde_json::from_value(arguments["paths"].clone())?;
                let profile = arguments["profile"]
                    .as_str()
                    .map(|s| s.to_string());

                let report = self.check_quality(paths, profile).await?;
                Ok(serde_json::to_value(report)?)
            }

            "evaluate_gate" => {
                let gate_name: String =
                    serde_json::from_value(arguments["gate"].clone())?;

                let result = self.evaluate_gate(gate_name).await?;
                Ok(serde_json::to_value(result)?)
            }

            "compute_debt" => {
                let paths: Vec<std::path::PathBuf> =
                    serde_json::from_value(arguments["paths"].clone())?;

                let debt = self.compute_debt(paths).await?;
                Ok(serde_json::to_value(debt)?)
            }

            // ... otras herramientas

            _ => Err(rmcp::Error::ToolNotFound(tool.to_string())),
        }
    }

    fn list_tools(&self) -> Vec<rmcp::Tool> {
        vec![
            rmcp::Tool {
                name: "check_quality".into(),
                description: "Análisis completo de calidad".into(),
                input_schema: check_quality_schema(),
            },
            rmcp::Tool {
                name: "evaluate_gate".into(),
                description: "Evalúa si se cumple un quality gate".into(),
                input_schema: evaluate_gate_schema(),
            },
            rmcp::Tool {
                name: "compute_debt".into(),
                description: "Calcula technical debt en horas".into(),
                input_schema: compute_debt_schema(),
            },
            // ... otras 12 herramientas
        ]
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = "127.0.0.1:8001".parse()?;
    let listener = TcpListener::bind(addr).await?;

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".cognicode");

    let handler = QualityHandler::new(cache_dir)?;

    let server = McpServer::new(handler);
    server.accept(listener).await?;

    Ok(())
}
```

---

## 9. Configuración del Cliente (Claude Code)

### 9.1 Configuración de Dos Servidores MCP

```json
// ~/.claude/settings.json (o claude_desktop_config.json)

{
  "mcpServers": {
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--port", "8000"],
      "env": {
        "CACHI_CACHE_DIR": "~/.cognicode/cache"
      }
    },
    "cognicode-quality": {
      "command": "cognicode-quality",
      "args": ["--port", "8001"],
      "env": {
        "CACHI_CACHE_DIR": "~/.cognicode/cache"
      }
    }
  }
}
```

### 9.2 Variables de Entorno Comunes

| Variable | Descripción | Default |
|----------|-------------|---------|
| `COGNICODE_CACHE_DIR` | Directorio de cache compartido | `~/.cognicode/cache` |
| `COGNICODE_LOG_LEVEL` | Nivel de logging | `info` |
| `COGNICODE_MAX_PARALLELISM` | Threads para análisis paralelo | `num_cpus` |

### 9.3 Ejemplo de Uso desde Claude Code

```typescript
// El agente puede llamar herramientas de ambos servidores:

// Desde cognicode-mcp (code intelligence)
const symbols = await mcp_cognicode.get_symbols({
  file: "src/auth/service.rs"
});

// Desde cognicode-quality (quality analysis)
const quality = await mcp_cognicode_quality.check_quality({
  paths: ["src/**/*.rs"],
  profile: "default"
});

const debt = await mcp_cognicode_quality.compute_debt({
  paths: ["src/**/*.rs"]
});
```

---

## Resumen

La arquitectura de dos servidores MCP permite:

1. **Separación de dominios clara**: code intelligence vs quality analysis
2. **Evolución independiente**: cada servidor puede cambiar sin afectar al otro
3. **Cache compartido**: parse trees y call graphs se reutilizan entre servidores
4. **Simplicidad del core**: `cognicode-mcp` permanece ligero y estable
5. **Infraestructura compartida**: `cognicode-core` es el foundation layer para ambos

`cognicode-axiom` como crate interno proporciona análisis de calidad puro sin las responsabilidades de governance, audit, o reflection que fueron removidas de la arquitectura anterior.
