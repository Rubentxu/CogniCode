# 02 — Architecture

> Arquitectura en 5 capas del SDK. Desde los adaptadores de herramientas por
> lenguaje hasta la capa de orquestación multi-nivel (Karpathy + SAGA + Meta).

---

## 1. Visión General

```
┌─────────────────────────────────────────────────────────────────────┐
│                    COGNICODE AUTORESEARCH SDK                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  LAYER 5: USER INTERFACE                                            │
│  ┌──────────┐  ┌───────────────┐  ┌────────────────────────┐       │
│  │program.md│  │Backlog entries│  │Weight approval UI      │       │
│  └──────────┘  └───────────────┘  └────────────────────────┘       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  LAYER 4: ORCHESTRATION (3 levels)                                  │
│  ┌──────────────────┐ ┌────────────────┐ ┌────────────────────┐    │
│  │ Level 3: META    │ │ Level 2: SAGA  │ │ Level 1: KARPATHY  │    │
│  │ (every 200 iters)│ │ (every 50)     │ │ (every iteration)  │    │
│  └──────────────────┘ └────────────────┘ └────────────────────┘    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ Level 1 Expanded: Multi-Agent Swarm Orchestrator             │  │
│  └──────────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  LAYER 3: SDLC PIPELINES                                            │
│  ┌──────┐ ┌──────┐ ┌───────┐ ┌──────┐ ┌──────┐ ┌────────┐ ┌──────┐│
│  │Plan  │ │Reqs  │ │Design │ │Code  │ │Test  │ │Deploy  │ │Maint ││
│  └──────┘ └──────┘ └───────┘ └──────┘ └──────┘ └────────┘ └──────┘│
│  Backtrack mechanism: fail → go back to earliest fixable phase     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  LAYER 2: EVALUATION HARNESS (IMMUTABLE)                            │
│  ┌────────────┐  ┌──────────────────┐  ┌───────────────────────┐   │
│  │   GATES    │  │     METRICS      │  │    HEALTH SCORE       │   │
│  │            │  │                  │  │                       │   │
│  │ Compile    │  │ Complexity       │  │ W_g × Σ gates +      │   │
│  │ Tests      │  │ SOLID (5)        │  │ W_m × Σ metrics      │   │
│  │ Lint       │  │ Connascence      │  │                       │   │
│  │ Fmt        │  │ Smells (3 lvl)   │  │ Configurable weights  │   │
│  │ Security   │  │ Coverage         │  │ SAGA rebalancing      │   │
│  │ Coverage%  │  │ Security         │  │                       │   │
│  │            │  │ LLM Clean Code   │  │                       │   │
│  │            │  │ LLM Design       │  │                       │   │
│  └────────────┘  └──────────────────┘  └───────────────────────┘   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  LAYER 1: TOOL ADAPTERS (per language)                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│  │   Rust   │ │  Python  │ │   JS/TS  │ │    Go    │ │   Java   │ │
│  │ cargo    │ │ ruff     │ │ eslint   │ │ go vet   │ │checkstyle│ │
│  │ clippy   │ │ pylint   │ │ tsc      │ │staticchk │ │ spotbugs │ │
│  │ audit    │ │ mypy     │ │ jest     │ │golint    │ │ jacoco   │ │
│  │ llvm-cov │ │ bandit   │ │ c8       │ │govulnchk │ │ pmd      │ │
│  │ miri     │ │ coverage │ │ prettier │ │ coverage │ │           │ │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘ │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Layer 1: Tool Adapters

Aíslan las herramientas específicas de cada lenguaje detrás de una interfaz común.

### Trait ToolAdapter

```rust
pub trait ToolAdapter: Send + Sync {
    fn language(&self) -> Language;
    fn check_compilation(&self, ctx: &ProjectContext) -> Result<ToolOutput>;
    fn run_tests(&self, ctx: &ProjectContext) -> Result<ToolOutput>;
    fn run_linter(&self, ctx: &ProjectContext) -> Result<ToolOutput>;
    fn check_formatting(&self, ctx: &ProjectContext) -> Result<ToolOutput>;
    fn audit_security(&self, ctx: &ProjectContext) -> Result<ToolOutput>;
    fn measure_coverage(&self, ctx: &ProjectContext) -> Result<ToolOutput>;
}

