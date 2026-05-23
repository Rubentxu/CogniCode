# Plan de Actuación — Rules Workflow

## Objetivo

Construir una base de reglas de calidad y seguridad para CogniCode mediante batches
iterativos, usando el workflow agéntico documentado en `.opencode/` y ejecutado con
comandos `/rules-*`.

## Principios operativos

1. **Iterativo por batches.** Cada batch es una unidad de trabajo con estado propio.
   Los batches se procesan secuencialmente o en paralelo según prioridad y recursos.
2. **Sin borrar fracasos.** Si una regla falla, se registra el motivo y se conserva.
   Nunca se borra silencio.
3. **Gates obligatorios.** No se pasa de fase sin cumplir el gate correspondiente.
4. **Artifact store:** Engram como default; `openspec` cuando se necesite
   compartir o versionar.
5. **El estado es la verdad.** `rules/{batch}/state` es la fuente de verdad para
   todo el workflow.

---

## lifecycle de un batch

```
/rules-new <batch>
  ├── rule-knowledge-researcher   → knowledge-research
  ├── rule-concept-normalizer      → concepts
  └── rule-legal-auditor          → legal-review

/rules-ff <batch>
  ├── rule-concept-normalizer      → (si falta)
  ├── rule-legal-auditor          → (si falta)
  ├── rule-designer               → rule-designs
  └── rule-test-engineer          → fixture-matrix, tasks

/rules-apply <batch>
  ├── rule-implementer             → implementation
  ├── rule-test-engineer          → test-report
  └── rule-benchmark-auditor      → benchmark-report

/rules-verify <batch>
  ├── rule-reviewer               → review-report
  └── rule-commit-orchestrator    → commit-plan

/rules-archive <batch>
  └── state → published / deprecated / archived
```

---

## Identidades en el registro

| Identidad | Ejemplo | Propósito |
|---|---|---|
| `concept_id` | `concept/security/sql-injection` | Problema abstracto deduplicado. |
| `candidate_id` | `candidate/semgrep/python.django.sql-injection` | Entrada desde fuente externa. |
| `rule_id` | `CGR-RUST-SEC-001` | Regla publicada en CogniCode. |

---

## Estados del ciclo de vida

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

Cada transición: timestamp, agente, gate aprobado, motivo si hay fallo.

---

## Gates entre fases

| Siguiente fase | Gate requerido |
|---|---|
| `rule-design` | Concept normalizado + procedencia registrada + **legal aprobado** |
| `fixture-matrix` | Diseño con estrategia, exclusiones, coste |
| `implementation` | Legal aprobado + fixtures definidos |
| `benchmark` | Tests unitarios e integración pasan |
| `review` | Benchmark dentro de presupuesto |
| `commit` | Review sin críticos + SARIF/metadata completos |
| `archive` | Commits o PR preparados + KB actualizada |

---

## Métricas por batch

Se calculan después de cada fase y se guardan en `rules/{batch}/state`:

```yaml
batch_metrics:
  discovered: N
  normalized: N
  duplicates: N
  legal_blocked: N
  candidates: N
  designed: N
  implemented: N
  published: N
  implementation_failed: N
  test_failed: N
  benchmark_failed: N
  review_failed: N
  median_file_ms: N.N
  false_positive_rate_estimate: N.N
  cwe_coverage_added: [CWE-...]
```

---

## Fuentes de conocimiento

| Fuente | Tipo de datos | Uso |
|---|---|---|
| Sonar Rules | Metadatos, descripción, ejemplos, remediación | Taxonomía, explicación |
| CodeQL | `@id`, `@kind`, `@precision`, query help | Precisión, seguridad, data-flow |
| Semgrep Registry | YAML con patterns, metadata, tests | Patrones, exclusiones, metavariables |
| SARIF | reportingDescriptor, severidad, fingerprints | Salida interoperable |
| Clippy | Lint metadata, applicability, examples | Reglas Rust, autofix |
| PMD | Reglas XPath/visitor, prioridades, AST | Arquitectura AST, propiedades |
| CWE / OWASP / CERT | Debilidad, impacto, mitigación | Taxonomía seguridad |
| GitHub Code Scanning | Reglas activas en repositorios OSS | Candidatos adicionales |

---

## Categorías de reglas por dominio

### Seguridad
- Injection (SQL, command, LDAP, XML, XSS, path traversal)
- Crypto (weak hash, weak cipher, hardcoded secret, insecure TLS)
- Access control (IDOR, broken authentication, insecure permissions)
- Memory safety (use after free, buffer overflow, uninitialized memory)

### Bug detection
- Error handling ( swallowed exception, missing error prop, incorrect error type)
- Null/None handling ( dereference, unwrap on None, optional chain)
- Logic errors (off-by-one, incorrect operator, redundant condition)
- Concurrency ( race condition, dead lock, incorrect mutex usage)

### Maintainability
- Complexity ( function length, cyclomatic complexity, nesting depth)
- Duplication ( exact, near-miss, semantic clone)
- Dead code ( unused function, unused import, unreachable code)
- Naming ( misleading name, inconsistent convention)

