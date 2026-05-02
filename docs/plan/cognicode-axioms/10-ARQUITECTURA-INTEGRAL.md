# Arquitectura Definitiva: cognicode-quality + cognicode-mcp (sin governance)

## Resumen Ejecutivo

Este documento describe la **arquitectura final** de CogniCode: un sistema de análisis de calidad de código puro, sin gobernanza. El pivote es claro — se eliminó Cedar Policy, reflection, hooks y audit para dejar solo **code quality analysis**.

**Arquitectura de 2 servidores MCP**:
- `cognicode-mcp` (puerto 8000): 32 herramientas de inteligencia de código — sin cambios
- `cognicode-quality` (puerto 8001): análisis de calidad — NUEVO servidor dedicado

Ambos comparten `cognicode-core` (tree-sitter, CallGraph, ComplexityCalculator, ImpactAnalyzer, CycleDetector) y cache Redb en disco (`~/.cognicode/cache/`).

**Lo que se fue**: cedar-policy, rusqlite, uuid, notify, reflection/, hooks/, audit/, policy/

**Lo que queda**: rules/ (declare_rule! + inventory), quality/ (SOLID, connascence, LCOM, boundaries, delta), linters/ (clippy, eslint, semgrep).

---

## 1. Principios de Diseño

### 1.1 Performance

Diseño para latencia en **milisegundos** (ms), no microsegundos — no hay evaluations de políticas en tiempo real:

- **Rule Analysis**: 10-100ms por archivo con rayon (paralelo)
- **tree-sitter parse**: 5-50ms por archivo, cacheable en Redb
- **Quality compute**: 1-10ms para debt, ratings, duplications
- **Shared cache**: Redb en disco (~/.cognicode/cache/) compartido entre ambos servidores

### 1.2 Reliability

- **Type-safe rules**: El macro `declare_rule!` genera código con verificación de tipos en compilación
- **Compile-time verification**: El compilador de Rust garantiza que las reglas no violen invariantes del dominio
- **Graceful degradation**: Si el análisis falla, retorna warnings sin bloquear
- **Early termination**: Si se exceden N issues críticos, se detiene y reporta

### 1.3 Simplicity

Sin arquitectura de 3 capas. Sin Cedar. Sin gobernanza. Solo:

```
parse → analyze (parallel) → report
```

---

