# Orquestación con LangGraph

## 1. ¿Por qué LangGraph?

| Dimensión | LangGraph | CrewAI |
|-----------|-----------|--------|
| **Loop infinito** | Nativo — ciclo en el grafo | ❌ Hay que luchar contra el framework |
| **State management** | `TypedDict` tipado + checkpointing | Memoria conversacional no estructurada |
| **Control de flujo** | Condicional edges + loops | `crew.kickoff()` asume tarea finita |
| **Error recovery** | `handle_node_error` + retry budgets | Manual, sin reintentos |
| **Observability** | LangSmith tracing nativo | Logging básico |

**Decisión**: LangGraph — la arquitectura de state machine con ciclos es el ajuste natural para el bucle de mejora autónoma.

---

## 2. Arquitectura de Agentes

### 2.1 Los 4 Nodos del Grafo

```
┌──────────────────────────────────────────────────────────────────┐
│                    LANGGRAPH STATE MACHINE                        │
│                                                                   │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐     │
│  │ANALYZER  │──▶│ IMPROVER │──▶│EVALUATOR │──▶│ DECIDER  │     │
│  │          │   │          │   │          │   │          │     │
│  │ Input:   │   │ Input:   │   │ Input:   │   │ Input:   │     │
│  │ metrics  │   │ rule_id  │   │ modified │   │ baseline │     │
│  │ history  │   │ catalog  │   │ catalog  │   │ vs       │     │
│  │          │   │          │   │          │   │ current  │     │
│  │ Output:  │   │ Output:  │   │ Output:  │   │ Output:  │     │
│  │ target   │   │ proposed │   │ current  │   │ KEEP or  │     │
│  │ rule_id  │   │ change   │   │ metrics  │   │ DISCARD  │     │
│  └──────────┘   └──────────┘   └──────────┘   └────┬─────┘     │
│       ▲                                              │           │
│       │         conditional_edge                     │           │
│       │         (if iterations remain)               │           │
│       └──────────────────────────────────────────────┘           │
│                                                                   │
│  State: EvolutionState {                                         │
│    iteration, target_rule_id, proposed_change,                   │
│    baseline_metrics, current_metrics, status,                    │
│    commit_hash, error_message, iteration_log                     │
│  }                                                                │
│                                                                   │
│  Checkpointing: MemorySaver (in-memory) o SQLiteSaver (persist)  │
└──────────────────────────────────────────────────────────────────┘
```

### 2.2 Analyzer Agent

**Rol**: Identificar la peor regla para mejorar en esta iteración.

| Herramienta | Descripción |
|-------------|-------------|
| `read(evolution.tsv)` | Leer log histórico de experimentos |
| `read(metrics.db)` | Consultar SQLite para métricas por regla |
| `parse_sandbox_results()` | Parsear JSONL de resultados de sandbox |

**Lógica de selección**:
1. Cargar todas las métricas del último run de evaluación
2. Filtrar reglas intentadas en las últimas 5 iteraciones (evitar ciclos)
3. Ordenar por F1 ascendente (peores primero)
4. Si F1 < 0.50 → prioridad alta; si FPR > 0.30 → prioridad media
5. Retornar `target_rule_id` + `baseline_metrics`

```python
def analyzer_node(state: EvolutionSchema) -> EvolutionSchema:
    agent = AnalyzerAgent()
    
    # Cargar métricas, filtrar recientes
    worst_rule = agent.identify_worst_rule(
        metrics_history=state["iteration_log"],
        baseline=state["baseline_metrics"]
    )
    
    return {
        **state,
        "target_rule_id": worst_rule["rule_id"],
        "baseline_metrics": {
            **state["baseline_metrics"],
            worst_rule["rule_id"]: worst_rule["metrics"]
        }
    }
```

### 2.3 Improver Agent

**Rol**: Proponer y aplicar una mejora a la regla objetivo.

