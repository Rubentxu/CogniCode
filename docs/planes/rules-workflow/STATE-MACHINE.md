# State Machine — Ciclo de Vida de Reglas

> **Fecha**: 15 de Mayo de 2026
> **Versión**: 1.0
> **Estado**: Diseño acordado tras grillado

---

## 1. Fuentes de Verdad

`rules/{batch}/state` es la **fuente de verdad** para todo el workflow.

### 1.1 Tres Identidades

| Identidad | Ejemplo | Propósito |
|-----------|---------|-----------|
| `concept_id` | `concept/security/sql-injection` | Problema abstracto deduplicado |
| `candidate_id` | `candidate/semgrep/python.django.sql-injection` | Entrada desde fuente externa |
| `rule_id` | `CGR-RUST-SEC-001` | Regla implementada/publicada |

### 1.2 Integridad

> **Regla crítica**: La pérdida de continuidad del registro es CRÍTICA y debe parar el workflow.

---

## 2. Estados del Ciclo de Vida

```
discovered ─────────────────────────────────────────────────────────►
    │                                                               │
    ▼                                                               │
normalized ──► duplicate ─────────────────────────────────────────►│
    │               │                                              │
    ▼               ▼                                              │
legal_blocked ◄──────┴──────► reference_only ◄───────────────────►│
    │                              │                               │
    ▼                              ▼                               │
candidate ◄─────────────────────────────────────────────────────►│
    │                                                           │
    ▼                                                           │
designed ◄─────────────────────────────────────────────────────►│
    │                                                           │
    ▼                                                           │
fixtures_ready ◄──────────────────────────────────────────────►│
    │                                                           │
    ▼                                                           │
implementing ──► implementation_failed ◄────────────────────►│
    │                                                         │
    ▼                                                         │
implemented ◄──────────────────────────────────────────────►│
    │                                                         │
    ▼                                                         │
testing ──► test_failed ◄────────────────────────────────────►│
    │                                                         │
    ▼                                                         │
tested ◄────────────────────────────────────────────────────►│
    │                                                         │
    ▼                                                         │
benchmarking ──► benchmark_failed ◄────────────────────────►│
    │                                                         │
    ▼                                                         │
benchmarked ◄──────────────────────────────────────────────►│
    │                                                         │
    ▼                                                         │
reviewing ──► review_failed ◄─────────────────────────────►│
    │                                                         │
    ▼                                                         │
approved ◄────────────────────────────────────────────────►│
    │                                                         │
    ▼                                                         │
committed ◄──────────────────────────────────────────────►│
    │                                                         │
    ▼                                                         │
published ◄──────────────────────────────────────────────►│
    │                    │                                   │
    ▼                    ▼                                   │
feedback_open         deprecated                             │
    │                    │                                   │
    └────────┬──────────┘                                   │
             ▼                                              ▼
          archived ◄─────────────────────────────────────►│
```

---

## 3. Transiciones Detalladas

### 3.1 Fase de Research

| Estado | Transición | Condición | Agente |
|--------|------------|-----------|--------|
| (inicio) | → discovered | Candidato encontrado | rule-knowledge-researcher |
| discovered | → normalized | Concepto deduplicado y normalizado | rule-concept-normalizer |
| discovered | → duplicate | Ya existe concepto similar | rule-concept-normalizer |

### 3.2 Fase de Legal

| Estado | Transición | Condición | Agente |
|--------|------------|-----------|--------|
| normalized | → legal_blocked | Problema de licencia | rule-legal-auditor |
| normalized | → reference_only | Solo referencia, no derivar | rule-legal-auditor |
| normalized | → candidate | Aprobado para derivación | rule-legal-auditor |

### 3.3 Fase de Diseño

| Estado | Transición | Condición | Agente |
|--------|------------|-----------|--------|
| candidate | → designed | Diseño completo con estrategia | rule-designer |
| candidate | → redesign_required | Diseño necesita revisión | rule-designer |
| designed | → redesign_required | Problema detectado en review | rule-reviewer |
| designed | → fixtures_ready | Fixtures y tasks definidos | rule-test-engineer |

### 3.4 Fase de Implementación

| Estado | Transición | Condición | Agente |
|--------|------------|-----------|--------|
| fixtures_ready | → implementing | Inicio de implementación | rule-implementer |
| implementing | → implemented | Implementación completa | rule-implementer |
| implementing | → implementation_failed | Error en implementación | rule-implementer |

### 3.5 Fase de Testing

| Estado | Transición | Condición | Agente |
|--------|------------|-----------|--------|
| implemented | → testing | Inicio de tests | rule-test-engineer |
| testing | → tested | Tests pasan | rule-test-engineer |
| testing | → test_failed | Tests fallan | rule-test-engineer |

### 3.6 Fase de Benchmark

| Estado | Transición | Condición | Agente |
|--------|------------|-----------|--------|
| tested | → benchmarking | Inicio de benchmark | rule-benchmark-auditor |
| benchmarking | → benchmarked | Dentro de budget | rule-benchmark-auditor |
| benchmarking | → benchmark_failed | Excede budget | rule-benchmark-auditor |

### 3.7 Fase de Review

