# Arquitectura del Sistema

## 1. Visión General

El sistema **Self-Evolving Rules** aplica el patrón de investigación autónoma de Karpathy al dominio de reglas de calidad de código. En lugar de entrenar modelos LLM, el sistema **mejora reglas estáticas de análisis de código** (regex patterns, thresholds, lógica de detección).

### 1.1 Inspiración: Karpathy/autoresearch

| Concepto Karpathy | Adaptación CogniCode |
|-------------------|---------------------|
| `train.py` (modificable) | `catalog.rs` — reglas `declare_rule!` |
| `prepare.py` (inmutable) | Corpus OSS + pipeline de evaluación |
| `evaluate_bpb()` (métrica) | Health Score (F1 + SNR + RES + DAR) |
| `program.md` (instrucciones) | `autoresearch/program.md` |
| `results.tsv` (log) | `evolution.tsv` + SQLite |
| Git keep/discard | Git commit/reset por regla |
| 5-min timeout | 30s por regla en corpus |

### 1.2 Diferencias Clave

| Aspecto | Karpathy | CogniCode |
|---------|----------|-----------|
| Dominio | Entrenamiento de LLMs | Reglas de calidad de código |
| Lenguaje | Python | Rust (reglas) + Python (orquestación) |
| Métrica | BPB (single) | Multi-métrica (F1, SNR, RES, DAR) |
| Ground truth | Dataset fijo | Consenso multi-tool (SonarQube, ESLint, etc.) |
| Agentes | 1 agente secuencial | 4 agentes especializados (LangGraph) |
| Escala | 1 modelo | 862 reglas |

## 2. Componentes del Sistema

```
┌─────────────────────────────────────────────────────────────────────┐
│                     SELF-EVOLVING RULE SYSTEM                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    ORCHESTRATION LAYER                         │   │
│  │                    (Python + LangGraph)                        │   │
│  │                                                               │   │
│  │  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌─────────┐   │   │
│  │  │ANALYZER  │──▶│ IMPROVER │──▶│EVALUATOR │──▶│ DECIDER │   │   │
│  │  │Identifica│   │Edita Rust│   │Corre     │   │keep/    │   │   │
│  │  │peor regla│   │catalog.rs│   │tests +   │   │discard  │   │   │
│  │  │por F1    │   │+ compila │   │sandbox   │   │+ git    │   │   │
│  │  └──────────┘   └──────────┘   └──────────┘   └────┬────┘   │   │
│  │       ▲                                             │        │   │
│  │       └─────────── LOOP (condicional) ◀─────────────┘        │   │
│  │                                                               │   │
│  │  State: EvolutionState { iteration, rule_id, metrics... }    │   │
│  │  Checkpointing: MemorySaver / SQLite                         │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│                              │ CLI (subprocess)                      │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    RUST ENGINE LAYER                           │   │
│  │                                                               │   │
│  │  cargo check ─── cargo test ─── sandbox-orchestrator eval    │   │
│  │  git commit ──── git reset ──── git diff                      │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    EVALUATION LAYER                            │   │
│  │                                                               │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐     │   │
│  │  │SonarQube │  │ ESLint   │  │ Clippy   │  │ Ruff     │     │   │
│  │  │Scanner   │  │          │  │          │  │          │     │   │
│  │  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘     │   │
│  │       │              │              │              │          │   │
│  │       └──────────────┼──────────────┼──────────────┘          │   │
│  │                      │              │                          │   │
│  │                      ▼              ▼                          │   │
│  │               ┌──────────────────────────┐                    │   │
│  │               │    CONSENSUS ENGINE      │                    │   │
│  │               │  Match → Classify → Score│                    │   │
│  │               └────────────┬─────────────┘                    │   │
│  │                            │                                   │   │
│  │                            ▼                                   │   │
│  │               ┌──────────────────────────┐                    │   │
│  │               │    METRICS DATABASE       │                    │   │
│  │               │  SQLite + JSONL + TSV    │                    │   │
│  │               └──────────────────────────┘                    │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    CORPUS LAYER                                │   │
│  │                                                               │   │
│  │  corpus/                                                      │   │
│  │  ├── rust/  (ripgrep, servo, alacritty, tokio, ...)          │   │
│  │  ├── python/ (flask, django, requests, fastapi, ...)          │   │
│  │  ├── js/    (express, react, next.js, ...)                    │   │
│  │  ├── java/  (junit5, spring-petclinic, ...)                   │   │
│  │  └── go/    (cobra, hugo, prometheus, ...)                    │   │
│  │                                                               │   │
│  │  Todos pineados por git hash — INMUTABLES                     │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## 3. Flujo de Datos

### 3.1 Bucle de Mejora (1 iteración)

```
1. ANALYZER
   Input:  evolution.tsv, baseline_metrics (SQLite)
   Output: target_rule_id, baseline_metrics
   ─────────────────────────────────────────────
   Lee métricas históricas, identifica la peor regla por F1
   que no haya sido intentada en las últimas 5 iteraciones.