| Herramienta | Descripción |
|-------------|-------------|
| `read(catalog.rs)` | Leer bloque `declare_rule!` de la regla |
| `edit(catalog.rs)` | Modificar regex, thresholds, lógica |
| `bash(cargo check)` | Verificar que el código compila |
| `bash(git diff)` | Mostrar cambios propuestos |
| `bash(git checkout)` | Revertir cambios si falla |

**Estrategias de mejora** (según el problema):

| Problema detectado | Estrategia |
|-------------------|------------|
| FPR > 0.30 | Tighten regex: añadir negative lookahead, clases más específicas |
| Recall < 0.50 | Extender patrón: añadir alternativas, relajar condiciones |
| Execution time alto | Optimizar: reducir complejidad del patrón, early exit |
| Issue density anómalo | Revisar threshold: ajustar parámetros de la regla |

**Gate de compilación**: Si `cargo check` falla → revertir inmediatamente, reportar error.

```python
def improver_node(state: EvolutionSchema) -> EvolutionSchema:
    agent = ImproverAgent()
    
    # Proponer cambio basado en métricas actuales
    change = agent.analyze_and_propose(
        rule_id=state["target_rule_id"],
        current_metrics=state["baseline_metrics"].get(state["target_rule_id"], {})
    )
    
    # Verificar compilación
    if not agent.verify_compilation():
        agent.revert_changes()
        return {**state, "status": "failed", "error_message": "Compilation failed"}
    
    return {**state, "proposed_change": change}
```

### 2.4 Evaluator Agent

**Rol**: Ejecutar la suite completa de evaluación sobre el código modificado.

| Herramienta | Descripción |
|-------------|-------------|
| `bash(cargo test)` | Ejecutar todos los tests del workspace |
| `bash(sandbox-orchestrator)` | Evaluar regla sobre corpus |
| `bash(sonar-scanner)` | Análisis SonarQube |
| `bash(eslint)` | Análisis ESLint |
| `run_consensus_engine()` | Clasificar hallazgos vía consenso multi-tool |

**Pipeline de evaluación**:
1. `cargo test --workspace` → todos deben pasar
2. `sandbox-orchestrator eval --rule X --corpus all` → métricas por regla
3. Herramientas externas (SonarQube, ESLint, etc.) sobre el corpus
4. Consensus engine → clasificación TP/FP/FN/TN
5. Cálculo de métricas: precision, recall, F1, FPR, execution time

```python
def evaluator_node(state: EvolutionSchema) -> EvolutionSchema:
    agent = EvaluatorAgent()
    
    # Gate 1: Tests
    if not agent.run_tests():
        return {**state, "status": "failed", "error_message": "Tests failed"}
    
    # Gate 2: Sandbox evaluation
    sandbox_metrics = agent.run_sandbox_eval(rule_id=state["target_rule_id"])
    
    # Gate 3: External tools + consensus
    consensus_metrics = agent.run_consensus_eval(rule_id=state["target_rule_id"])
    
    return {**state, "current_metrics": {**sandbox_metrics, **consensus_metrics}}
```

### 2.5 Decider Agent

**Rol**: Decidir si el cambio se mantiene o se descarta.

| Herramienta | Descripción |
|-------------|-------------|
| `compute_health_score()` | Calcular Health Score multi-métrica |
| `bash(git commit)` | Aceptar cambio |
| `bash(git checkout)` | Revertir cambio |
| `write(evolution.tsv)` | Registrar resultado |

**Lógica de decisión**:

