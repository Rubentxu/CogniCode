# 11 — Patterns Catalog

> Catálogo de patrones de diseño para sistemas de agentes autónomos de mejora
> de software. Extraídos del análisis de Karpathy, SAGA, Deep Researcher, y
> la implementación de CogniCode.

---

## Patrón 1: Autonomous Experiment Loop

```
También conocido como: Karpathy Loop, FOREVER Loop

Propósito: Un agente ejecuta indefinidamente un ciclo de hipótesis →
implementación → evaluación → accept/reject sin intervención humana.

Estructura:
┌────────────────────────────────────────────┐
│  1. EVALUATE baseline                      │
│  2. FORMULATE hypothesis                   │
│  3. IMPLEMENT minimal change               │
│  4. PRE-VALIDATE (fast gates)              │
│  5. COMMIT (checkpoint)                    │
│  6. EVALUATE after                         │
│  7. DECIDE keep/discard                    │
│  8. LOG result                             │
│  9. REPEAT from 1                          │
└────────────────────────────────────────────┘

Componentes:
- Fixed harness: evaluación inmutable y determinista
- Git as memory: commit = checkpoint, reset = rollback
- Simplicity criterion: no aceptar complejidad sin mejora medible
- Time budget: cada iteración usa recursos acotados

Cuándo usarlo: Cuando el espacio de posibles mejoras es amplio y cada
evaluación es costosa pero automatizable.

Ejemplo: Karpathy optimizando BPB de un modelo GPT. CogniCode optimizando
Health Score de un proyecto Rust.
```

---

## Patrón 2: Fixed Evaluation Harness

```
También conocido como: Sacred Harness, Design by Contract for Agents

Propósito: Separar la función de evaluación del espacio de búsqueda, haciendo
la primera inmutable para el agente.

Estructura:
┌────────────────────────────────────────────┐
│  HARNESS (inmutable)                       │
│  ┌──────────────────────────────────────┐  │
│  │  GATES (pass/fail)                   │  │
│  │  ├─ CompilationGate                  │  │
│  │  ├─ TestsGate                        │  │
│  │  └─ LintGate                         │  │
│  │                                      │  │
│  │  METRICS (0.0-1.0)                   │  │
│  │  ├─ ComplexityMetric                 │  │
│  │  ├─ SolidMetric                      │  │
│  │  └─ ...                              │  │
│  │                                      │  │
│  │  HEALTH SCORE = Σ(W × metric)        │  │
│  └──────────────────────────────────────┘  │
│                                            │
│  ⛔ AGENT CANNOT MODIFY THIS               │
└────────────────────────────────────────────┘
         │
         │ reads results
         ▼
┌────────────────────────────────────────────┐
│  AGENT (mutable scope)                     │
│  ┌──────────────────────────────────────┐  │
│  │  crates/ (source code)               │  │
│  │  tests/ (test code)                  │  │
│  │                                      │  │
│  │  ✅ AGENT CAN MODIFY THESE           │  │
│  └──────────────────────────────────────┘  │
└────────────────────────────────────────────┘

Beneficios:
- Previene reward hacking (el agente no puede falsear la métrica)
- Garantiza comparabilidad entre experimentos
- La métrica es la verdad absoluta del sistema

Riesgo si se omite: El agente puede "optimizar" la métrica sin mejorar la
calidad real (ej: borrar tests para que pasen, modificar la fórmula del score).

Ejemplo: prepare.py de Karpathy. La función evaluate_bpb() nunca cambia.
```

---

## Patrón 3: Git as Memory