pub struct ToolOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub parsed: Option<serde_json::Value>,
    pub exit_code: i32,
    pub duration_ms: u64,
}
```

### Implementaciones por Lenguaje

| Adapter | Compila con | Testea con | Lintea con |
|---------|------------|------------|------------|
| `RustAdapter` | `cargo check` | `cargo test` | `cargo clippy` |
| `PythonAdapter` | `ast.parse()` | `pytest` | `ruff check` |
| `JavaScriptAdapter` | `tsc --noEmit` | `jest --ci` | `eslint` |
| `GoAdapter` | `go build` | `go test ./...` | `golangci-lint` |
| `JavaAdapter` | `javac` | `mvn test` | `checkstyle` |

### Resolución Automática

```rust
pub fn detect_adapter(project_dir: &Path) -> Result<Box<dyn ToolAdapter>> {
    if project_dir.join("Cargo.toml").exists() {
        Ok(Box::new(RustAdapter::new(project_dir)?))
    } else if project_dir.join("pyproject.toml").exists() {
        Ok(Box::new(PythonAdapter::new(project_dir)?))
    } else if project_dir.join("package.json").exists() {
        Ok(Box::new(JavaScriptAdapter::new(project_dir)?))
    } else if project_dir.join("go.mod").exists() {
        Ok(Box::new(GoAdapter::new(project_dir)?))
    } else {
        Err(anyhow!("No supported project type detected"))
    }
}
```

---

## 3. Layer 2: Evaluation Harness

El corazón inmutable del SDK. Implementa el principio de Karpathy: la evaluación
es fija, determinista, y está fuera del alcance del agente.

### Harness

```rust
pub struct Harness {
    config: HarnessConfig,
    adapter: Box<dyn ToolAdapter>,
    gates: Vec<Box<dyn QualityGate>>,
    metrics: Vec<Box<dyn QualityMetric>>,
    weights: HashMap<QualityDimension, f64>,
}

impl Harness {
    pub fn new(config: HarnessConfig) -> Result<Self> {
        let adapter = detect_adapter(&config.project_dir)?;
        let gates = build_gates(&config);
        let metrics = build_metrics(&config);
        let weights = config.default_weights();
        Ok(Harness { config, adapter, gates, metrics, weights })
    }

