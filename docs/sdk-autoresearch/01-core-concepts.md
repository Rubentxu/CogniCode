# 01 — Core Concepts

> Las cuatro abstracciones fundamentales del SDK: Gates, Métricas, Health Score
> y Pipeline. Todo lo demás son implementaciones concretas de estos cuatro traits.

---

## 1. QualityGate — Condición Binaria

Un Gate es una verificación que **debe cumplirse** para que el sistema considere
válido el estado actual del código. Es binario: pasa o no pasa. Si cualquier
Gate falla, el Health Score es 0 y el Pipeline se detiene.

### Trait

```rust
pub trait QualityGate: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn phase(&self) -> SdlcPhase;
    fn check(&self, ctx: &ProjectContext) -> Result<GateResult, GateError>;
    fn is_blocking(&self) -> bool { true }
}

pub struct GateResult {
    pub name: String,
    pub passed: bool,
    pub detail: Option<String>,
    pub duration_ms: u64,
    pub message: Option<String>,
}
```

### Propiedades

- **Determinista**: Mismo `ProjectContext` → mismo `GateResult`. Siempre.
- **Rápido**: Los gates de pre-validación deben ejecutarse en <30s.
- **Bloqueante**: Si `is_blocking() == true`, su fallo detiene la evaluación.
  Los gates no bloqueantes son informativos (warnings).
- **Independiente**: Cada gate no depende del resultado de otros gates.

### Ejemplos Concretos

| Gate | Qué verifica | Rust | Python |
|------|-------------|------|--------|
| CompilationGate | El código compila | `cargo check` | `ast.parse()` |
| TestsGate | Tests pasan | `cargo test` | `pytest` |
| LintGate | Sin warnings | `cargo clippy` | `ruff check` |
| FmtGate | Formato consistente | `cargo fmt --check` | `ruff format --check` |
| SecurityGate | Sin CVEs | `cargo audit` | `bandit` |

---

## 2. QualityMetric — Valor Numérico Normalizado

Una Métrica produce un valor en el rango [0.0, 1.0] que mide algún aspecto
de la calidad del código. A diferencia de los Gates, las métricas siempre
se calculan (si los gates pasaron) y admiten valores intermedios.

### Trait

```rust
pub trait QualityMetric: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn dimension(&self) -> QualityDimension;
    fn source(&self) -> MetricSource;
    fn evaluate(&self, ctx: &ProjectContext) -> Result<MetricValue, MetricError>;
    fn normalize(&self, raw: f64) -> f64;
}
```

### Tipos de Origen

```rust
pub enum MetricSource {
    Deterministic,  // Herramienta que produce siempre el mismo resultado
    LlmAssisted,    // LLM aporta criterio (peso bajo, verificable)
    Hybrid,         // Combinación de herramienta + LLM
}
```

### Dimensiones de Calidad

```rust
pub enum QualityDimension {
    Complexity,      // Complejidad ciclomática, cognitiva, MI
    Solid,           // 5 principios SOLID
    Connascence,     // Acoplamiento por nombre, tipo, algoritmo
    Smells,          // Code smells (arquitectura, diseño, implementación)
    Coverage,        // Cobertura de tests
    Documentation,   // Documentación de API pública
    Security,        // Vulnerabilidades
    Performance,     // Tiempo de build, tamaño de binario
    CleanCode,       // Nombres, estructura, estilo (LLM-assisted)
    DesignQuality,   // Calidad del diseño (LLM-assisted)
}
```

### Propiedades

- **Normalizado**: Siempre en [0.0, 1.0]. 1.0 = calidad máxima en esa dimensión.
- **Trazable**: Cada valor se puede reproducir ejecutando la herramienta subyacente.
- **Ponderable**: El Health Score asigna un peso a cada métrica. Σ pesos = 1.0.
- **Revisable**: Si `source == LlmAssisted`, el valor incluye `confidence` y
  puede marcarse para revisión humana si diverge >20% de métricas deterministas.

---

## 3. HealthScore — La Métrica de Verdad Absoluta

El Health Score es UN número [0.0, 1.0] que resume la calidad del proyecto.
Es el equivalente a `evaluate_bpb()` de Karpathy: **inmutable para el agente,
determinista en su cálculo.**

### Definición

```
Health Score = GATES_FLAG × Σ( W_i × metric_i )

Donde:
  GATES_FLAG = 0 si cualquier gate bloqueante falla
               1 si todos los gates bloqueantes pasan
  W_i        = peso de la métrica i (configurable, ΣW = 1.0)
  metric_i   = valor normalizado en [0.0, 1.0]
```

### Estructura de Datos