### Performance
- Allocation ( unnecessary clone, inefficient collection, excessive allocation)
- Algorithmic ( O(n²) in loops, missing index, inefficient search)
- Memory ( memory leak, excessive memory usage, missing drop)

### Style
- Formatting ( trailing whitespace, line length, indentation)
- Documentation ( missing doc, incomplete doc, wrong style)
- Naming ( snake_case, camelCase, screaming snake case)

---

## Priorización de batches

Orden recomendado para procesar:

1. **Bug detection de alto impacto** — errores que producen crashes o comportamiento
   incorrecto. Fáciles de validar con tests.
2. **Seguridad crítica** — CWE Top 25 o OWASP Top 10. Alto valor para usuarios.
3. **Error handling** — swallowed exceptions, incorrect error propagation. Muy
   frecuente en code reviews.
4. **Maintainability** — complexity, dead code, duplication. Impacto en deuda
   técnica.
5. **Style y naming** — menor prioridad, pero fácil de implementar.
6. **Performance** — requiere benchmarking real, menor prioridad inicial.
7. **Concurrency** — difícil de detectar y reproducir, se hace cuando haya
   demanda real.

---

## Primeros batches recomendados

| Batch | Dominio | Prioridad | Motivo |
|---|---|---|---|
| `security-injection` | Security | 1 | Alto impacto, buena documentación en fuentes |
| `bug-error-handling` | Bug | 2 | Frecuente, fácil de validar |
| `security-crypto-basics` | Security | 3 | CWE críticos, evidencia clara |
| `maintainability-complexity` | Maintainability | 4 | Deuda técnica, cobertura alta |
| `style-naming` | Style | 5 | Bajo riesgo, rápido de implementar |

---

## Formato de topic keys en Engram

```
rules/{batch}/state              — registro operacional
rules/{batch}/knowledge-research  — investigación de fuentes
rules/{batch}/concepts           — conceptos normalizados
rules/{batch}/legal-review       — auditoría legal
rules/{batch}/rule-designs       — diseños técnicos
rules/{batch}/fixture-matrix     — matriz de tests
rules/{batch}/tasks             — tareas de implementación
rules/{batch}/apply-progress    — progreso de implementación
rules/{batch}/test-report       — reporte de tests
rules/{batch}/benchmark-report   — reporte de rendimiento
rules/{batch}/review-report     — reporte de revisión
rules/{batch}/commit-plan       — plan de commits
rules/{batch}/archive-report     — reporte de archivo
```

---

## Formato de retorno de cada fase

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
skill_resolution: injected | fallback-registry | none
```

---

## Reuniones de seguimiento

Cada batch produce un `archive-report` que se revisa en la siguiente sesión:

- Reglas publicadas vs. intentadas.
- Fracasos por tipo: implementation, test, benchmark, review.
- Coverage añadido: CWE, OWASP, Clean Code attributes.
- Tendencia de FP rate y tiempo medio por archivo.
- Próximo batch recomendado basado en prioridad y feedback.

---

## Comando de seguimiento rápido

Para revisar el estado de un batch en cualquier momento:

```
/rules-verify <batch>    — revisa estado, gates y propone commit
/rules-archive <batch>    — cierra y produce métricas finales
/rules-feedback <issue>   — convierte feedback en regression fixture
```

---

## Cómo arrancar un nuevo batch

1. Elegir tema o fuente del próximo batch.
2. Lanzar `/rules-new <batch>`.
3. El workflow ejecuta research → normalize → legal → design automáticamente.
4. Revisar resultados y decidir si continuar con `/rules-apply <batch>`.
5. Tras implementación, `/rules-verify <batch>`.
6. Cerrar con `/rules-archive <batch>`.
7. Documentar lecciones aprendidas en el archive report.

---

## Notas importantes

- Si el documento `docs/propuestas/workflows-rules/README.md` se quiere
  versionar, requiere `git add -f docs/propuestas/workflows-rules/README.md`.
- Los archivos en `.opencode/` no están ignorados; se pueden trackear.
- El estado en Engram sobrevive entre sesiones pero no tiene historial
  visible. Para auditoría completa, usar `openspec` con archivos.
- Si se pierden las credenciales de Perplexity API, usar contexto 7 u
  otras herramientas de búsqueda de documentación.

---

## Documentación del Flujo Agéntico

Ver documentos específicos en `docs/planes/rules-workflow/`:

| Documento | Descripción |
|-----------|-------------|
| `AGENTIC-FLOW.md` | Visión unificada del sistema agéntico |
| `ORCHESTRATOR.md` | Rol y responsabilidades del orquestador |
| `SUBAGENTS.md` | Especificación de cada subagente |
| `STATE-MACHINE.md` | Estados y transiciones completas |
| `GATES.md` | Gates detallados con criterios de paso |
| `.opencode/skills/cognicode-rules/SKILL.md` | Patrones de reglas (sonar, codeql, semgrep, kiuwan) |
