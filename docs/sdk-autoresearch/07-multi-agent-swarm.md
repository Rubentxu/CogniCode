# 07 — Multi-Agent Swarm

> Arquitectura de enjambre competitivo: múltiples agentes trabajando en paralelo
> sobre ramas git separadas, con un orquestador que mergea periódicamente los
> mejores resultados.

---

## 1. Visión General

```
┌──────────────────────────────────────────────────────────────────┐
│                    AGENT SWARM ORCHESTRATOR                       │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │ Agent A  │  │ Agent B  │  │ Agent C  │  │ Agent D  │        │
│  │          │  │          │  │          │  │          │        │
│  │ rules    │  │ python   │  │ perf     │  │ bugs     │        │
│  │ health   │  │ expansion│  │ optimize │  │ fixing   │        │
│  │          │  │          │  │          │  │          │        │
│  │ rama:    │  │ rama:    │  │ rama:   │  │ rama:   │        │
│  │ auto/    │  │ auto/    │  │ auto/   │  │ auto/   │        │
│  │ rules    │  │ py-exp   │  │ perf    │  │ bugs    │        │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘        │
│       │              │              │              │              │
│       └──────────────┴──────────────┴──────────────┘              │
│                              │                                     │
│                     ┌────────▼────────┐                           │
│                     │   ORCHESTRATOR  │                           │
│                     │                 │                           │
│                     │ Cada N iters:   │                           │
│                     │ 1. mergea ramas │                           │
│                     │ 2. evalúa health│                           │
│                     │ 3. selecciona   │                           │
│                     │    best commits │                           │
│                     │ 4. rebasea resto│                           │
│                     └────────┬────────┘                           │
│                              │                                     │
│                     ┌────────▼────────┐                           │
│                     │   MAIN BRANCH   │                           │
│                     │  (solo mejoras  │                           │
│                     │   confirmadas)  │                           │
│                     └─────────────────┘                           │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

---

## 2. Tipos de Agentes

### Agent A — Rules Health Improver

```yaml
name: rules-health
focus:
  - code smells reduction
  - SOLID compliance
  - complexity reduction
  - connascence removal
weights:
  smells: 0.35
  solid: 0.30
  connascence: 0.20
  complexity: 0.15
```

### Agent B — Multi-Language Expansion

```yaml
name: python-expansion
focus:
  - port Rust rules to Python
  - add Python-specific rules
  - increase language coverage
metrics:
  - python_coverage
  - cross_language_parity
```

### Agent C — Performance Optimizer

```yaml
name: performance
focus:
  - build time reduction
  - binary size reduction
  - runtime performance
metrics:
  - build_time
  - binary_size
  - benchmark_results
```

### Agent D — Bug Fixer

```yaml
name: bug-fixer
focus:
  - failing tests
  - known bugs
  - crash reports (Chronos)
  - production error logs
metrics:
  - test_pass_rate
  - bug_count
  - crash_free_sessions
```

---

## 3. Estrategias de Merge

### 3.1 Winner-Takes-All

```
Solo el mejor commit de cada ronda sobrevive.
Simple, evita conflictos, pero desperdicia trabajo paralelo.
```

### 3.2 Merge-All-Passing

```
Todos los commits que mejoran health se mergean en orden.
Si hay conflictos, se intenta merge; si falla, se descarta el que
menos mejoró.
```

### 3.3 Tournament

```
┌──────────────────────────────────────────┐
│  Ronda 1: 8 agentes                      │
│  ┌──┐ ┌──┐ ┌──┐ ┌──┐ ┌──┐ ┌──┐ ┌──┐┌──┐│
│  │A │ │B │ │C │ │D │ │E │ │F │ │G ││H ││
│  └┬─┘ └┬─┘ └┬─┘ └┬─┘ └┬─┘ └┬─┘ └┬─┘└┬─┘│
│   │    │    │    │    │    │    │   │   │
│   └────┴────┘    └────┴────┘    └───┴───┘
│       │              │              │
│  Ronda 2: 4 agentes                   │
│  ┌──┐ ┌──┐ ┌──┐ ┌──┐                 │
│  │A'│ │B'│ │C'│ │D'│                 │
│  └┬─┘ └┬─┘ └┬─┘ └┬─┘                 │
│   │    │    │    │                     │
│   └────┘    └────┘                     │
│     │          │                        │
│  Final: 2 agentes → mejor mergea      │
└──────────────────────────────────────────┘
```

---

## 4. Swarm Orchestrator

```rust
pub struct SwarmOrchestrator {
    agents: Vec<AgentConfig>,
    harness: Arc<Harness>,
    merge_strategy: MergeStrategy,
    sync_interval: usize, // iteraciones entre merges
}

pub struct AgentConfig {
    pub name: String,
    pub branch: String,
    pub focus_dimensions: Vec<QualityDimension>,
    pub custom_weights: HashMap<QualityDimension, f64>,
    pub custom_metrics: Vec<Box<dyn QualityMetric>>,
    pub max_iterations_per_session: usize,
}