    pub fn evaluate(&self) -> Result<HealthScore> {
        // 1. Run all gates
        let gate_results: Vec<GateResult> = self.gates.iter()
            .map(|g| g.check(&self.ctx()))
            .collect::<Result<Vec<_>, _>>()?;

        let all_passed = gate_results.iter()
            .filter(|g| g.is_blocking)
            .all(|g| g.passed);

        if !all_passed {
            return Ok(HealthScore::zero_with_gates(gate_results));
        }

        // 2. Run all metrics
        let weighted: Vec<WeightedMetric> = self.metrics.iter()
            .map(|m| {
                let value = m.evaluate(&self.ctx())?;
                let weight = self.weights.get(&m.dimension()).copied().unwrap_or(0.0);
                Ok(WeightedMetric {
                    contribution: value.score * weight,
                    weight,
                    metric: value,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // 3. Calculate Health Score
        let value = weighted.iter().map(|w| w.contribution).sum();

        Ok(HealthScore {
            value,
            gates_passed: true,
            gate_results,
            metric_values: weighted,
            timestamp: Utc::now(),
            commit_hash: self.current_commit(),
        })
    }
}
```

### HarnessConfig

```rust
pub struct HarnessConfig {
    pub project_dir: PathBuf,
    pub language: Option<Language>,
    pub enabled_gates: Vec<String>,
    pub enabled_metrics: Vec<String>,
    pub weights: Option<HashMap<QualityDimension, f64>>,
    pub gate_thresholds: HashMap<String, f64>,
    pub baseline_commit: Option<String>,
    pub results_tsv: PathBuf,
}
```

---

## 4. Layer 3: SDLC Pipelines

Cada fase del SDLC tiene un pipeline específico con sus propios gates, métricas
y pesos. El pipeline de **Maintenance** es el más completo (todos los gates y
métricas activos).

### Implementación de Pipeline

```rust
pub struct CodingPipeline {
    harness: Harness,
}

impl SdlcPipeline for CodingPipeline {
    fn phase(&self) -> SdlcPhase { SdlcPhase::Coding }

    fn gates(&self) -> Vec<Box<dyn QualityGate>> {
        // Fase Coding: gates mínimos para iteración rápida
        vec![
            Box::new(CompilationGate::new()),
            Box::new(TestsGate::new()),
            Box::new(SyntaxGate::new()), // regex/tree-sitter validity
        ]
    }

    fn execute(&self, ctx: &ProjectContext) -> Result<PipelineResult> {
        let score = self.harness.evaluate_with(&self.gates(), &self.metrics())?;
        let delta = if let Some(baseline) = &ctx.baseline_commit {
            let baseline_score = self.harness.evaluate_at(baseline)?;
            Some(score.value - baseline_score.value)
        } else {
            None
        };
        Ok(PipelineResult { health_score: score, delta, phase: self.phase() })
    }
}
```

---

## 5. Layer 4: Orchestration (3 Levels)

### Level 1: Karpathy Inner Loop

El bucle fundamental. El agente ejecuta este protocolo en cada iteración.

```
1. EVALUATE → health_before
2. SUGGEST  → LLM analiza qué componente mejorar
3. PROPOSE  → LLM genera diff concreto
4. MODIFY   → Agente aplica el cambio al código
5. PRE-GATE → Verificación rápida (compila? tests pasan?)
6. COMMIT   → git commit checkpoint
7. EVALUATE → health_after
8. DECIDE   → keep (health mejoró) o discard (git reset)
9. LOG      → results.tsv
10. REPEAT  → vuelta al paso 1
```

### Level 2: SAGA Rebalancing

Cada ~50 iteraciones, analiza la distribución de mejoras y rebalancea pesos.

```rust
pub struct SagaAnalyzer {
    results_tsv: PathBuf,
}

impl SagaAnalyzer {
    pub fn analyze(&self, window: usize) -> Result<WeightProposal> {
        let recent = self.load_recent_iterations(window)?;

        // ¿Qué componente generó más mejoras?
        let gains = self.component_gains(&recent);

        // ¿Qué componente tiene más margen restante?
        let headroom = self.component_headroom(&recent);

        // Proponer nuevo reparto de pesos:
        // Aumentar peso de componentes con alto headroom
        // Reducir peso de componentes estancados (>0.90)
        let proposal = self.rebalance(&gains, &headroom);

        Ok(proposal)
    }
}
```

### Level 3: Meta-Agent

Cada ~200 iteraciones, analiza eficiencia del protocolo completo.

```rust
pub struct MetaAnalyzer {
    results_tsv: PathBuf,
    skill_md: PathBuf,
}

impl MetaAnalyzer {
    pub fn analyze(&self, window: usize) -> Result<ProtocolImprovement> {
        let recent = self.load_recent_iterations(window)?;

        let findings = vec![
            self.analyze_failure_patterns(&recent)?,
            self.analyze_cost_efficiency(&recent)?,
            self.analyze_time_distribution(&recent)?,
            self.analyze_improvement_rate(&recent)?,
        ];

        let proposals = findings.iter()
            .filter_map(|f| f.to_proposal())
            .collect();

        Ok(ProtocolImprovement { findings, proposals })
    }
}
```

---

## 6. Layer 5: User Interface

### program.md

Archivo Markdown que el humano edita para "programar" al agente. Define
objetivos, restricciones, protocolo y criterios de decisión.

### Backlog

```
backlog.md  →  autoresearch_backlog MCP tool  →  Agente prioriza
```

### Weight Approval

Las propuestas de SAGA y Meta se escriben como archivos Markdown en
`proposals/`. El humano las revisa y aprueba (o rechaza) manualmente.

---

## 7. Crate Structure

```
crates/
├── cognicode-autoresearch-core/       ← Traits + HealthScore + Harness
│   ├── src/
│   │   ├── lib.rs
│   │   ├── traits/
│   │   │   ├── gate.rs                ← QualityGate trait
│   │   │   ├── metric.rs              ← QualityMetric trait
│   │   │   ├── pipeline.rs            ← SdlcPipeline trait
│   │   │   └── adapter.rs             ← ToolAdapter trait
│   │   ├── harness/
│   │   │   ├── mod.rs                 ← Harness struct
│   │   │   ├── health_score.rs        ← HealthScore calculation
│   │   │   └── config.rs              ← HarnessConfig
│   │   ├── gates/
│   │   │   ├── compilation.rs
│   │   │   ├── tests.rs
│   │   │   ├── lint.rs
│   │   │   ├── formatting.rs
│   │   │   ├── security.rs
│   │   │   └── syntax.rs
│   │   ├── metrics/
│   │   │   ├── complexity.rs
│   │   │   ├── solid.rs
│   │   │   ├── connascence.rs
│   │   │   ├── smells.rs
│   │   │   ├── coverage.rs
│   │   │   ├── llm_review.rs
│   │   │   └── documentation.rs
│   │   ├── adapters/
│   │   │   ├── rust.rs
│   │   │   ├── python.rs
│   │   │   ├── javascript.rs
│   │   │   ├── go.rs
│   │   │   └── java.rs
│   │   └── sdlc/
│   │       ├── mod.rs
│   │       ├── coding.rs
│   │       ├── testing.rs
│   │       ├── maintenance.rs
│   │       └── backtrack.rs
│   └── Cargo.toml

├── cognicode-autoresearch-orchestrator/ ← SAGA + Meta + Swarm
│   ├── src/
│   │   ├── lib.rs
│   │   ├── saga.rs
│   │   ├── meta.rs
│   │   └── swarm.rs
│   └── Cargo.toml

└── cognicode-autoresearch-mcp/         ← MCP Server integration
    ├── src/
    │   ├── main.rs
    │   ├── tools.rs
    │   ├── resources.rs
    │   └── prompts.rs
    └── Cargo.toml
```

---

## 8. Dependency Flow

```
cognicode-autoresearch-mcp
  └── cognicode-autoresearch-orchestrator
        └── cognicode-autoresearch-core
              └── cognicode-core (existing) ← 30 MCP tools, DDD models
```

---

## Siguiente: [03 — Gates Catalog](03-gates.md)
