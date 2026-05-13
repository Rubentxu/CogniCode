# 12 — Agent Integration (Anexo)

> Cómo acoplar el SDK con cualquier agente de codificación (Claude Code,
> OpenCode, Codex, Cursor) a través de tres interfaces estándar: MCP Tools,
> Skills (SKILL.md) y Workflows (CI/CD).

---

## 1. Principio Arquitectónico

```
EL BUCLE VIVE EN EL AGENTE, NO EN EL SDK

SDK    = CUERPO  → herramientas, evaluación, métricas (determinista, inmutable)
AGENTE = CEREBRO → decisión, creatividad, exploración (LLM, flexible)
```

El SDK nunca ejecuta el bucle por sí mismo. Expone herramientas que el agente
invoca en el orden correcto. Esto permite que cualquier agente use exactamente
el mismo harness de evaluación.

---

## 2. Capa 1: MCP Tools (10 herramientas)

Extensión del servidor MCP existente de CogniCode. Feature flag `autoresearch`.

### 2.1 `autoresearch_evaluate` — Evaluación Completa

```
Input:  project_dir, phase (opcional), baseline_commit (opcional)
Output: HealthScore, gates[], metrics[], breakdown, delta
```

Equivalente a `evaluate_bpb()` de Karpathy. Determinista, inmutable para el
agente. Evalúa gates + métricas y devuelve el Health Score con desglose.

### 2.2 `autoresearch_gates` — Pre-validación Rápida

```
Input:  project_dir
Output: GateResult[] (con passed: bool y detalle)
```

Solo gates (compilación, tests, lint). ~30 segundos. Para pre-validar antes
de commit y no perder tiempo en experimentos rotos.

### 2.3 `autoresearch_suggest` — ¿Qué Mejorar?

```
Input:  project_dir, focus (opcional), max_suggestions (opcional)
Output: Suggestion[] (componente, ganancia potencial, riesgo, confianza, esfuerzo)
```

Analiza el Health Score y sugiere cambios usando LLM. Filtra intentos previos
fallidos consultando git log.

### 2.4 `autoresearch_propose` — Generar Cambio

```
Input:  suggestion_id
Output: ProposedChange (diff, archivos afectados, impacto estimado, riesgo)
```

Convierte una sugerencia en un diff concreto. NO modifica archivos.

### 2.5 `autoresearch_decide` — Keep o Discard

```
Input:  health_before, health_after, change_description, lines_added, lines_removed
Output: Decision { Keep(reason) | Discard(reason) }
```

Aplica el criterio de simplicidad de Karpathy: mejora <0.001 que añade
complejidad → DISCARD. Simplificación sin pérdida → KEEP.

### 2.6 `autoresearch_backlog` — Gestión de Tareas

```
Input:  action (List|Add|Prioritize|Complete|Fail), item (opcional)
Output: BacklogItem[]
```

CRUD del backlog de mejoras. El usuario añade ideas; el agente las prioriza
y ejecuta.

### 2.7 `autoresearch_saga_rebalance` — Nivel 2

```
Input:  project_dir, iterations (default 50)
Output: WeightProposal (pesos actuales, propuestos, justificación, métricas)
```

Analiza distribución de mejoras y propone rebalanceo de pesos del Health Score.
NO aplica cambios sin aprobación humana.

### 2.8 `autoresearch_meta_analyze` — Nivel 3

```
Input:  project_dir, iterations (default 200)
Output: ProtocolImprovement (hallazgos, propuestas, snippets sugeridos)
```

Analiza eficiencia del protocolo. Detecta patrones de fallo. Propone mejoras
al SKILL.md. NO aplica sin aprobación humana.

### 2.9 `autoresearch_phase_planning` — Planificación SDLC

```
Input:  project_dir
Output: PlanningReport (backlog inicial priorizado, deuda técnica, hotspots)
```

### 2.10 `autoresearch_phase_testing` — Pruebas SDLC

```
Input:  project_dir
Output: TestingReport (gaps de cobertura, sugerencias de tests, mutation score)
```

---

## 3. Capa 2: Skills (SKILL.md)

El SKILL.md es el `program.md` de Karpathy en formato estándar de skills.

### 3.1 Estructura

