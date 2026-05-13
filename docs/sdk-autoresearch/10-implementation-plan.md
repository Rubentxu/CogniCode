# 10 — Implementation Plan

> Plan de implementación en 4 fases, 12 semanas. Del SDK core hasta el enjambre
> multi-agente. Todo en Rust, integrado con el ecosistema CogniCode existente.

---

## Resumen de Fases

| Fase | Semanas | Entregable | Dependencias |
|------|---------|------------|-------------|
| Fase 1: Core | 1-3 | `cognicode-autoresearch-core` crate | cognicode-core |
| Fase 2: Advanced Metrics | 4-6 | Métricas SOLID, Connascence, Smells, LLM | Fase 1 |
| Fase 3: SDLC + Orchestration | 7-9 | Pipelines, SAGA, Meta-agent | Fase 2 |
| Fase 4: Multi-Agent + MCP | 10-12 | Swarm, MCP tools, Skills | Fase 3 |

---

## Fase 1: SDK Core (Semanas 1-3)

### Semana 1 — Traits y Harness

```rust
// File: crates/cognicode-autoresearch-core/src/traits/gate.rs
pub trait QualityGate: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn check(&self, ctx: &ProjectContext) -> Result<GateResult, GateError>;
}

// File: crates/cognicode-autoresearch-core/src/traits/metric.rs
pub trait QualityMetric: Send + Sync {
    fn name(&self) -> &str;
    fn dimension(&self) -> QualityDimension;
    fn source(&self) -> MetricSource;
    fn evaluate(&self, ctx: &ProjectContext) -> Result<MetricValue, MetricError>;
}

// File: crates/cognicode-autoresearch-core/src/harness/mod.rs
pub struct Harness { /* ... */ }
impl Harness {
    pub fn new(config: HarnessConfig) -> Result<Self>;
    pub fn evaluate(&self) -> Result<HealthScore>;
}
```

**Tests**: Unit tests para HealthScore.calculate() con métricas mock.

### Semana 2 — Gates Básicos

- `CompilationGate` (Rust + Python)
- `TestsGate` (Rust + Python)
- `SyntaxGate` (regex/tree-sitter validation)
- `LintGate` (Rust clippy)
- `RuleAdapter` trait + `RustAdapter` implementation

**Tests**: Integration test: ejecutar gates contra el propio proyecto CogniCode.

### Semana 3 — Health Score + Config

- `HealthScore.calculate()` con pesos configurables
- `HarnessConfig` con YAML/JSON deserialization
- `results.tsv` logging
- `cognicode-autoresearch-core` publicado como crate

**Tests**: E2E: configurar harness, evaluar CogniCode, verificar score > 0.

---

## Fase 2: Advanced Metrics (Semanas 4-6)

### Semana 4 — Complexity + SOLID

- M001-M003: Cyclomatic, Cognitive, Maintainability Index
- M004-M008: SOLID proxies (SRP, OCP, LSP, ISP, DIP)
- Usar `CallGraph` y `DependencyRepository` existentes de cognicode-core

### Semana 5 — Connascence + Smells

- M009-M010: Connascence (Name, Algorithm)
- M011-M013: Smells en 3 niveles (Architecture, Design, Implementation)
- Integrar con `check_architecture` MCP tool para detección de ciclos

### Semana 6 — LLM-Assisted Metrics

- Framework para métricas LLM: prompt template, parseo de respuesta, confidence
- M017: CleanCode (LLM review)
- M018: DesignQuality (LLM review)
- Integración con MiniMax Anthropic API (cliente ya existente)

---

## Fase 3: SDLC + Orchestration (Semanas 7-9)

### Semana 7 — SDLC Pipelines

- `CodingPipeline`: bucle Karpathy completo
- `TestingPipeline`: coverage gaps + test generation
- `MaintenancePipeline`: todos los gates + métricas
- `BacktrackEngine`: lógica de retroceso entre fases

### Semana 8 — SAGA (Nivel 2)

- `SagaAnalyzer`: analiza últimas N iteraciones
- `WeightProposal`: genera propuesta de rebalanceo
- Sistema de archivos `proposals/` para revisión humana
- Tests: verificar que SAGA sugiere bajar peso de dimensiones estancadas

### Semana 9 — Meta-Agent (Nivel 3)

- `MetaAnalyzer`: analiza eficiencia del protocolo
- `ProtocolImprovement`: propone cambios concretos a SKILL.md
- Detección de patrones de fallo (30%+ mismo tipo → propuesta)
- Cálculo de coste por punto de mejora

