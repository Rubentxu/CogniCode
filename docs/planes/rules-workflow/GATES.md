# Gates — Criterios de Paso entre Fases

> **Fecha**: 15 de Mayo de 2026
> **Versión**: 1.0
> **Estado**: Diseño acordado tras grillado

---

## 1. Visión General

Un **gate** es una condición que debe cumplirse antes de avanzar a la siguiente fase. No se avanza si falta el gate.

### 1.1 Lista de Gates

| Fase | Gate requerido |
|------|----------------|
| `rule-design` | concepto normalizado + procedencia registrada + **legal aprobado** |
| `fixture-matrix` | diseño con estrategia, exclusiones, coste, metadata Axiom |
| `implementation` | legal aprobado + fixtures definidos |
| `benchmark` | tests pasan |
| `review` | benchmark dentro de presupuesto |
| `commit` | review sin críticos + SARIF/metadata completos |
| `archive` | commit/PR preparado + KB actualizada |

---

## 2. Gate: Diseño (rule-design)

### 2.1 Condiciones

1. **concepto normalizado**: Existe `concept_id` creado por `rule-concept-normalizer`
2. **procedencia registrada**: Fuente, URL, licencia documentadas en `rules/{batch}/knowledge-research`
3. **legal aprobado**: `rule-legal-auditor` marcó el candidato como `candidate` (no `legal_blocked` ni `reference_only`)

### 2.2 Verificación

El orquestador verifica antes de lanzar `rule-designer`:
```yaml
candidate.state in [normalized]
candidate.legal_status == approved
candidate.provenance.url exists
candidate.provenance.license exists
```

### 2.3 Fallo

Si no se cumple el gate:
- No se lanza `rule-designer`
- Se reporta qué candidatos están bloqueados y por qué
- El workflow se pausa hasta que se resuelva

---

## 3. Gate: Fixtures (fixture-matrix)

### 3.1 Condiciones

El diseño de cada regla debe incluir:

1. **estrategia**: Detection strategy definida (regex/token, AST query, visitor, metrics, call graph, data flow)
2. **exclusiones**: False positives conocidos documentados
3. **coste**: Estimación LOC y complejidad
4. **metadata Axiom**:
   - category
   - cwe, owasp, cert
   - impact (reliability, availability, correctness)
   - fix_effort
   - remediation

### 3.2 Verificación

```yaml
design.strategy exists
design.exclusion_patterns is not empty
design.estimated_loc > 0
design.axiom.category exists
design.axiom.cwe is not empty
```

### 3.3 Formato del Diseño

```yaml
rule_design:
  rule_id: S1872
  strategy: tree-sitter query
  ast_pattern: |
    (closure_expression
      body: (block
        (statement
          (expression_statement
            (assignment_expression ...)))))
  exclusions:
    - identifier wrapped in Arc<Mutex<T>>
    - atomic types
  estimated_loc: 160
  complexity:
    cognitive: 14
    cyclomatic: 3
  axiom:
    category: concurrency-bug
    cwe: CWE-362
```

---

## 4. Gate: Implementación (implementation)

### 4.1 Condiciones

1. **legal aprobado**: Sin cambios desde el gate de diseño
2. **fixtures definidos**: `rules/{batch}/fixture-matrix` existe con tests para esta regla

### 4.2 Verificación

```yaml
rule.legal_status == approved
fixture_matrix[rule_id] exists
fixture_matrix[rule_id].positives >= 2
fixture_matrix[rule_id].negatives >= 2
fixture_matrix[rule_id].edge_cases >= 1
```

### 4.3 Reglas Duras de Implementación

El `rule-implementer` debe cumplir:

- No implementar sin `legal-review` aprobado
- No implementar sin `fixture-matrix` existente
- Usar AST/query/visitor antes que regex
- Usar `Issue::from_node` cuando exista nodo primario
- Definir `layer()` y `required_keywords()`
- Compilar queries/regex con `OnceLock`/`LazyLock`

---

## 5. Gate: Benchmark (benchmark)

### 5.1 Condiciones

Todos los tests unitarios e de integración **pasan**.

### 5.2 Verificación

```yaml
test_report.total_passed > 0
test_report.total_failed == 0
test_report.positive_tests > 0
test_report.negative_tests > 0
```

### 5.3 Budgets por Archivo

| Tipo | Budget |
|------|--------|
| regex/token | 1 ms |
| AST query | 3 ms |
| visitor | 5 ms |
| metric | 2 ms |
| call graph | 10 ms |
| taint/data-flow | 25 ms |