impl SwarmOrchestrator {
    pub async fn run_session(
        &self,
        iterations_per_agent: usize,
    ) -> Result<SwarmSessionResult> {
        // 1. Crear ramas para cada agente
        let handles: Vec<_> = self.agents.iter().map(|agent| {
            let harness = self.harness.clone();
            tokio::spawn(async move {
                run_agent_loop(agent, harness, iterations_per_agent).await
            })
        }).collect();

        // 2. Esperar a que todos terminen
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await??);
        }

        // 3. Aplicar estrategia de merge
        let merged = self.merge(&results)?;

        // 4. Evaluar resultado final
        let final_score = self.harness.evaluate()?;

        Ok(SwarmSessionResult {
            agents: results,
            merge: merged,
            final_health_score: final_score,
        })
    }

    fn merge(&self, results: &[AgentResult]) -> Result<MergeResult> {
        match self.merge_strategy {
            MergeStrategy::WinnerTakesAll => self.merge_winner(results),
            MergeStrategy::MergeAllPassing => self.merge_all_passing(results),
            MergeStrategy::Tournament => self.merge_tournament(results),
        }
    }
}
```

---

## 5. Configuración del Enjambre

### program.md (sección multi-agent)

```yaml
multi_agent:
  enabled: true
  agents:
    - name: rules-health
      branch: auto/rules-health
      focus: [smells, solid, connascence, complexity]
      max_iterations: 50

    - name: python-expansion
      branch: auto/python-exp
      focus: [python_coverage, cross_language_parity]
      max_iterations: 30

    - name: performance
      branch: auto/perf
      focus: [build_time, binary_size]
      max_iterations: 20

    - name: bug-fixer
      branch: auto/bug-fix
      focus: [test_pass_rate, bug_count]
      max_iterations: 100

  merge_strategy: tournament
  sync_interval: 25
  max_concurrent_agents: 4
```

### CI/CD Nocturno con Enjambre

```yaml
# .github/workflows/autoresearch-swarm.yml
name: AutoResearch Swarm Nightly
on:
  schedule: [cron: '0 2 * * *']

jobs:
  swarm:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        agent: [rules-health, python-expansion, performance, bug-fixer]
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --bin cognicode-mcp
      - run: cargo run --bin cognicode-mcp &
      - name: Run Agent ${{ matrix.agent }}
        run: |
          opencode run \
            --skill autoresearch-${{ matrix.agent }} \
            --mcp http://localhost:3000 \
            --max-iterations 50 \
            --branch auto/${{ matrix.agent }} \
            "Run autonomous improvement loop for ${{ matrix.agent }}. NEVER STOP."

  merge:
    needs: swarm
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Orchestrate merge
        run: |
          opencode run \
            --skill autoresearch-orchestrator \
            "Merge all agent branches using tournament strategy.
             Evaluate final Health Score and create PR."
```

---

## 6. Evitar Conflictos

### Estrategia de Aislamiento

```
Cada agente trabaja en su propia rama.
Áreas de código con solapamiento → coordinación explícita.

┌─────────────────────────────────────────────────┐
│ ÁREAS DE CÓDIGO (ejemplo CogniCode)              │
├─────────────────────────────────────────────────┤
│ crates/cognicode-axiom/     ← Agent A: rules    │
│ crates/cognicode-core/      ← Agent B: core     │
│ crates/cognicode-sandbox/   ← Agent C: perf     │
│ crates/cognicode-mcp/       ← Agent D: handlers │
└─────────────────────────────────────────────────┘
```

### Detección de Conflictos

```rust
pub struct ConflictDetector {
    dependency_graph: CallGraph,
    agent_assignments: HashMap<String, Vec<PathBuf>>,
}

impl ConflictDetector {
    pub fn check_conflicts(
        &self,
        agent_a: &str,
        agent_b: &str,
    ) -> Vec<ConflictWarning> {
        let files_a = self.agent_assignments.get(agent_a).unwrap();
        let files_b = self.agent_assignments.get(agent_b).unwrap();

        // Solapamiento directo
        let direct: Vec<_> = files_a.iter()
            .filter(|f| files_b.contains(f))
            .collect();

        // Solapamiento por dependencia
        let dependency: Vec<_> = self.dependency_graph
            .find_dependencies_between(files_a, files_b);

        direct.into_iter().chain(dependency).collect()
    }
}
```

---

## 7. Costes Estimados

Basado en los datos de Deep Researcher Agent:

| Configuración | Agentes | Iteraciones/noche | Coste API/día | Coste/mes |
|--------------|---------|-------------------|---------------|-----------|
| Single agent | 1 | ~100 | ~$0.08 | ~$2.40 |
| Light swarm | 2 | ~200 | ~$0.16 | ~$4.80 |
| Standard swarm | 4 | ~400 | ~$0.32 | ~$9.60 |
| Full swarm | 8 | ~800 | ~$0.64 | ~$19.20 |

Coste por mejora (Health Score +0.01): ~$0.50 en promedio.

---

## Siguiente: [08 — Backlog Integration](08-backlog-integration.md)