```rust
pub struct HealthScore {
    pub value: f64,                          // [0.0, 1.0]
    pub gates_passed: bool,                  // ¿todos los gates OK?
    pub gate_results: Vec<GateResult>,       // detalle de cada gate
    pub metric_values: Vec<WeightedMetric>,  // métricas con pesos
    pub breakdown: HashMap<QualityDimension, f64>, // score por dimensión
    pub timestamp: DateTime<Utc>,
    pub commit_hash: Option<String>,
}

pub struct WeightedMetric {
    pub metric: MetricValue,
    pub weight: f64,
    pub contribution: f64,  // weight × metric.score
}
```

### Principio Karpathy

> El Health Score es SAGRADO. El agente de Nivel 1 NUNCA puede modificar cómo
> se calcula. Solo SAGA (Nivel 2) puede proponer cambios de pesos, y solo con
> aprobación humana.

Si el agente pudiera modificar la evaluación, podría hacer trampa — exactamente
igual que si el agente de Karpathy pudiera editar `evaluate_bpb()`.

---

## 4. SdlcPipeline — Orquestador de Fase

Un Pipeline orquesta Gates → Métricas → Health Score para una fase específica
del SDLC. Define qué gates y métricas aplican en cada fase.

### Trait

```rust
pub trait SdlcPipeline: Send + Sync {
    fn phase(&self) -> SdlcPhase;

    fn gates(&self) -> Vec<Box<dyn QualityGate>>;
    fn metrics(&self) -> Vec<Box<dyn QualityMetric>>;
    fn weights(&self) -> HashMap<QualityDimension, f64>;

    fn execute(&self, ctx: &ProjectContext) -> Result<PipelineResult, PipelineError>;

    fn backtrack(
        &self,
        failure: &PipelineResult,
    ) -> Option<(SdlcPhase, ChangeSuggestion)>;
}
```

### Ciclo de Vida del Pipeline

```
1. GATES: Ejecutar todos los gates. Si alguno falla → HealthScore = 0.
2. MÉTRICAS: Si gates pasan, ejecutar todas las métricas.
3. HEALTH SCORE: Calcular Σ(W_i × metric_i).
4. COMPARAR: Si hay baseline → delta = current - baseline.
5. DECIDIR: El agente (no el pipeline) decide keep/discard.
```

### Backtrack

Si un pipeline falla, el método `backtrack()` sugiere a qué fase anterior
retroceder y qué cambiar. Ejemplo:

```
Test failure en Deploy → backtrack a Coding ("los tests no cubren el nuevo código")
Design flaw detectado en Test → backtrack a Design ("hay un ciclo de dependencias")
```

---

## 5. ProjectContext — Estado del Proyecto

Toda la información que el harness necesita para evaluar.

```rust
pub struct ProjectContext {
    pub project_dir: PathBuf,
    pub language: Language,
    pub toolchain: ToolchainConfig,
    pub manifests: Vec<Manifest>,
    pub baseline_commit: Option<String>,
    pub phase: Option<SdlcPhase>,
}

pub struct ToolchainConfig {
    pub rust_version: Option<String>,
    pub python_version: Option<String>,
    pub node_version: Option<String>,
}
```

---

## 6. Relaciones entre Abstracciones

```
ProjectContext ──▶ QualityGate.check() ──▶ GateResult
                                        │
                                        ▼ (si todos OK)
ProjectContext ──▶ QualityMetric.evaluate() ──▶ MetricValue
                                                  │
                                                  ▼
                              HealthScore.calculate(gates, metrics, weights)
                                                  │
                                                  ▼
                              SdlcPipeline.execute(ctx) ──▶ PipelineResult
```

---

## 7. Ejemplo Mínimo de Uso

```rust
use cognicode_autoresearch_sdk::prelude::*;

fn main() -> anyhow::Result<()> {
    let ctx = ProjectContext::new(".")?;

    // Configurar pipeline de Mantenimiento
    let pipeline = MaintenancePipeline::new(
        vec![
            Box::new(CompilationGate::new()),
            Box::new(TestsGate::new()),
            Box::new(LintGate::new()),
        ],
        vec![
            Box::new(ComplexityMetric::new()),
            Box::new(SolidMetric::new()),
            Box::new(CoverageMetric::new()),
        ],
        weights!{
            Complexity => 0.30,
            Solid => 0.40,
            Coverage => 0.30,
        },
    );

    let result = pipeline.execute(&ctx)?;
    println!("Health Score: {:.3}", result.health_score.value);
    println!("Gates: {:?}", result.health_score.gates_passed);
    for wm in &result.health_score.metric_values {
        println!("  {} = {:.2} (weight {:.2})",
            wm.metric.name, wm.metric.score, wm.weight);
    }

    Ok(())
}
```

---

## Siguiente: [02 — Architecture](02-architecture.md)
