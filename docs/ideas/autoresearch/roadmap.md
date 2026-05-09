# Roadmap de Implementación

## Visión General

```
Phase 1 (MVP)          Phase 2 (Autónomo)       Phase 3 (Dashboard)      Phase 4 (Producción)
┌─────────────────┐    ┌─────────────────┐     ┌─────────────────┐      ┌─────────────────┐
│ Single-agent    │    │ LangGraph       │     │ Dashboard       │      │ Multi-rule      │
│ prototype       │───▶│ multi-agent     │────▶│ + human review  │─────▶│ + production    │
│                 │    │                 │     │                 │      │                 │
│ • Corpus setup  │    │ • 4 agentes     │     │ • Leptos pages  │      │ • Rule families │
│ • 1 script      │    │ • Checkpoints   │     │ • Trend charts  │      │ • Batch eval    │
│ • 10 iters      │    │ • evolution.tsv │     │ • Review queue  │      │ • CI/CD         │
│ • Manual        │    │ • 50 iters      │     │ • Alerts        │      │ • Scale-out     │
└─────────────────┘    └─────────────────┘     └─────────────────┘      └─────────────────┘
     2 semanas              3 semanas               2 semanas               ongoing
```

---

## Phase 1: MVP — Prototipo Single-Agent

**Objetivo**: Validar el flujo completo end-to-end con un solo script Python.

**Duración**: 2 semanas

### 1.1 Corpus Setup (día 1-3)

- [ ] Seleccionar 5 repos OSS (1 por lenguaje)
- [ ] Pinear a git hash específico en `autoresearch/corpus/`
- [ ] Crear `corpus_config.yaml` con metadata (LOC, lenguaje, complejidad)
- [ ] Escribir script de filtrado de archivos (excluir tests, generated, vendor)

### 1.2 Tool Integration (día 3-5)

- [ ] Instalar y configurar SonarQube Scanner CLI
- [ ] Configurar ESLint con reglas estándar
- [ ] Integrar Clippy (`cargo clippy --message-format=json`)
- [ ] Integrar Ruff (`ruff check --output-format=json`)
- [ ] Crear wrappers Python para cada herramienta

### 1.3 Consensus Engine (día 5-7)

- [ ] Implementar normalización de rule IDs (cross-tool mapping)
- [ ] Implementar normalización de severidades (P0-P4)
- [ ] Implementar matching por (file, line±1, rule_id)
- [ ] Implementar clasificación TP/FP/FN según acuerdo multi-tool
- [ ] Tests unitarios con fixtures conocidos

### 1.4 Single-Agent Loop (día 7-10)

- [ ] Script `autoresearch/run_once.py`:
  - Lee métricas baseline → identifica peor regla
  - Edita `catalog.rs` → `cargo check` → revierte si falla
  - Ejecuta `cargo test` + `sandbox-orchestrator` + externals
  - Calcula métricas → decide keep/discard
  - Log en `evolution.tsv`
- [ ] 10 iteraciones manuales sobre reglas reales

### 1.5 Validation (día 10-12)

- [ ] Verificar que el sistema NO rompe tests existentes
- [ ] Verificar que los cambios propuestos son sintácticamente correctos
- [ ] Medir tiempo por iteración (target: < 5 minutos)
- [ ] Documentar lecciones aprendidas

### Criterios de Éxito Phase 1

- [x] 10 iteraciones completadas sin intervención manual
- [x] Al menos 2 reglas mejoradas (ΔF1 > 0)
- [x] 0 regresiones (ningún test roto)
- [x] `evolution.tsv` con 10 entradas válidas

---

## Phase 2: LangGraph Multi-Agent

**Objetivo**: Produccionizar el bucle con LangGraph, checkpointing y 50 iteraciones.

**Duración**: 3 semanas

### 2.1 LangGraph Setup (día 1-3)

- [ ] Instalar `langgraph`, `langchain-core`, `pydantic`
- [ ] Definir `EvolutionSchema` (TypedDict)
- [ ] Implementar 4 nodos: Analyzer, Improver, Evaluator, Decider
- [ ] Construir grafo con ciclo condicional