```python
def decider_node(state: EvolutionSchema) -> EvolutionSchema:
    agent = DeciderAgent()
    
    baseline = state["baseline_metrics"].get(state["target_rule_id"], {})
    current = state["current_metrics"]
    
    # Calcular Health Score
    health_before = compute_health_score(baseline)
    health_after = compute_health_score(current)
    
    # Decisión
    f1_delta = current.get("f1", 0) - baseline.get("f1", 0)
    fpr_delta = current.get("fpr", 0) - baseline.get("fpr", 0)
    
    if f1_delta > 0.01 and fpr_delta < 0.05:
        decision = "keep"
        agent.commit_change(state["proposed_change"])
    elif baseline.get("f1", 0) == 0 and current.get("f1", 0) > 0:
        decision = "keep"  # Fixed broken rule
        agent.commit_change(f"Fixed broken rule {state['target_rule_id']}")
    else:
        decision = "discard"
        agent.revert_changes()
    
    # Log
    new_log = state["iteration_log"] + [{
        "rule_id": state["target_rule_id"],
        "health_before": health_before,
        "health_after": health_after,
        "decision": decision
    }]
    
    # Check termination
    if state["iteration"] + 1 >= state["max_iterations"]:
        return {**state, "iteration_log": new_log, "status": "ended"}
    
    return {
        **state,
        "iteration": state["iteration"] + 1,
        "iteration_log": new_log,
        "target_rule_id": None,
        "proposed_change": None,
        "current_metrics": {}
    }
```

---

## 3. Definición del Grafo

```python
# autoresearch/orchestrator/graph.py

from langgraph.graph import StateGraph, END
from langgraph.checkpoint.memory import MemorySaver
from typing import TypedDict, Optional

class EvolutionSchema(TypedDict):
    iteration: int
    max_iterations: int
    target_rule_id: Optional[str]
    proposed_change: Optional[str]
    baseline_metrics: dict
    current_metrics: dict
    status: str
    commit_hash: Optional[str]
    error_message: Optional[str]
    iteration_log: list

def build_evolution_graph():
    graph = StateGraph(EvolutionSchema)
    
    # Nodos
    graph.add_node("analyzer", analyzer_node)
    graph.add_node("improver", improver_node)
    graph.add_node("evaluator", evaluator_node)
    graph.add_node("decider", decider_node)
    
    # Edge recovery: si improver o evaluator fallan → saltar a decider
    graph.add_edge("analyzer", "improver")
    graph.add_edge("improver", "evaluator")
    graph.add_edge("evaluator", "decider")
    
    # Conditional: loop o terminar
    def should_continue(state: EvolutionSchema) -> str:
        if state["status"] == "ended":
            return END
        if state["status"] == "failed":
            return "analyzer"  # Skip rule, try next
        return "analyzer"  # Continue loop
    
    graph.add_conditional_edges(
        "decider",
        should_continue,
        {"analyzer": "analyzer", END: END}
    )
    
    graph.set_entry_point("analyzer")
    
    return graph.compile(checkpointer=MemorySaver())
```

---

## 4. Estructura de Archivos

```
autoresearch/
├── program.md                  # Objetivos, reglas, criterios (el humano)
├── config.yaml                 # max_iterations, thresholds, corpus paths
├── requirements.txt            # langgraph, langchain-core, pydantic
│
├── orchestrator/
│   ├── __init__.py
│   ├── graph.py               # LangGraph state machine
│   ├── state.py               # EvolutionSchema TypedDict
│   └── main.py                # Entry point: run_evolution_loop()
│
├── agents/
│   ├── __init__.py
│   ├── analyzer.py            # AnalyzerAgent
│   ├── improver.py            # ImproverAgent (edita Rust)
│   ├── evaluator.py           # EvaluatorAgent (tests + sandbox + externals)
│   └── decider.py             # DeciderAgent (keep/discard)
│
├── tools/
│   ├── __init__.py
│   ├── rust_tools.py          # cargo, git wrappers
│   ├── metric_tools.py        # Parse evolution.tsv, compute stats
│   ├── sandbox_tools.py       # sandbox-orchestrator CLI
│   └── consensus_tools.py     # Multi-tool consensus engine
│
├── corpus/                     # Pinned OSS repos (git submodules)
│   ├── rust/
│   ├── python/
│   ├── js/
│   ├── java/
│   └── go/
│
├── baseline/                   # Baseline run results
├── results/                    # Iteration results (gitignored)
├── evolution.tsv              # Structured experiment log
└── metrics.db                 # SQLite time series
```

---

## 5. `program.md` — El Contrato Humano-Agente