| Estado | Transición | Condición | Agente |
|--------|------------|-----------|--------|
| benchmarked | → reviewing | Inicio de review | rule-reviewer |
| reviewing | → approved | Sin críticos | rule-reviewer |
| reviewing | → review_failed | Críticos encontrados | rule-reviewer |

### 3.8 Fase de Commit

| Estado | Transición | Condición | Agente |
|--------|------------|-----------|--------|
| approved | → committed | Commit creado | rule-commit-orchestrator |

### 3.9 Fase de Publicación

| Estado | Transición | Condición | Agente |
|--------|------------|-----------|--------|
| committed | → published | En producción | rule-orchestrator |
| published | → feedback_open | Feedback recibido | rule-orchestrator |
| published | → deprecated | Obsoleto | rule-orchestrator |
| deprecated | → archived | Archivado | rule-orchestrator |

---

## 4. Formato de Transición

Cada transición registra:

```yaml
transitions:
  - from: discovered
    to: normalized
    timestamp: 2026-05-15T10:30:00Z
    agent: rule-concept-normalizer
    gate: concept_id_generated
    reason: null
    artifacts:
      - rules/{batch}/concepts
```

### 4.1 Campos de Transición

| Campo | Descripción |
|-------|-------------|
| `from` | Estado anterior |
| `to` | Estado nuevo |
| `timestamp` | ISO 8601 |
| `agent` | Subagente que ejecutó |
| `gate` | Gate que se cumplió |
| `reason` | Motivo si falló |
| `artifacts` | artifacts generados |

---

## 5. Métricas por Batch

Se calculan después de cada fase y se guardan en `rules/{batch}/state`:

```yaml
batch_metrics:
  discovered: N           # Candidatos encontrados
  normalized: N           # Conceptos normalizados
  duplicates: N           # Duplicados detectados
  legal_blocked: N        # Bloqueados por legal
  candidates: N            # Aprobados para diseño
  designed: N              # Con diseño completo
  implemented: N           # Implementación completa
  published: N             # En producción

  implementation_failed: N # Fallos en implementación
  test_failed: N          # Fallos en tests
  benchmark_failed: N      # Fallos en benchmark
  review_failed: N         # Fallos en review

  median_file_ms: N.N      # Mediana tiempo por archivo
  false_positive_rate_estimate: N.N  # Estimación FP
  cwe_coverage_added: [CWE-...]      # Coverage CWE añadido
```

---

## 6. Reevaluación y Rollback

### 6.1 Reevaluación

Al repetir una fase, se revisa y actualiza el estado anterior:
- El subagente lee el estado completo
- Actualiza solo sus reglas
- Añade nuevas transiciones (append-only)
- Recalcula métricas

### 6.2 Rollback

Si se descubre un error después de avanzar:

```
designed → implemented → redesign_required
     ▲                              │
     │                              │
     └──────────────────────────────┘
```

El rollback es **lógico**, no físico. Se añaden nuevas transiciones que reflejan el estado correcto.

### 6.3 Ejemplo de Rollback

```yaml
transitions:
  - from: designed
    to: implemented
    timestamp: 2026-05-14T15:00:00Z
    agent: rule-implementer
    gate: implementation_complete

  - from: implemented
    to: redesign_required
    timestamp: 2026-05-15T09:00:00Z
    agent: rule-reviewer
    gate: critical_issues_found
    reason: "AST pattern incorrecto para el caso edge"
```

---

## 7. Continuidad del Registro

### 7.1 Verificación

Antes de delegar a un subagente, el orquestador verifica:
1. `rules/{batch}/state` existe
2. Transiciones son válidas (no se saltan estados requeridos)
3. Métricas son consistentes

### 7.2 Recuperación

Si el registro se corrompe:
1. El orquestador detecta la inconsistencia
2. Para el workflow
3. Requiere intervención manual para reparar

### 7.3 Consistencia

> **Regla**: El orquestador es responsable de la validez de las transiciones. No se delegan transiciones inválidas.

---

## 8. Estados Especiales

### 8.1 duplicate

Un `candidate_id` que representa el mismo problema que un `concept_id` existente.

- No se diseña nueva regla
- Se vincula al concepto existente
- Se registra la fuente duplicada

### 8.2 reference_only

Un candidato usable como referencia pero no para derivar código directamente.

- El `rule-designer` reformula desde el problema abstracto
- No se copia implementación de la fuente

### 8.3 legal_blocked

Un candidato que no puede usarse por problemas de licencia.

- Se registra el motivo legal
- No se continúa con el workflow

### 8.4 redesign_required

Un diseño que necesita revisión antes de implementar.

- Se lanzan los subagentes necesarios para corregir
- Se registra el motivo específico

---

## 9. Gatos (Gates) entre Fases

| Siguiente fase | Gate requerido |
|----------------|----------------|
| `rule-design` | concept normalizado + procedencia + **legal aprobado** |
| `fixture-matrix` | diseño con estrategia, exclusiones, coste, Axiom |
| `implementation` | legal aprobado + fixtures definidos |
| `benchmark` | tests pasan |
| `review` | benchmark dentro de presupuesto |
| `commit` | review sin críticos + SARIF/metadata |
| `archive` | commit/PR + KB actualizada |

---

*Documento creado tras grillado*
*Última actualización: 15 de Mayo de 2026*