---

## Fase 4: Multi-Agent + MCP (Semanas 10-12)

### Semana 10 — MCP Tools

Extender `cognicode-mcp` con 10 nuevas herramientas (feature flag `autoresearch`):

```rust
// En crates/cognicode-mcp/src/tools/autoresearch.rs
#[tool(name = "autoresearch_evaluate")]
async fn evaluate(project_dir: String) -> EvaluateResult;

#[tool(name = "autoresearch_gates")]
async fn gates(project_dir: String) -> Vec<GateResult>;

#[tool(name = "autoresearch_suggest")]
async fn suggest(project_dir: String, focus: Option<String>) -> Vec<Suggestion>;

#[tool(name = "autoresearch_propose")]
async fn propose(suggestion_id: String) -> ProposedChange;

#[tool(name = "autoresearch_decide")]
async fn decide(/* params */) -> Decision;

#[tool(name = "autoresearch_backlog")]
async fn backlog(action: String, item: Option<BacklogItemInput>) -> Vec<BacklogItem>;

#[tool(name = "autoresearch_saga_rebalance")]
async fn saga_rebalance(project_dir: String, iterations: Option<u32>) -> WeightProposal;

#[tool(name = "autoresearch_meta_analyze")]
async fn meta_analyze(project_dir: String, iterations: Option<u32>) -> ProtocolImprovement;

#[tool(name = "autoresearch_phase_planning")]
async fn phase_planning(project_dir: String) -> PlanningReport;

#[tool(name = "autoresearch_phase_testing")]
async fn phase_testing(project_dir: String) -> TestingReport;
```

### Semana 11 — Skills

Crear el ecosistema de SKILL.md:

```
.claude/skills/autoresearch-sdk/SKILL.md
.claude/skills/autoresearch-sdlc-coding/SKILL.md
.claude/skills/autoresearch-sdlc-testing/SKILL.md
.claude/skills/autoresearch-sdlc-maintain/SKILL.md
.claude/skills/autoresearch-saga/SKILL.md
.claude/skills/autoresearch-meta/SKILL.md
```

### Semana 12 — Swarm + CI/CD

- `SwarmOrchestrator`: multi-agente con merge strategies
- `.github/workflows/autoresearch-nightly.yml`
- `.github/workflows/autoresearch-swarm.yml`
- Documentación final + ejemplos

---

## Dependencias entre Tareas

```
Phase 1: Core
  ├─► Phase 2: Advanced Metrics
  │     ├─► Phase 3: SDLC Pipelines
  │     │     ├─► Phase 4: Multi-Agent + MCP
  │     │     │
  │     │     └─► SAGA Analyzer (can start in parallel with SDLC)
  │     │
  │     └─► LLM Metrics (can start in parallel with Complexity/SOLID)
  │
  └─► Tool Adapters (Rust first, Python/JS in Phase 3)
```

---

## Estimación de Esfuerzo

| Fase | Semanas | Desarrolladores | Esfuerzo (días-hombre) |
|------|---------|-----------------|------------------------|
| Core | 1-3 | 1 | 15 |
| Advanced Metrics | 4-6 | 1 | 15 |
| SDLC + Orchestration | 7-9 | 1-2 | 20 |
| Multi-Agent + MCP | 10-12 | 1-2 | 20 |
| **Total** | **12** | **1-2** | **~70** |

---

## Criterios de Éxito por Fase

### Fase 1
- [ ] `cargo test` pasa en `cognicode-autoresearch-core`
- [ ] `Harness::evaluate()` devuelve Health Score real > 0 para CogniCode
- [ ] Health Score es determinista (mismo commit, mismo resultado)

### Fase 2
- [ ] Las 18 métricas se calculan sin errores
- [ ] Métricas SOLID correlacionan con inspección manual (r > 0.7)
- [ ] Las métricas LLM tienen confidence reportado

### Fase 3
- [ ] CodingPipeline ejecuta 50 iteraciones sin intervención
- [ ] SAGA genera propuesta de pesos razonable tras 50 iteraciones
- [ ] Backtrack funciona: test failure → back a coding

### Fase 4
- [ ] Las 10 herramientas MCP responden correctamente
- [ ] Un agente externo (OpenCode) ejecuta el bucle usando las tools
- [ ] Swarm de 2 agentes completa 50 iteraciones cada uno sin conflictos
- [ ] CI/CD nocturno crea PR automáticamente si hay mejoras

---

## Siguiente: [11 — Patterns Catalog](11-patterns-catalog.md)