### 5.4 Benchmark Report

```yaml
benchmark_report:
  rule_id: S1872
  measurements:
    - file: src/main.rs
      layer: 1
      time_ms: 2.3
      budget_ms: 3
      status: within_budget
    - file: src/lib.rs
      layer: 1
      time_ms: 4.1
      budget_ms: 3
      status: exceeded
  median_file_ms: 2.8
  budget: 3
  delta: -0.2
  recommendation: accept_with_explanation
```

### 5.5 Decisión Post-Benchmark

| Resultado | Decisión |
|-----------|----------|
| Dentro de budget | Avanzar a review |
| Excede budget | Marcar pero no descartar. Avanzar con explicación |
| Regresión detectada | Repetir diseño o implementación |

---

## 6. Gate: Review (review)

### 6.1 Condiciones

El benchmark está dentro del presupuesto.

### 6.2 Checklist de Review

| Check | Descripción | Severity |
|-------|-------------|----------|
| legal_provenance | Legal/procedencia aprobada | CRITICAL |
| test_coverage | Tests positivos, negativos, edge y FP cubiertos | CRITICAL |
| performance | Performance dentro de presupuesto | CRITICAL |
| metadata_completeness | Rule con metadata UI, Clean Code, qualities, tags | WARNING |
| issue_enrichment | Issue incluye snippet, entidad, scope, variable, remediación | WARNING |
| agent_semantics | agent_semantics y fix playbook presentes | WARNING |
| sarif_descriptor | SARIF descriptor y fingerprints estables | WARNING |
| no_duplication | No duplicado de regla existente | SUGGESTION |

### 6.3 Clasificación de Hallazgos

- **CRITICAL**: Debe resolverse antes de aprobar
- **WARNING**: Recomendación que no bloquea
- **SUGGESTION**: Mejora opcional

### 6.4 Veredicto

| Resultado | Estado |
|-----------|--------|
| Sin CRITICAL | `approved` |
| Con CRITICAL | `review_failed` |

---

## 7. Gate: Commit (commit)

### 7.1 Condiciones

1. **Review aprobado**: Sin CRITICAL en el review
2. **SARIF/metadata completos**: Issue con todos los campos requeridos

### 7.2 Verificación

```yaml
review.status == approved
review.critical_count == 0
rule.sarif_descriptor exists
rule.metadata_complete == true
```

### 7.3 Commit Plan

```yaml
commit_plan:
  batch: bug-concurrency
  commits:
    - unit: infrastructure
      files:
        - crates/cognicode-axiom/src/rules/mod.rs
        - crates/cognicode-axiom/src/rules/catalog.rs
      message: "feat(rules): add concurrency rules infrastructure"
    - unit: s1872-race-condition
      files:
        - crates/cognicode-axiom/src/rules/rust/bugs/concurrency/s1872_rule.rs
        - crates/cognicode-axiom/src/rules/rust/bugs/concurrency/s1872_test.rs
      message: "feat(rule): implement S1872 race condition detection"
  pr_summary: |
    ## Summary
    - 8 concurrency bug detection rules
    - Cover CWE-362, CWE-821, CWE-833, etc.
    - ~1200 LOC total
```

---

## 8. Gate: Archive (archive)

### 8.1 Condiciones

1. **Commit o PR preparado**: Plan de commit creado
2. **KB actualizada**: Knowledge base refleja las nuevas reglas

### 8.2 Verificación

```yaml
commit_plan exists
commit_plan.commits is not empty
kb.updated == true
```

### 8.3 Archive Report

```yaml
archive_report:
  batch: bug-concurrency
  rules_published: 8
  rules_attempted: 10
  implementation_failed: 1
  test_failed: 1
  cwe_coverage_added:
    - CWE-362 (Race Condition)
    - CWE-821 (Use of Incorrect Synchronization)
    - CWE-833 (Deadlock)
    - CWE-400 (Uncontrolled Resource Consumption)
  median_file_ms: 2.3
  total_loc: 1180
```

---

## 9. Decisiones Dinámicas

### 9.1 Repetición de Fase

El orquestador puede repetir una fase si:
- >50% de reglas fallan
- Entropy score alto
- Feedback indica problemas

### 9.2 Saltar Fase

El orquestador puede saltar una fase si:
- <20% de reglas fallan
- Fase ya completada en batch anterior
- Gates ya cumplidos

---

*Documento creado tras grillado*
*Última actualización: 15 de Mayo de 2026*