```markdown
# CogniCode Self-Evolving Rules — Program

## Goal
Improve the precision, recall, and signal-to-noise ratio of CogniCode's 
862 code quality rules through autonomous experimentation.

## Rules
1. NEVER modify prepare.py equivalents (corpus, evaluation pipeline, metrics DB)
2. ONLY modify catalog.rs declare_rule! blocks
3. ALWAYS verify compilation before evaluation (cargo check)
4. ALWAYS run full test suite (cargo test --workspace)
5. NEVER install new dependencies
6. NEVER skip the consensus evaluation step

## Decision Criteria
- KEEP if: ΔF1 > 0.01 AND ΔFPR < 0.05
- KEEP if: Rule was broken (F1=0) and now works (F1>0)
- DISCARD otherwise

## Output Format
Log EVERY experiment to evolution.tsv:
iteration | rule_id | f1_before | f1_after | fpr_before | fpr_after | decision | description

## NEVER STOP
Once started, do NOT pause to ask for permission. The human may be 
away. Continue until max_iterations reached or manual interrupt.
```

---

## 6. Python ↔ Rust Boundary

Toda comunicación vía **CLI subprocess** — sin gRPC ni MCP:

```python
# autoresearch/tools/rust_tools.py

class CargoTool:
    def check(self, package="cognicode-axiom", timeout=120) -> Tuple[bool, str]:
        result = subprocess.run(
            ["cargo", "check", "-p", package],
            capture_output=True, text=True, timeout=timeout
        )
        return result.returncode == 0, result.stderr

class GitTool:
    def commit(self, message: str):
        subprocess.run(["git", "add", "-A"], check=True)
        subprocess.run(["git", "commit", "-m", message], check=True)
    
    def revert(self, file_path: str):
        subprocess.run(["git", "checkout", "--", file_path], check=True)

class SandboxTool:
    def eval_rule(self, rule_id: str, corpus: str, timeout=300) -> dict:
        result = subprocess.run(
            ["./target/release/sandbox-orchestrator", "eval",
             "--rule", rule_id, "--corpus", corpus, "--jsonl"],
            capture_output=True, text=True, timeout=timeout
        )
        return json.loads(result.stdout)
```

---

## 7. Manejo de Errores

### 7.1 Compilación Fallida

```
Improver edita catalog.rs → cargo check falla
  → Improver parsea el error de compilación
  → Si es error trivial (typo, missing import) → corrige y reintenta (max 3)
  → Si es error estructural → revierte, reporta "failed", salta regla
```

### 7.2 Tests Fallidos

```
Evaluator ejecuta cargo test → algún test falla
  → NO es aceptable modificar una regla que rompe tests
  → Revierte inmediatamente, reporta "failed"
```

### 7.3 Timeout de Evaluación

```
Evaluator excede 10 minutos en sandbox
  → Considerar como fallo
  → La regla es demasiado lenta para evaluación práctica
  → Reportar "timeout", saltar regla
```

### 7.4 Crash del Sistema

```
LangGraph checkpointing (MemorySaver o SQLiteSaver)
  → Recuperar último checkpoint válido
  → Reanudar desde la iteración donde se quedó
  → evolution.tsv tiene el historial completo hasta el crash
```

---

## 8. Observabilidad

### 8.1 LangSmith Tracing

Cada nodo del grafo genera spans en LangSmith:
- `analyzer.invoke` → regla seleccionada, métricas baseline
- `improver.invoke` → cambio propuesto, resultado compilación
- `evaluator.invoke` → métricas, tiempo de ejecución
- `decider.invoke` → decisión, health score delta

### 8.2 evolution.tsv

```
iteration	rule_id	f1_before	f1_after	fpr_before	fpr_after	decision	description
1	S5332	0.45	0.52	0.32	0.28	keep	tighten regex: added negative lookahead
2	S134	0.78	0.78	0.05	0.07	discard	adjust threshold: slight FP increase
3	S2068	0.89	0.93	0.01	0.01	keep	extend pattern: added base64 variant
```

### 8.3 Dashboard (futuro Phase 3)

- Gráfica de evolución de F1 por regla
- Tasa de keep/discard por tipo de regla
- Top reglas mejoradas
- Health Score trend del ecosistema
