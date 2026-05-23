# Flujo Agéntico de Reglas CogniCode

> **Fecha**: 15 de Mayo de 2026
> **Versión**: 1.0
> **Estado**: Diseño acordado tras grillado

---

## 1. Visión General

El workflow de reglas de CogniCode es un **sistema agéntico orquestado** donde el agente principal `rule-orchestrator` delega trabajo a subagentes especializados. El usuario solo arranca el goal; el orquestador gestiona todo el flujo internamente.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         RULE-ORCHESTRATOR                               │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  Rol: Coordinador puro — NO ejecuta, DELEGA                      │   │
│  │  Decisiones: flujo, estado, repetir/saltar fases                │   │
│  │  Criterio: entropy_score (métricas entrópicas)                 │   │
│  │  artifact store: Engram para todo                              │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                    │                                      │
│                    ┌───────────────┼───────────────┐                     │
│                    ▼               ▼               ▼                     │
│            ┌──────────────┐ ┌──────────────┐ ┌──────────────┐           │
│            │   RESEARCH    │ │    DESIGN    │ │  IMPLEMENT   │           │
│            │              │ │              │ │              │           │
│            │ • knowledge  │ │ • designer   │ │ • implementer│           │
│            │   researcher │ │ • test       │ │ • test       │           │
│            │ • concept    │ │   engineer   │ │   engineer   │           │
│            │   normalizer │ │              │ │ • benchmark  │           │
│            │ • legal      │ │              │ │   auditor   │           │
│            │   auditor    │ │              │ │              │           │
│            └──────────────┘ └──────────────┘ └──────────────┘           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Principio Central

> **El orquestador solo hace coordinación pura:**
> - Decidir flujo
> - Leer estados breves
> - Sintetizar reportes
> - Actuar según resultados
>
> **Todo lo demás son los subagentes.**

El orquestador no explora fuentes, no diseña reglas, no escribe código, no ejecuta tests. Es el jefe que delega en sus empleados, organiza y evalúa resultados, y actúa en consecuencia.

---

## 3. Comandos del Workflow

| Comando | Fases que ejecuta | Propósito |
|---------|-------------------|-----------|
| `/rules-new <batch>` | research → normalize → legal → design | Crear batch desde tema/fuente |
| `/rules-ff <batch>` | normalize → legal → design → fixture → tasks | Fast-forward de planning |
| `/rules-apply <batch>` | implement → test → benchmark | Implementar reglas pendientes |
| `/rules-verify <batch>` | review → commit-plan | Verificar contra quality gates |
| `/rules-archive <batch>` | state → published/deprecated | Publicar/archivar batch |
| `/rules-feedback <issue>` | feedback → regression → redesign | Iterar sobre feedback |

### 3.1 Flujo Dinámico

El orquestador puede **repetir o saltar fases** según su juicio basado en entropy_score. No es un pipeline rígido.

```
/rules-new tema
    │
    ├─► research ─► normalize ─► legal ─► design
    │        │                      │           │
    │        │                      │           ▼
    │        │                      │      [evalúa]
    │        │                      │           │
    │        ▼                      ▼           ▼
    │    [repetir?] ────► [saltar a implement?]
    │
    └──────────────────────────────────────────► siguiente comando
```

---

## 4. Arquitectura de Subagentes

### 4.1 Lista de Subagentes

| Agente | Rol | Gates que actualiza |
|--------|-----|-------------------|
| `rule-knowledge-researcher` | Investiga conocimiento público, registra procedencia | `discovered` |
| `rule-concept-normalizer` | Deduplica candidatos, normaliza a `RuleKnowledge` | `normalized`, `duplicate` |
| `rule-legal-auditor` | Audita licencia y procedencia | `legal_blocked`, `reference_only` |
| `rule-designer` | Diseña estrategia, AST patterns, metadata Axiom | `designed`, `redesign_required` |
| `rule-test-engineer` | Crea fixture matrix y tests | `fixtures_ready` |
| `rule-implementer` | Implementa reglas desde diseños aprobados | `implemented`, `implementation_failed` |
| `rule-benchmark-auditor` | Mide rendimiento, detecta regresiones | `benchmarked`, `benchmark_failed` |
| `rule-reviewer` | Verifica precisión, metadata, SARIF, quality gates | `approved`, `review_failed` |
| `rule-commit-orchestrator` | Planifica commits coherentes | `committed` |

### 4.2 Herramientas por Agente

Cada subagente tiene tools específicas:

- **`rule-knowledge-researcher`**: `context7_query-docs`, `webfetch`, `cognicode_semantic_search`
- **`rule-concept-normalizer`**: `cognicode_build_lightweight_index`, `cognicode_semantic_search`
- **`rule-designer`**: `cognicode_analyze_impact`, `cognicode_build_graph`, `cognicode_get_complexity`
- **`rule-implementer`**: `cognicode_analyze_impact`, `cognicode_find_usages`, `cognicode-quality_analyze_file`
- **`rule-reviewer`**: `cognicode_check_architecture`, `cognicode-quality_run_quality_gate`

---

## 5. Artifact Store (Engram)

Todos los artifacts se guardan en **Engram** con topic keys estándar:

```
rules/{batch}/state              — registro operacional (fuente de verdad)
rules/{batch}/knowledge-research  — investigación de fuentes
rules/{batch}/concepts           — conceptos normalizados
rules/{batch}/legal-review        — auditoría legal
rules/{batch}/rule-designs       — diseños técnicos
rules/{batch}/fixture-matrix     — matriz de tests
rules/{batch}/tasks             — tareas de implementación
rules/{batch}/apply-progress    — progreso de implementación
rules/{batch}/test-report       — reporte de tests
rules/{batch}/benchmark-report   — reporte de rendimiento
rules/{batch}/review-report     — reporte de revisión
rules/{batch}/commit-plan       — plan de commits
rules/{batch}/archive-report     — reporte de archivo
rules/{batch}/entropy-report    — métricas entrópicas
```