## 2. Arquitectura de 2 Servidores

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    CogniCode Workspace (mismo proceso)                       │
│                                                                             │
│   ┌───────────────────────────┐     ┌───────────────────────────┐          │
│   │    cognicode-mcp          │     │   cognicode-quality        │          │
│   │    (puerto 8000)          │     │   (puerto 8001)            │          │
│   │                           │     │                            │          │
│   │  32 code intelligence     │     │  Quality analysis          │          │
│   │  tools                    │     │  - rules/ (declare_rule!) │          │
│   │  - analyze_impact         │     │  - quality/ (SOLID, LCOM)  │          │
│   │  - get_call_hierarchy     │     │  - linters/ (clippy, eslint│          │
│   │  - check_architecture     │     │                            │          │
│   │  - semantic_search        │     │  Rules via inventory       │          │
│   │  - etc.                   │     │  auto-registration         │          │
│   └───────────┬───────────────┘     └───────────┬───────────────┘          │
│               │                                 │                           │
│               └────────────┬────────────────────┘                           │
│                            │                                                │
│                            ▼                                                │
│              ┌─────────────────────────────┐                               │
│              │    cognicode-core (lib)      │                               │
│              │                             │                               │
│              │  - tree-sitter parser       │                               │
│              │  - CallGraph builder        │                               │
│              │  - ComplexityCalculator     │                               │
│              │  - ImpactAnalyzer           │                               │
│              │  - CycleDetector            │                               │
│              └─────────────┬───────────────┘                               │
│                            │                                                │
│                            ▼                                                │
│              ┌─────────────────────────────┐                               │
│              │   Redb Shared Cache         │                               │
│              │   ~/.cognicode/cache/       │                               │
│              │                             │                               │
│              │  - call_graphs (table)      │                               │
│              │  - parse_cache (table)      │                               │
│              │  - graph_metadata (table)   │                               │
│              └─────────────────────────────┘                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 cognicode-mcp (Puerto 8000)

32 herramientas de inteligencia de código existentes. Sin cambios.

### 2.2 cognicode-quality (Puerto 8001)

Nuevo servidor dedicado a quality analysis. Expone tools como `check_quality`, `list_rules`, `evaluate_gate`, `detect_duplications`, `check_lint`, etc.

---

## 3. cognicode-axiom: El Rule Engine

El crate `cognicode-axiom` se refactoriza internamente como motor de reglas de calidad. Sigue llamándose `cognicode-axiom` en el workspace pero su enfoque es puramente análisis de calidad.

### 3.1 declare_rule! Macro

```rust
#[macro_export]
macro_rules! declare_rule {
    ($name:ident, $doc:expr, [$($input:ty),+], $analyze:expr) => {
        #[derive(Debug, Clone)]
        pub struct $name;

        impl Rule for $name {
            const DOC: &'static str = $doc;
            type Input = ($($input),+);

            fn analyze(inputs: Self::Input) -> Option<Issue> {
                $analyze(&inputs)
            }
        }

        // Auto-registration via inventory
        inventory::submit! {
            RuleEntry {
                name: stringify!($name),
                doc: $doc,
                create: || Box::new($name) as Box<dyn Rule>
            }
        }
    };
}
```

### 3.2 Rule Trait

```rust
pub trait Rule: Send + Sync {
    const DOC: &'static str;
    type Input;
    fn analyze(inputs: Self::Input) -> Option<Issue>;
}

pub struct RuleEntry {
    pub name: &'static str,
    pub doc: &'static str,
    pub create: fn() -> Box<dyn Rule>,
}
```

### 3.3 Rule Catalog (inventory auto-discovery)

```rust
// Al linkear, inventory recolecta automáticamente todas las reglas
inventory::collect!(RuleEntry);

pub struct RuleCatalog {
    rules: Vec<RuleEntry>,
}

impl RuleCatalog {
    pub fn new() -> Self {
        Self {
            rules: inventory::iter::<RuleEntry>::into_iter().collect(),
        }
    }

    pub fn list(&self) -> Vec<(&'static str, &'static str)> {
        self.rules.iter().map(|r| (r.name, r.doc)).collect()
    }

    pub fn get(&self, name: &str) -> Option<Box<dyn Rule>> {
        self.rules.iter().find(|r| r.name == name).map(|r| (r.create)())
    }
}
```

### 3.4 RuleContext Helpers

```rust
pub struct RuleContext<'a> {
    pub file_path: &'a Path,
    pub source: &'a str,
    pub tree: &'a Tree,
    pub call_graph: Option<&'a CallGraph>,
    pub complexity: Option<&'a ComplexityMetrics>,
}

impl RuleContext<'_> {
    pub fn file_name(&self) -> &str {
        self.file_path.file_name().and_then(|s| s.to_str()).unwrap_or("")
    }

    pub fn lang(&self) -> Language {
        // Detect language from extension
        match self.file_path.extension().and_then(|s| s.to_str()) {
            Some("rs") => Language::Rust,
            Some("ts") | Some("tsx") => Language::TypeScript,
            Some("js") | Some("jsx") => Language::JavaScript,
            Some("py") => Language::Python,
            _ => Language::Unknown,
        }
    }
}
```

### 3.5 Ejemplo de Regla

```rust
// Regla: MaxComplexity
declare_rule!(
    MaxComplexity,
    "La complejidad ciclomática no debe exceder 10",
    [Complexity],
    |ctx: &RuleContext<Complexity>| {
        if ctx.metric.value > 10 {
            Some(Issue::new(
                Severity::Major,
                format!("Complejidad {} excede el límite de 10", ctx.metric.value),
                ctx.file_path.to_path_buf(),
                ctx.tree.root_node().start_position(),
            ))
        } else {
            None
        }
    }
);
```

---

## 4. Calidad Nativa

### 4.1 Code Smells (9 tipos)

Inspirados en SonarQube y el SQL Access Model:

| # | Smell | Descripción | Severidad |
|---|-------|-------------|-----------|
| 1 | CognitiveComplexity | Complejidad cognitiva excesiva | Major |
| 2 | CyclomaticComplexity | branching excesivo | Major |
| 3 | GodClass | Clase con demasiadas responsabilidades | Critical |
| 4 | LongMethod | Método que hace demasiado | Major |
| 5 | ShotgunSurgery | Cambios requieren modificar muchos archivos | Major |
| 6 | DataClump | Mismos datos en múltiples lugares | Minor |
| 7 | PrimitiveObsession | Abuso de tipos primitivos | Minor |
| 8 | ParallelInheritance | Jerarquías paralelas | Minor |
| 9 | FeatureEnvy | Método que usa más datos de otra clase | Minor |

### 4.2 Duplications (BLAKE3)

```rust
pub struct DuplicationDetector {
    block_size: usize,  // default: 20 líneas
    hash_algorithm: blake3::Hasher,
}

impl DuplicationDetector {
    pub fn find_duplications(&self, files: &[&str]) -> Vec<DuplicationBlock> {
        // 1. Fragmentar en bloques de N líneas
        // 2. Calcular BLAKE3 hash por bloque
        // 3. Agrupar por hash igual
        // 4. Retornar bloques con >1 occurrence
    }
}
```

### 4.3 Quality Gates (YAML)

```yaml
# quality-gates/default.yaml
gates:
  - name: "CI Gate"
    conditions:
      - metric: "code_smells"
        operator: "<"
        threshold: 15
      - metric: "complexity"
        operator: "<"
        threshold: 15
      - metric: "duplications"
        operator: "<"
        threshold: 5  # porcentaje
      - metric: "coverage"
        operator: ">="
        threshold: 80

  - name: "Deploy Gate"
    conditions:
      - metric: "security_smells"
        operator: "=="
        threshold: 0
      - metric: "technical_debt"
        operator: "<"
        threshold: 60  # minutos
      - metric: "rating"
        operator: ">="
        threshold: "B"
```

### 4.4 Technical Debt (SQALE)

```rust
pub struct DebtCalculator;

impl DebtCalculator {
    pub fn compute_debt(&self, issues: &[Issue]) -> Duration {
        issues.iter()
            .map(|issue| issue.severity.debt_minutes())
            .sum()
    }
}

pub enum Severity {
    Blocker = 60,   // 1 hora
    Critical = 30,  // 30 min
    Major = 15,     // 15 min
    Minor = 5,      // 5 min
    Info = 1,       // 1 min
}
```

### 4.5 Ratings A-E

Basado en SonarQube maintainability index:

```rust
pub fn compute_rating(maintainability: f64) -> Rating {
    match maintainability {
        80..=100 => Rating::A,
        70..80 => Rating::B,
        50..70 => Rating::C,
        20..50 => Rating::D,
        _ => Rating::E,
    }
}

pub struct QualityReport {
    pub issues: Vec<Issue>,
    pub smell_counts: HashMap<SmellType, usize>,
    pub duplications: DuplicationReport,
    pub debt_minutes: u64,
    pub rating: Rating,
    pub maintainability_index: f64,
}
```

---

## 5. Flujo de Análisis

### 5.1 Diagrama de Flujo

```
Input: paths[], profile?
         │
         ▼
┌─────────────────┐
│ 1. Parse        │ ← tree-sitter, con cache Redb (BLAKE3 key)
│    (parallel)   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 2. Analyze      │ ← rayon: files.par_iter().map(|f| analyze_file(f))
│    (parallel)   │
│                 │
│ Para cada file: │
│  - extract metrics (complexity, LCOM, fan-in/out) │
│  - check rules via inventory                     │
│  - detect duplications                           │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 3. Merge Report │ ← reduce QualityReport
│    (sequential)  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 4. Compute      │
│    - debt       │
│    - rating     │
│    - smells     │
└────────┬────────┘
         │
         ▼
    QualityReport
```

### 5.2 Timing Estimates

| Step | Componente | Latencia | Notas |
|------|------------|----------|-------|
| 1 | tree-sitter parse | 5-50ms | Por archivo, cacheable |
| 2 | Rule analysis | 10-100ms | Paralelo con rayon |
| 3 | Merge reports | <1ms | Sequential |
| 4 | Quality compute | 1-10ms | Debt, ratings |
| **Total por archivo** | | **~20-160ms** | Con cache hit |
| **Total proyecto** | | **N × 20-160ms** | Paralelizado |

---

## 6. Persistencia

### 6.1 Redb Schema

```rust
// ~/.cognicode/cache/cognicode.db

// Call graphs: content-addressed por file path
const CALL_GRAPHS: TableDefinition<&str, &str> = TableDefinition::new("call_graphs");
// key: file_path, value: JSON serialized CallGraph

const PARSE_CACHE: TableDefinition<&str, &str> = TableDefinition::new("parse_cache");
// key: BLAKE3(source), value: JSON serialized ParseResult

const GRAPH_METADATA: TableDefinition<&str, &str> = TableDefinition::new("graph_metadata");
// key: file_path, value: JSON { hash, last_modified, line_count }
```

### 6.2 YAML Files

```
~/.cognicode/
├── cache/
│   └── cognicode.db        # Redb shared cache
├── config/
│   └── cognicode-quality.yaml
├── quality-gates/
│   ├── default.yaml
│   └── deploy.yaml
└── quality-profiles/
    ├── default.yaml
    ├── rust.yaml
    └── typescript.yaml
```

### 6.3 Compiled Rules (inventory)

Las reglas son código compilado. El macro `declare_rule!` usa `inventory::submit!` para auto-registro. No hay archivos de reglas en runtime — todo está en el binario.

---

## 7. MCP Tools (cognicode-quality)

### 7.1 Quality Tools

| Tool | Descripción | Input | Output |
|------|-------------|-------|--------|
| `check_quality` | Analiza calidad de código | `{paths[], profile?}` | `QualityReport` |
| `quality_delta` | Compara calidad entre dos análisis | `{analysis_a, analysis_b}` | `{delta_score, delta_issues}` |
| `check_boundaries` | Verifica límites de métricas | `{path, metric, threshold}` | `{within_bounds, actual}` |
| `detect_duplications` | Detecta código duplicado | `{paths[]}` | `{duplications[]}` |
| `evaluate_gate` | Evalúa un quality gate | `{report, gate_name}` | `{pass, status, details}` |
| `list_rules` | Lista reglas disponibles | `{filter?}` | `{rules[]}` |
| `test_rule` | Prueba una regla con fixtures | `{rule_id, fixtures}` | `{results[]}` |
| `get_profile` | Obtiene perfil de calidad | `{profile_name}` | `{profile}` |
| `check_lint` | Ejecuta linter externo | `{paths[], linter}` | `{issues[]}` |
| `list_smells` | Lista code smells detectados | `{paths[]}` | `{smells[]}` |
| `get_debt` | Obtiene technical debt | `{paths[]}` | `{debt_minutes, breakdown}` |
| `get_rating` | Obtiene rating por archivo | `{path}` | `{rating, maintainability}` |

### 7.2 Linters Soportados

| Linter | Languages | Wrapper |
|--------|-----------|---------|
| clippy | Rust | `linters/clippy.rs` |
| eslint | JS/TS | `linters/eslint.rs` |
| semgrep | Multi | `linters/semgrep.rs` |

---

## 8. Configuración YAML

```yaml
# ~/.cognicode/config/cognicode-quality.yaml

# Análisis
analysis:
  max_complexity: 10
  max_depth: 4
  max_duplications_percent: 5.0
  max_debt_minutes: 60
  max_issues_per_file: 100
  early_termination_threshold: 10  # critical issues

# Linters
linters:
  clippy:
    enabled: true
    args: ["--", "-W", "clippy::all"]
  eslint:
    enabled: true
    config: "./.eslintrc.json"
  semgrep:
    enabled: true
    rules: ["security", "best-practices"]

# Quality Gates
gates:
  default: "./quality-gates/default.yaml"
  deploy: "./quality-gates/deploy.yaml"

# Profiles
profiles:
  default: "./quality-profiles/default.yaml"
  rust: "./quality-profiles/rust.yaml"

# Cache
cache:
  directory: "~/.cognicode/cache"
  max_size_mb: 512

# Performance
performance:
  max_concurrent_analyses: 4
  parse_cache_size: 10000
```

---

## 9. Workspace Structure

```
CogniCode/
├── crates/
│   ├── cognicode-core/           # lib: tree-sitter, CallGraph, ComplexityCalculator, etc.
│   │
│   ├── cognicode-mcp/            # bin: puerto 8000, 32 code intelligence tools
│   │   └── src/
│   │       └── main.rs
│   │
│   ├── cognicode-quality/        # bin: NUEVO — puerto 8001, quality analysis tools
│   │   └── src/
│   │       └── main.rs
│   │
│   └── cognicode-axiom/          # lib: refactorizado — quality rules engine
│       └── src/
│           ├── lib.rs
│           ├── rules/            # declare_rule! + inventory
│           │   ├── engine.rs
│           │   ├── trait.rs
│           │   ├── context.rs
│           │   ├── types.rs
│           │   ├── macros.rs
│           │   └── catalog/
│           │       └── mod.rs
│           │
│           ├── quality/          # SOLID, connascence, LCOM, boundaries, delta
│           │   ├── mod.rs
│           │   ├── solid.rs
│           │   ├── connascence.rs
│           │   ├── lcom.rs
│           │   ├── boundaries.rs
│           │   └── delta.rs
│           │
│           ├── smells/           # 9 code smells
│           │   ├── mod.rs
│           │   └── ...
│           │
│           └── linters/         # clippy, eslint, semgrep wrappers
│               ├── mod.rs
│               ├── clippy.rs
│               ├── eslint.rs
│               └── semgrep.rs
│
├── docs/
│   └── research/
│       └── 10-ARQUITECTURA-INTEGRAL.md
│
└── Cargo.toml
```

### 9.1 Módulos de cognicode-axiom (refactorizado)

| Módulo | Responsabilidad | Dependencias |
|--------|-----------------|---------------|
| `rules/engine` | Orquestación de análisis | tree-sitter, rayon, inventory |
| `rules/trait` | Rule trait y RuleContext | — |
| `rules/catalog` | Auto-discovery via inventory | inventory crate |
| `quality/solid` | Principios SOLID | tree-sitter |
| `quality/connascence` | Métricas de connascence | tree-sitter |
| `quality/lcom` | Lack of Cohesion of Methods | tree-sitter |
| `quality/boundaries` | Verificación de límites | — |
| `quality/delta` | Delta entre análisis | serde |
| `smells/*` | 9 code smells | tree-sitter |
| `linters/*` | Wrappers de linters externos | subprocess |

### 9.2 Dependencias

**Eliminadas**: cedar-policy, rusqlite, uuid, notify

**Mantenidas**: cognicode-core, rayon, tokio, serde, rmcp, chrono, thiserror, dashmap, parking_lot, regex

**Nuevas**: inventory (para declare_rule! auto-registration)

---

## 10. Métricas de Calidad

### 10.1 Maintainability Index

```rust
pub fn maintainability_index(
    complexity: f64,
    duplications: f64,
    lines: u32,
) -> f64 {
    // Fórmula SQALE simplificada
    let a = 100.0;
    let b = 0.25 * duplications;
    let c = 0.25 * (complexity as f64);
    let d = 50.0 * (lines as f64 / 1000.0);

    (a - b - c - d).max(0.0).min(100.0)
}
```

### 10.2 Quality Scores

| Categoría | Peso | Métricas |
|-----------|------|----------|
| Complexity | 30% | Cyclomatic, Cognitive |
| Duplications | 20% | % líneas duplicadas |
| Cohesion | 20% | LCOM, connascence |
| Naming | 15% | Reglas de naming |
| Error Handling | 15% | Missing error handling |

---

*Documento versión 2.0 — pivote de governance a pure quality analysis*