### 2.2 Agent Specialization (día 3-8)

- [ ] **AnalyzerAgent**: Optimizar selección de reglas (evitar repetición, priorizar peores)
- [ ] **ImproverAgent**: Mejorar edición de Rust (parseo de `declare_rule!`, edición segura)
- [ ] **EvaluatorAgent**: Paralelizar herramientas externas
- [ ] **DeciderAgent**: Implementar Health Score multi-métrica

### 2.3 Checkpointing & Recovery (día 8-10)

- [ ] Configurar `MemorySaver` para checkpointing en memoria
- [ ] Implementar recuperación ante crash (reanudar desde último checkpoint)
- [ ] Tests de recovery: matar proceso a mitad, verificar reanudación

### 2.4 Tool Hardening (día 10-12)

- [ ] Añadir retry budgets (Improver: 3 intentos, Evaluator: 2 intentos)
- [ ] Mejorar parseo de errores de compilación Rust
- [ ] Timeout handling para herramientas externas
- [ ] Circuit breaker: si 5 fallos seguidos → pausar y alertar

### 2.5 50-Iteration Run (día 12-15)

- [ ] Ejecutar 50 iteraciones autónomas
- [ ] Monitorear con LangSmith tracing
- [ ] Analizar resultados: keep rate, F1 promedio, reglas mejoradas
- [ ] Ajustar thresholds si es necesario

### Criterios de Éxito Phase 2

- [x] 50 iteraciones completadas autónomamente
- [x] Keep rate > 20% (al menos 10 mejoras aceptadas)
- [x] Recuperación ante crash verificada
- [x] LangSmith traces para todas las iteraciones
- [x] 0 regresiones en tests existentes

---

## Phase 3: Dashboard + Human Review

**Objetivo**: Visualizar evolución de reglas y añadir puntos de revisión humana.

**Duración**: 2 semanas

### 3.1 Database Migration (día 1-2)

- [ ] Migrar de `evolution.tsv` a SQLite como fuente primaria
- [ ] Esquema: `analysis_runs`, `rule_metrics`, `file_metrics`, `project_metrics`
- [ ] TSV como export/backup (no fuente de verdad)

### 3.2 Dashboard Pages (día 2-7)

- [ ] **Rule Evolution**: Gráfica F1/SNR por regla a lo largo del tiempo
- [ ] **Experiment Log**: Tabla filtrable de todas las iteraciones
- [ ] **Health Overview**: Health Score agregado del ecosistema
- [ ] **Top Improvers**: Ranking de reglas más mejoradas
- [ ] **Review Queue**: Reglas que necesitan revisión humana

### 3.3 Human Review Triggers (día 7-10)

- [ ] Trigger: ΔF1 > 0.10 (mejora significativa → revisar que no sea overfitting)
- [ ] Trigger: ΔFPR > 0.20 (aumento de falsos positivos → revisar)
- [ ] Trigger: 10 descartes seguidos (posible bug en el agente)
- [ ] Trigger: Daily digest (resumen diario de actividad)
- [ ] UI para aprobar/rechazar decisiones del agente

### 3.4 Alerts & Notifications (día 10-12)

- [ ] Alertas por email/Slack en triggers críticos
- [ ] Dashboard badge con estado del sistema (RUNNING/PAUSED/ERROR)
- [ ] Log de auditoría para todas las decisiones humanas

### Criterios de Éxito Phase 3

- [x] Dashboard con 5 páginas funcionales
- [x] Human review loop integrado (approve/reject)
- [x] Daily digest funcional
- [x] Alertas en tiempo real para anomalías

---

## Phase 4: Multi-Rule Coordination + Production

**Objetivo**: Escalar a mejora coordinada de múltiples reglas y producción hardening.

**Duración**: Ongoing

### 4.1 Rule Family Awareness (día 1-5)

