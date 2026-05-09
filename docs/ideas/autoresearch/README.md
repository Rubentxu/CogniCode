# Self-Evolving Rule System (autoresearch)

> Inspirado en [Karpathy/autoresearch](https://github.com/karpathy/autoresearch) — aplicado a reglas de calidad de código.

## Visión

Sistema autónomo que **mejora iterativamente las 862 reglas de CogniCode** mediante un bucle de experimentación: analizar → proponer → evaluar → decidir → repetir. Sin intervención humana durante la operación.

## Principios

| Principio | Origen | Aplicación |
|-----------|--------|------------|
| **Harness de evaluación fijo** | Karpathy `prepare.py` | Corpus de repos OSS pineados por git hash |
| **Métrica inmutable** | Karpathy `evaluate_bpb()` | Health Score multi-métrica (F1, SNR, RES, DAR) |
| **Loop autónomo** | Karpathy "NEVER STOP" | LangGraph state machine con ciclo infinito |
| **Git como memoria** | Karpathy git keep/discard | Commit = mejora aceptada, reset = descartada |
| **Simplicidad como meta** | Karpathy Ockham | ΔF1 > 0.01 requerido para justificar complejidad |
| **Budget fijo** | Karpathy 5-min timeout | 30s por regla en corpus de evaluación |

## Documentos

| # | Documento | Contenido |
|---|-----------|-----------|
| 1 | [Arquitectura del Sistema](architecture.md) | Visión general, componentes, diagramas |
| 2 | [Framework de Métricas](metrics-framework.md) | KPIs multi-nivel, estrategias de ground truth, pipeline de extracción |
| 3 | [Orquestación con LangGraph](orchestration.md) | Agentes Python, herramientas, protocolo del bucle |
| 4 | [Roadmap de Implementación](roadmap.md) | Fases, milestones, criterios de éxito |

## Arquitectura Resumida

```
┌──────────────────────────────────────────────────────────────┐
│              PYTHON ORCHESTRATION (LangGraph)                 │
│                                                               │
│  Analyzer → Improver → Evaluator → Decider → (loop)          │
│                                                               │
│  Estado: EvolutionState { iteration, rule_id, metrics... }   │
│  Checkpointing: MemorySaver (recuperación ante crash)        │
├──────────────────────────────────────────────────────────────┤
│                      CLI (subprocess)                         │
├──────────────────────────────────────────────────────────────┤
│  Rust: cargo check │ cargo test │ sandbox-orchestrator       │
│  External: SonarQube CLI │ ESLint │ Clippy │ Ruff            │
├──────────────────────────────────────────────────────────────┤
│  Corpus: 12+ repos OSS pineados (Rust, Python, JS, Java, Go) │
│  Storage: SQLite (métricas) + JSONL (raw) + TSV (log)       │
└──────────────────────────────────────────────────────────────┘
```

## Stack Tecnológico

| Capa | Tecnología | Justificación |
|------|-----------|---------------|
| Orquestación | **LangGraph** (Python) | State machine nativo, ciclos, checkpointing |
| Motor de reglas | **Rust** (CogniCode) | Performance, seguridad de tipos |
| Evaluación | **SonarQube + ESLint + Clippy + Ruff** | Ground truth multi-tool |
| Corpus | **Git submodules** pineados | Reproducibilidad, versionado |
| Métricas | **SQLite + JSONL + TSV** | Time series + raw replay + human-readable |
| Dashboard | **Leptos** (CogniCode dashboard) | Evolución de reglas en tiempo real |

## Estado

- [x] Propuesta de arquitectura
- [x] Framework de métricas
- [x] Diseño de orquestación
- [x] Roadmap de implementación
- [ ] Phase 1: Prototipo single-agent (10 iteraciones)
- [ ] Phase 2: LangGraph multi-agent
- [ ] Phase 3: Dashboard + human review
- [ ] Phase 4: Multi-rule coordination + production hardening