2. IMPROVER
   Input:  target_rule_id, catalog.rs
   Output: proposed_change, catalog.rs modificado
   ─────────────────────────────────────────────
   Lee el bloque declare_rule! de la regla objetivo.
   Propone mejora: tighten regex, adjust threshold, refactor logic.
   Edita catalog.rs, ejecuta cargo check.
   Si falla compilación → revierte y reporta error.

3. EVALUATOR
   Input:  catalog.rs modificado, corpus/
   Output: current_metrics (JSON)
   ─────────────────────────────────────────────
   Ejecuta cargo test --workspace (todos deben pasar).
   Ejecuta sandbox-orchestrator eval sobre el corpus.
   Ejecuta herramientas externas (SonarQube, ESLint, etc.).
   Normaliza y clasifica hallazgos vía consenso multi-tool.
   Calcula métricas: precision, recall, F1, FPR, tiempo.

4. DECIDER
   Input:  baseline_metrics, current_metrics
   Output: KEEP (git commit) | DISCARD (git reset)
   ─────────────────────────────────────────────
   Compara métricas baseline vs current.
   Health Score = 0.35×F1 + 0.25×SNR + 0.20×RES + 0.10×DAR - 0.10×cost
   ≥ 0.80 → KEEP, < 0.40 → DISCARD candidate.
   KEEP: git commit con mensaje descriptivo.
   DISCARD: git checkout -- catalog.rs (revierte).
   Log en evolution.tsv independientemente del resultado.
```

### 3.2 Estados del Sistema

```
                    ┌─────────────┐
                    │   INIT      │
                    │ (primera    │
                    │  iteración) │
                    └──────┬──────┘
                           │
                           ▼
                    ┌─────────────┐
              ┌────▶│  ANALYZING  │
              │     └──────┬──────┘
              │            │
              │            ▼
              │     ┌─────────────┐
              │     │ IMPROVING   │
              │     └──────┬──────┘
              │            │
              │      ┌─────┴──────┐
              │      │ compile ok?│
              │      └─────┬──────┘
              │      YES   │   NO
              │      │     │    │
              │      ▼     │    ▼
              │ ┌────────┐ │ ┌──────────┐
              │ │EVALUATE│ │ │ REPAIR/  │
              │ └───┬────┘ │ │ SKIP     │
              │     │      │ └──────────┘
              │     ▼      │
              │ ┌────────┐ │
              │ │ DECIDE │ │
              │ └───┬────┘ │
              │     │      │
              │  ┌──┴───┐  │
              │  │keep? │  │
              │  └──┬───┘  │
              │ KEEP│  DISCARD
              │  │  │   │
              │  ▼  │   ▼
              │ ┌──┐ │ ┌──────┐
              │ │OK│ │ │REVERT│
              │ └──┘ │ └──────┘
              │  │   │   │
              └──┴───┴───┘
                     │
              ┌──────┴──────┐
              │ iterations  │
              │ left?       │
              └──────┬──────┘
                 YES │   NO
                     │    │
                     │    ▼
                     │ ┌──────┐
                     │ │ END  │
                     │ └──────┘
                     │
                     └──▶ (loop)
```

## 4. Componentes Detallados

### 4.1 Orchestration Layer (Python + LangGraph)

**Archivo**: `autoresearch/orchestrator/`

```python
# graph.py — Definición del grafo LangGraph
StateGraph(EvolutionSchema)
  .add_node("analyzer", analyzer_node)
  .add_node("improver", improver_node)  
  .add_node("evaluator", evaluator_node)
  .add_node("decider", decider_node)
  .add_edge("analyzer", "improver")
  .add_edge("improver", "evaluator")
  .add_edge("evaluator", "decider")
  .add_conditional_edges("decider", should_continue, {
      "analyzer": "analyzer",  # loop
      END: END                 # terminar
  })
  .compile(checkpointer=MemorySaver())