```
Propósito: Usar el historial de control de versiones como memoria externa
del agente y mecanismo de aceptación/rechazo de experimentos.

Estructura:
┌────────────────────────────────────────────┐
│  Rama principal: main                      │
│  ├─ commit A (baseline)                    │
│  ├─ commit B (keep: "improved LR")         │
│  ├─ commit C (keep: "added RoPE")          │
│  └─ commit D (HEAD)                        │
│                                            │
│  Rama de trabajo: auto/nightly             │
│  ├─ commit A (baseline)                    │
│  ├─ commit X (keep)                        │
│  ├─ commit Y (discard → reset)            │
│  └─ commit Z (keep)                        │
└────────────────────────────────────────────┘

Operaciones:
- git commit → checkpoint del experimento
- git reset → descartar experimento fallido
- git log → consultar historial de intentos
- git diff → comparar cambios entre experimentos

Beneficios:
- Auditabilidad perfecta (cada cambio tiene commit message + diff)
- Rollback determinista (git reset a cualquier estado anterior)
- Branching para multi-agente (cada agente en su rama)
- Separación limpia entre intentos exitosos (en la rama) y fallidos (borrados)

Ejemplo: El bucle de Karpathy usa git add/commit/reset como mecanismo central.
```

---

## Patrón 4: SAGA Rebalancing

```
También conocido como: Two-Tier Self-Evolving Agent

Propósito: Un bucle externo (cada ~50 iteraciones) analiza la eficiencia del
bucle interno y ajusta sus parámetros (pesos del Health Score).

Estructura:
┌────────────────────────────────────────────┐
│  OUTER LOOP (SAGA) — every ~50 iterations  │
│                                            │
│  1. ANALYZE last N iterations              │
│     ├─ Which dimensions improved most?     │
│     ├─ Which dimensions are stagnant?      │
│     └─ Which have most headroom?           │
│                                            │
│  2. PROPOSE new weights                    │
│     ├─ Boost dimensions with high headroom │
│     ├─ Reduce stagnant dimensions          │
│     └─ Maintain productive dimensions      │
│                                            │
│  3. HUMAN REVIEWS (approve/reject)         │
│                                            │
│  4. APPLY new weights to inner loop        │
└──────────────┬─────────────────────────────┘
               │ weights
               ▼
┌────────────────────────────────────────────┐
│  INNER LOOP (every iteration)              │
│                                            │
│  Same Karpathy loop, but with              │
│  UPDATED health score weights              │
└────────────────────────────────────────────┘

Cuándo usarlo: Cuando el sistema tiene múltiples dimensiones de calidad y
la importancia relativa de cada dimensión cambia con el tiempo.

Ejemplo: CogniCode empieza priorizando SONARQUBE (46% accuracy → urgente),
luego SAGA rebalancea hacia CLIPPY cuando SONARQUBE llega al 92%.
```

---

## Patrón 5: Meta-Agent Improvement

```
También conocido como: autoautoresearch, Self-Improving Protocol

Propósito: Un agente de nivel superior analiza y mejora el propio protocolo
de mejora (program.md, SKILL.md).

Estructura:
┌────────────────────────────────────────────┐
│  META-AGENT — every ~200 iterations        │
│                                            │
│  Analyzes:                                 │
│  ├─ Failure patterns: 30% discard due to X │
│  ├─ Cost efficiency: $ per delta_health    │
│  ├─ Time distribution: bottlenecks         │
│  └─ Improvement rate: are we slowing down? │
│                                            │
│  Proposes:                                 │
│  ├─ New rules in program.md                │
│  ├─ Removal of ineffective rules           │
│  ├─ New pre-validation gates               │
│  └─ Protocol optimizations                 │
│                                            │
│  HUMAN REVIEWS before applying             │
└────────────────────────────────────────────┘

Ejemplo concreto:
  Meta-agent detecta que 30% de discard son por "lint gate failed".
  Propone añadir al protocolo: "Run lint gate in pre-validation (step 5)".
  Resultado: esos fallos se detectan en 30s en vez de 3min.
```

---

## Patrón 6: Competitive Swarm

```
Propósito: Múltiples agentes exploran el espacio de mejoras en paralelo,
compitiendo por el mejor resultado. Un orquestador mergea periódicamente.

Estructura:
┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
│ Agent A  │  │ Agent B  │  │ Agent C  │  │ Agent D  │
│ rules    │  │ python   │  │ perf     │  │ bugs     │
└────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘
     │              │              │              │
     └──────────────┴──────────────┴──────────────┘
                         │
                ┌────────▼────────┐
                │  ORCHESTRATOR   │
                │  • merge        │
                │  • evaluate     │
                │  • select best  │
                └────────┬────────┘
                         │
                ┌────────▼────────┐
                │   MAIN BRANCH   │
                └─────────────────┘

Merge strategies:
- Winner-takes-all: solo el mejor commit sobrevive
- Merge-all-passing: todos los que mejoran health
- Tournament: competición por rondas

Cuándo usarlo: Cuando hay recursos para ejecutar múltiples agentes en
paralelo y el espacio de búsqueda es particionable (diferentes componentes).

Ejemplo: Deep Researcher ejecutó 4 proyectos concurrentes durante 30+ días.
```