### 5.1 Delegación con Contexto

Cuando el orquestador delega a un subagente:

1. **Empaqueta contexto completo en el prompt**
2. **Si es demasiado largo**, referencia el artifact en Engram para que el subagente lo lea
3. **El subagente no lee artifacts por su cuenta** — recibe todo lo necesario

---

## 6. Estados y Ciclo de Vida

### 6.1 Identidades

| Identidad | Ejemplo | Propósito |
|-----------|---------|-----------|
| `concept_id` | `concept/security/sql-injection` | Problema abstracto deduplicado |
| `candidate_id` | `candidate/semgrep/python.django.sql-injection` | Entrada desde fuente externa |
| `rule_id` | `CGR-RUST-SEC-001` | Regla publicada en CogniCode |

### 6.2 Estados

```
discovered → normalized → duplicate | legal_blocked | reference_only | candidate
candidate → designed → fixtures_ready → implementing
implementing → implemented | implementation_failed
implemented → testing → tested | test_failed
tested → benchmarking → benchmarked | benchmark_failed
benchmarked → reviewing → review_failed | approved
approved → committed → published
published → feedback_open | deprecated
deprecated → archived
```

### 6.3 Transiciones

Cada transición registra:
- Timestamp
- Agente que la ejecutó
- Gate aprobado
- Motivo si hay fallo

### 6.4 Reglas de Transición

1. **Reevaluación**: al repetir una fase, se revisa y actualiza el estado anterior
2. **Rollback**: debe poder volver atrás lógicamente si se descubre un error
3. **Continuidad**: pérdida de continuidad del registro es CRÍTICA y para el workflow

---

## 7. Gates entre Fases

| Siguiente fase | Gate requerido |
|----------------|----------------|
| `rule-design` | Concept normalizado + procedencia registrada + **legal aprobado** |
| `fixture-matrix` | Diseño con estrategia, exclusiones, coste, metadata Axiom |
| `implementation` | Legal aprobado + fixtures definidos |
| `benchmark` | Tests unitarios e integración pasan |
| `review` | Benchmark dentro de presupuesto |
| `commit` | Review sin críticos + SARIF/metadata completos |
| `archive` | Commits o PR preparados + KB actualizada |

---

## 8. Criterio de Decisión: Entropy Score

El orquestador usa **métricas entrópicas** para decidir:
- ¿Repetir una fase?
- ¿Saltar una fase?
- ¿Parar el workflow?

### 8.1 Fuentes de Métricas

- **`entropy-sdd`**: Connascence metrics, SOLID entropy, Information Bottleneck
- **`cognicode-quality-sdd`**: Code smells, complexity, technical debt

### 8.2 Thresholds

Los thresholds exactos se definen experimentalmente. Guía general:
- Si >50% de reglas fallan → repetir fase
- Si <20% de reglas fallan → continuar
- Entropy score alto → redesign requerido

---

## 9. Paralelismo

El orquestador **decide si subagentes corren en paralelo o secuencial** según lo estime.

```
Paralelo (independientes):          Secuencial (dependientes):
┌─────────┐ ┌─────────┐            ┌─────────┐
│implement│ │ test    │            │ design  │──► │implement│
│  -er    │ │engineer │            └─────────┘    └─────────┘
└─────────┘ └─────────┘
```

---

## 10. Skills Injectados por Contexto

El orquestador inyecta skills compactos en prompts delegados:

| Skill | Propósito |
|-------|-----------|
| `cognicode-rules` | Reglas Axiom, AST-first, Issue::from_node, metadata |
| `rule-knowledge-workflow` | Research, normalización, dedupe |
| `rule-legal-provenance` | Licencia, procedencia, reference-only |
| `rule-operational-registry` | Estados, métricas, transiciones |
| `rule-test-matrix` | Positivos, negativos, edge, FP, performance |
| `rule-performance-budget` | layer(), required_keywords(), ms/file |
| `rule-agent-semantics` | Fix playbook, RAG chunks, review questions |
| `rust-testing` | Tests Rust cuando se escriban fixtures |

---

## 11. Formato de Retorno de Subagentes

```markdown
status: success | failed | blocked | duplicate
executive_summary: >
  Frase de 1-2 líneas con el resultado principal.

artifacts:
  - rules/{batch}/artifact-name

rules_processed:
  successes: [rule_id, ...]
  failures: [rule_id: reason, ...]
  blocked: [candidate_id: gate, ...]
  duplicates: [candidate_id, ...]

batch_metrics:
  discovered: N
  normalized: N
  ...

next_recommended: next-phase | stop | pause
risks:
  - Risk description
entropy_score: N.N
```

---

## 12. Relación con Otros Documentos

| Documento | Descripción |
|-----------|-------------|
| `ORCHESTRATOR.md` | Detalle del rol y responsabilidades del orquestador |
| `SUBAGENTS.md` | Especificación de cada subagente |
| `STATE-MACHINE.md` | Máquina de estados completa con transiciones |
| `GATES.md` | Gates detallados con criterios de paso |
| `.opencode/skills/cognicode-rules/SKILL.md` | Patrones de reglas (a crear) |

---

*Documento creado tras grillado con documentación de decisiones tomadas*
*Última actualización: 15 de Mayo de 2026*