```

**Estado**: `EvolutionState` (TypedDict) con `iteration`, `target_rule_id`, `baseline_metrics`, `current_metrics`, `status`, `iteration_log`.

### 4.2 Rust Engine Layer

**Archivos modificables**: `crates/cognicode-axiom/src/rules/catalog.rs`

**Comandos CLI**:
- `cargo check -p cognicode-axiom` — verificar compilación
- `cargo test --workspace` — ejecutar todos los tests
- `sandbox-orchestrator eval --rule X --corpus Y` — evaluar regla
- `git commit` / `git reset` — aceptar/rechazar cambios

### 4.3 Evaluation Layer

**Herramientas externas** (vía CLI):
- SonarQube Scanner CLI (`sonar-scanner`)
- ESLint (`npx eslint --format=json`)
- Clippy (`cargo clippy --message-format=json`)
- Ruff (`ruff check --output-format=json`)
- staticcheck (`staticcheck -f json ./...`)

**Consensus Engine**: Normaliza IDs de reglas, severidades y ubicaciones. Clasifica hallazgos como TP/FP/FN/TN según acuerdo multi-tool.

### 4.4 Corpus Layer

**Repositorios OSS pineados** como git submodules en `autoresearch/corpus/`:

| Lenguaje | Repos | KLOC estimado |
|----------|-------|---------------|
| Rust | ripgrep, alacritty, tokio, rayon | ~200K |
| Python | flask, django, fastapi, requests | ~300K |
| JavaScript | express, react, next.js | ~250K |
| Java | junit5, spring-petclinic, guava | ~200K |
| Go | cobra, hugo, prometheus | ~150K |

**Inmutabilidad**: Cada repo está pineado a un git hash específico. El corpus NO cambia durante una sesión de evaluación.

### 4.5 Storage Layer

| Formato | Ubicación | Propósito |
|---------|-----------|-----------|
| **SQLite** | `autoresearch/metrics.db` | Time series de métricas por regla, consultable |
| **JSONL** | `autoresearch/results/*.jsonl` | Raw output de herramientas (reproducible) |
| **TSV** | `autoresearch/evolution.tsv` | Log legible de experimentos |

**Schema SQLite**:
```sql
analysis_runs (run_id, timestamp, corpus_commit, duration_ms, files_analyzed)
rule_metrics (run_id, rule_id, language, tp, fp, fn, tn, precision, recall, f1, fpr)
file_metrics (run_id, file_path, issues_count, complexity, debt)
project_metrics (run_id, quality_gate, ratings, densities, coverage)
```

## 5. Ciclo de Vida de un Experimento

```
T=0    Analyzer identifica S5332 (F1=0.45, worst in corpus)
T=1    Improver lee S5332 declare_rule! block
T=2    Improver propone: tighten regex, add negative lookahead
T=3    Improver edita catalog.rs, ejecuta cargo check → OK
T=5    Evaluator ejecuta cargo test --workspace → 283/283 pass
T=30   Evaluator ejecuta sandbox-orchestrator sobre corpus
T=35   Evaluator ejecuta SonarQube + ESLint sobre corpus
T=40   Consensus engine clasifica hallazgos
T=41   Métricas: F1=0.52 (era 0.45), FPR=0.28 (era 0.32)
T=42   Decider: ΔF1=+0.07, ΔFPR=-0.04 → KEEP
T=43   git commit -m "autoresearch: S5332 regex tightening (+0.07 F1)"
T=44   Log en evolution.tsv
T=45   Loop → Analyzer (siguiente iteración)
```

## 6. Decisiones de Diseño

### ¿Por qué Python para orquestación?

- LangGraph solo existe en Python
- Las herramientas externas (ESLint, SonarQube) tienen CLIs estándar
- El código de orquestación es "glue code" — no necesita performance de Rust
- Los agentes LLM (futuro) tienen mejor soporte en Python

### ¿Por qué CLI/subprocess en vez de gRPC/MCP?

- Menor overhead de desarrollo
- Debuggable: `stderr` directo
- Compatible con cualquier binario Rust sin modificar
- No requiere servidores adicionales

### ¿Por qué LangGraph y no CrewAI?

- LangGraph tiene ciclos nativos (el loop es un edge condicional)
- State tipado con checkpointing (recuperación ante crash)
- Control de flujo explícito (decisiones condicionales)
- LangSmith tracing para observabilidad