---

## Patrón 7: Backlog-Driven Autonomy

```
Propósito: El humano proporciona dirección estratégica mediante un backlog
priorizado. El agente ejecuta tácticamente con autonomía total.

Estructura:
┌────────────────────────────────────────────┐
│  HUMANO                                    │
│  ┌──────────────────────────────────────┐  │
│  │ backlog.md                           │  │
│  │                                      │  │
│  │ [P0] Fix security vulnerability      │  │
│  │ [P1] Reduce complexity in module X   │  │
│  │ [P2] Add documentation for API Y     │  │
│  └──────────────────────────────────────┘  │
└──────────────┬─────────────────────────────┘
               │
               ▼
┌────────────────────────────────────────────┐
│  AGENT                                     │
│                                            │
│  1. Check backlog for pending items        │
│  2. If P0 items → do them first            │
│  3. If P1 items in current SDLC phase → do │
│  4. If backlog empty → free exploration    │
│     guided by Health Score breakdown       │
└────────────────────────────────────────────┘

Beneficio: Combina dirección estratégica humana con ejecución táctica
autónoma. El humano decide QUÉ; el agente decide CÓMO.
```

---

## Patrón 8: Backtrack Chain

```
Propósito: Cuando una fase SDLC falla, retroceder a la fase más temprana
que puede corregir el problema, en lugar de fallar completamente.

Estructura:
Deploy (API break) → backtrack a Coding (fix signature)
Test (coverage gap) → backtrack a Coding (add tests)
Coding (build fail) → backtrack a Design (rethink approach)

Implementación:
pub trait SdlcPipeline {
    fn backtrack(&self, failure: &PipelineResult)
        -> Option<(SdlcPhase, ChangeSuggestion)>;
}

Cuándo usarlo: En pipelines multi-fase donde los fallos en fases tardías
a menudo se originan en fases tempranas.
```

---

## Patrón 9: Differential Validation

```
Propósito: Comparar el comportamiento OLD vs NEW del código para detectar
regresiones sutiles que los tests no cubren.

Estructura:
┌────────────────────────────────────────────┐
│  1. Ejecutar código OLD con inputs de test │
│     └─▶ Capturar outputs + traces          │
│                                            │
│  2. Aplicar cambio (agente modifica código)│
│                                            │
│  3. Ejecutar código NEW con mismos inputs  │
│     └─▶ Capturar outputs + traces          │
│                                            │
│  4. Comparar (Chronos)                     │
│     ├─ ¿Outputs difieren? → potential bug  │
│     ├─ ¿Nuevos crashes? → DISCARD          │
│     └─ ¿Performance regression? → warning  │
└────────────────────────────────────────────┘

Herramientas: Chronos MCP (time-travel debugging) para comparar trazas
de ejecución entre versiones.
```

---

## Mapa de Relaciones entre Patrones

```
Autonomous Experiment Loop (1)
    │
    ├──► Fixed Evaluation Harness (2)      [requerido por 1]
    ├──► Git as Memory (3)                  [requerido por 1]
    ├──► SAGA Rebalancing (4)              [extiende 1 con bucle externo]
    ├──► Meta-Agent Improvement (5)         [extiende 1 con meta-nivel]
    ├──► Competitive Swarm (6)             [paraleliza 1]
    ├──► Backlog-Driven Autonomy (7)       [alimenta 1 con dirección humana]
    ├──► Backtrack Chain (8)              [maneja fallos en 1]
    └──► Differential Validation (9)       [mejora evaluación en 2]
```

---

## Siguiente: [12 — Agent Integration](12-agent-integration.md)