```yaml
---
name: autoresearch-sdk
description: Autonomous software improvement loop (Karpathy protocol)
triggers:
  - "improve the project"
  - "autonomously fix bugs"
  - "optimize code quality"
  - "run the improvement loop"
allowed-tools:
  - Bash(git:*)
  - Bash(cargo:*)
  - Read
  - Write
  - Edit
  - mcp__cognicode__autoresearch_evaluate
  - mcp__cognicode__autoresearch_gates
  - mcp__cognicode__autoresearch_suggest
  - mcp__cognicode__autoresearch_propose
  - mcp__cognicode__autoresearch_decide
  - mcp__cognicode__autoresearch_backlog
  - mcp__cognicode__autoresearch_saga_rebalance
  - mcp__cognicode__autoresearch_meta_analyze
  - mcp__cognicode__build_graph
  - mcp__cognicode__analyze_impact
  - mcp__cognicode__safe_refactor
  - mcp__cognicode__get_complexity
---

# CogniCode AutoResearch SDK — Agent Protocol

## Objective
You are an autonomous software improvement agent.

## NEVER STOP — NEVER ASK PERMISSION
DO NOT pause to ask the human if you should continue.
If stuck, think harder.

## The Fixed Evaluation Harness (DO NOT MODIFY)
The evaluation harness (autoresearch_evaluate) is SACRED.

## Protocol (10 Steps)
[Step 1: Evaluate → Step 2: Suggest → ... → Step 10: Repeat]

## Decision Rules
[Tabla Karpathy de keep/discard]

## Anti-Patterns
- Do NOT change the same threshold back and forth
- Do NOT modify the evaluation harness
- Do NOT add new dependencies
- Do NOT skip pre-validation
```

### 3.2 Ecosistema de Skills

```
skills/
├── autoresearch-sdk/SKILL.md           ← Bucle principal
├── autoresearch-sdk/references/        ← Documentación
├── autoresearch-sdlc-planning/SKILL.md
├── autoresearch-sdlc-coding/SKILL.md
├── autoresearch-sdlc-testing/SKILL.md
├── autoresearch-sdlc-maintain/SKILL.md
├── autoresearch-saga/SKILL.md          ← Nivel 2
└── autoresearch-meta/SKILL.md          ← Nivel 3
```

---

## 4. Capa 3: Workflows (CI/CD)

### 4.1 Nocturno

```yaml
# .github/workflows/autoresearch-nightly.yml
name: AutoResearch Nightly
on:
  schedule: [cron: '0 2 * * *']

jobs:
  improve:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release --bin cognicode-mcp
      - run: cargo run --release --bin cognicode-mcp &
      - name: Run AutoResearch
        run: |
          opencode run \
            --skill autoresearch-sdk \
            --mcp http://localhost:3000 \
            --max-iterations 50 \
            --timeout 8h \
            "Start the autonomous improvement loop. NEVER STOP."
      - name: Create PR
        if: success()
        run: |
          gh pr create --base main --head auto/nightly \
            --title "AutoResearch: Nightly improvements ($(date +%Y-%m-%d))"
```

### 4.2 Enjambre

```yaml
name: AutoResearch Swarm
jobs:
  swarm:
    strategy:
      matrix:
        agent: [rules-health, python-expansion, performance, bug-fixer]
    steps:
      - run: opencode run --skill autoresearch-${{ matrix.agent }} \
               --branch auto/${{ matrix.agent }} \
               --max-iterations 50
  merge:
    needs: swarm
    steps:
      - run: opencode run --skill autoresearch-orchestrator \
               "Merge all agent branches. Evaluate. Create PR."
```

---

## 5. Configuración por Agente

| Agente | MCP Config | Skill Location |
|--------|-----------|----------------|
| Claude Code | `claude mcp add cognicode -- cognicode-mcp` | `.claude/skills/autoresearch-sdk/` |
| OpenCode | `opencode mcp add cognicode` | `~/.config/opencode/skills/` |
| Codex | MCP endpoint en `settings.json` | System prompt |
| Cursor | `.cursor/mcp.json` | `.cursorrules` |
| Cualquiera | Conexión MCP al server | Skill o system prompt |

---

## 6. Modos de Operación

| Modo | Quién orquesta | Cuándo | Iteraciones |
|------|---------------|--------|-------------|
| Interactivo | Humano invoca skill | Durante desarrollo | 1-10 |
| Batch | `opencode run --max-iterations N` | Testing | 10-50 |
| Nocturno | GitHub Actions cron | Cada noche | 50-200 |
| Continuo | Agente con NEVER STOP | Background 24/7 | Ilimitado |

---

## 7. Trazabilidad

Cada experimento es trazable de extremo a extremo:

```
results.tsv:
  commit | health_before | health_after | delta | decision | cost | timestamp

git log auto/nightly:
  a1b2c3d experiment: fix S107 metadata → KEEP
  b2c3d4e experiment: tighten S1135 regex → KEEP
  c3d4e5f experiment: lower S134 threshold → DISCARD

GitHub PR:
  "AutoResearch: Nightly improvements (2026-05-11)"
  Health Score: 0.691 → 0.693 (+0.002)
  2 improvements kept, 1 discarded
```

---

## 8. Seguridad

- El harness de evaluación es **inmutable** para el agente (solo lectura vía MCP)
- Los cambios de pesos (SAGA) requieren **archivo de propuesta + revisión humana**
- Las mejoras al protocolo (Meta) requieren **doble revisión**
- El agente solo accede a herramientas MCP, no al filesystem del harness
- Cada cambio tiene trazabilidad completa: commit git + fila en results.tsv