- [ ] Detectar reglas relacionadas (misma categoría, patrones similares)
- [ ] Si se mejora S5332 (clear-text HTTP) → evaluar también S5331, S5333
- [ ] Evitar mejoras contradictorias entre reglas de la misma familia

### 4.2 Batch Evaluation (día 5-10)

- [ ] Evaluar TODAS las reglas después de cada cambio individual
- [ ] Detectar FPR shifts > 5% en reglas NO modificadas (efectos secundarios)
- [ ] Flaggear interacciones para revisión humana

### 4.3 CI/CD Integration (día 10-15)

- [ ] GitHub Actions workflow: `autoresearch-run.yml`
- [ ] Ejecutar N iteraciones en CI (nightly)
- [ ] PR automático con mejoras aceptadas
- [ ] Gate: solo merge si Health Score agregado no empeora

### 4.4 Scale-Out (día 15-20)

- [ ] Múltiples agentes en paralelo (1 por lenguaje)
- [ ] Orquestador mergea resultados
- [ ] Evitar conflictos de edición en catalog.rs (branch por agente)

### 4.5 Production Hardening (ongoing)

- [ ] Rate limiting para APIs externas (SonarQube, etc.)
- [ ] Cache de resultados de evaluación (evitar re-ejecutar tool externa si corpus no cambió)
- [ ] Métricas de coste ($ por iteración)
- [ ] SLOs: 99% de iteraciones sin fallo, < 5 min/iteración
- [ ] Playbook de incidentes (qué hacer si el agente rompe catalog.rs)

### Criterios de Éxito Phase 4

- [x] Batch evaluation detecta interacciones entre reglas
- [x] CI/CD ejecuta loop automático nightly
- [x] PRs automáticos con mejoras aceptadas
- [x] Multi-agente por lenguaje funcional
- [x] SLOs cumplidos (99% success rate, < 5 min/iter)

---

## Resumen de Entregables

| Phase | Entregable | Archivos clave |
|-------|-----------|----------------|
| **1** | Script single-agent funcional | `autoresearch/run_once.py` |
| **1** | Corpus 5 OSS repos | `autoresearch/corpus/` |
| **1** | Consensus engine | `autoresearch/tools/consensus_tools.py` |
| **2** | LangGraph state machine | `autoresearch/orchestrator/graph.py` |
| **2** | 4 agentes especializados | `autoresearch/agents/` |
| **2** | Checkpointing + recovery | `MemorySaver` / `SQLiteSaver` |
| **3** | Dashboard 5 páginas | `crates/cognicode-dashboard/src/pages/` |
| **3** | Human review UI | `review_queue.rs`, `daily_digest.rs` |
| **4** | Batch evaluation | `autoresearch/tools/batch_eval.py` |
| **4** | CI/CD pipeline | `.github/workflows/autoresearch-run.yml` |

## Dependencias entre Phases

```
Phase 1 ──▶ Phase 2 ──▶ Phase 3 ──▶ Phase 4
  │            │            │
  └─ Corpues   └─ Estado    └─ UI
     Base          Persistente   Humana
```

- **Phase 1 es bloqueante** para Phase 2 (sin corpus ni consensus engine, no hay evaluación)
- **Phase 2 es bloqueante** para Phase 3 (sin datos de iteraciones, no hay dashboard)
- **Phase 3 y 4 pueden solaparse** (dashboard + CI/CD son independientes)

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|-------------|---------|------------|
| Corpus insuficiente para detectar mejoras reales | Media | Alto | Ampliar corpus en Phase 2, añadir anotación manual |
| Agente genera Rust sintácticamente inválido | Alta | Bajo | `cargo check` gate + retry budget |
| Falsos positivos del consensus engine | Media | Alto | Pesos calibrados + human audit periódico |
| Interacciones no detectadas entre reglas | Alta | Medio | Batch evaluation en Phase 4 |
| Coste computacional excesivo | Baja | Medio | Cache de resultados, rate limiting |
| Overfitting al corpus de evaluación | Media | Alto | Cross-validation split, corpus rotation |
