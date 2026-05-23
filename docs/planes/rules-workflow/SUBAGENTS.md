# Subagentes — Especificación

> **Fecha**: 15 de Mayo de 2026
> **Versión**: 1.0
> **Estado**: Diseño acordado tras grillado

---

## 1. Visión General

Cada subagente es un especialista que recibe contexto completo del orquestador y retorna reportes breves y estructurados. Los subagentes **no leen artifacts por su cuenta** — reciben todo en el prompt.

---

## 2. rule-knowledge-researcher

### 2.1 Rol

Investigar conocimiento público sobre reglas de calidad/seguridad sin copiar implementaciones propietarias. Guarda procedencia y candidatos raw.

### 2.2 Recibe

- batch o tema
- artifact store
- topic keys relevantes
- fuentes permitidas o categoría objetivo

### 2.3 Pasos

1. Leer `rules/{batch}/state` si existe
2. Buscar conocimiento público: docs, metadatos, taxonomías, ejemplos permitidos
3. Registrar fuente, URL, versión/commit, licencia, fecha y tipo de dato
4. Separar `metadata`, `documentation`, `example`, `pattern`, `test_fixture` e `implementation_reference`
5. Marcar implementación de terceros dudosa como `reference_only`
6. Crear candidatos con `candidate_id` y agrupar por posible `concept_id`
7. Actualizar `rules/{batch}/knowledge-research` y `rules/{batch}/state`

### 2.4 Nunca hagas

- Copiar código propietario
- Convertir implementación externa directamente en regla CogniCode
- Omitir licencia o URL

### 2.5 Retorno

```markdown
status: success|blocked|failed
executive_summary: ...
artifacts: [rules/{batch}/knowledge-research, rules/{batch}/state]
next_recommended: concept-normalization|legal-review|stop
risks: ...
skill_resolution: injected|fallback-registry|fallback-path|none
```

### 2.6 Tools

- `context7_query-docs`
- `context7_resolve-library-id`
- `cognicode_semantic_search`
- `webfetch`

---

## 3. rule-concept-normalizer

### 3.1 Rol

Deduplica candidatos importados y los normaliza en `RuleKnowledge`.

### 3.2 Pasos

1. Leer `rules/{batch}/state` y `rules/{batch}/knowledge-research`
2. Agrupar `candidate_id` por problema abstracto (`concept_id`)
3. Normalizar a estructura `RuleKnowledge`:
   - domain, category, languages, severity
   - precision target, detection strategy
   - tests, performance, Axiom mapping
   - `agent_semantics`
4. Marcar duplicados
5. Actualizar `rules/{batch}/concepts` y `rules/{batch}/state`

### 3.3 Retorno

```markdown
status: success|blocked|failed
executive_summary: ...
artifacts: [rules/{batch}/concepts, rules/{batch}/state]
concepts_created: N
duplicates_found: N
next_recommended: legal-review|design|stop
```

### 3.4 Tools

- `cognicode_build_lightweight_index`
- `cognicode_semantic_search`

---

## 4. rule-legal-auditor

### 4.1 Rol

Audita procedencia y riesgo legal antes de diseño o implementación.

### 4.2 Pasos

1. Leer `rules/{batch}/state`, `rules/{batch}/concepts`, `rules/{batch}/knowledge-research`
2. Verificar licencia de cada fuente
3. Clasificar:
   - `legal_blocked`: no usable por problemas de licencia
   - `reference_only`: usable como referencia, no derivar código
   - `candidate`: usable para derivación
4. Actualizar `rules/{batch}/legal-review` y `rules/{batch}/state`

### 4.3 Retorno

```markdown
status: success|blocked|failed
executive_summary: ...
artifacts: [rules/{batch}/legal-review, rules/{batch}/state]
candidates_approved: N
reference_only: N
legal_blocked: N
next_recommended: design|stop
```

### 4.4 Tools

- `read`
- `webfetch`

---

## 5. rule-designer

### 5.1 Rol

Diseñar reglas CogniCode sin escribir implementación todavía.

### 5.2 Pasos

1. Leer `rules/{batch}/state`, `rules/{batch}/concepts` y `rules/{batch}/legal-review`
2. Para cada candidato viable, diseñar `RuleDesign`:
   - estrategia: regex/token, AST query, visitor, metrics, call graph, data flow
   - nodos AST y queries
   - `RuleContext` necesario (`symbol_table`, `graph`, `metrics`)
   - `layer()` y `required_keywords()`
   - exclusiones y falsos positivos conocidos
   - coste esperado y presupuesto
   - metadata Axiom completa
   - `Issue` enrichment
   - SARIF mapping
   - `agent_semantics` y fix playbook
3. Marcar `designed` o `redesign_required`
4. Guardar `rules/{batch}/rule-designs` y actualizar `state`

### 5.3 Gate

- No diseñar candidatos `legal_blocked`
- Para `reference_only`, no derivar código; reformular desde el problema abstracto
- ** Requiere legal aprobado antes de diseñar**

### 5.4 Multi-lenguaje

El diseño es agnóstico + personalizado por lenguaje:
- Patrón AST genérico cuando sea posible
- Variantes por lenguaje cuando la abstracción no funcione
- El implementador adapta según el contexto proporcionado

### 5.5 Retorno

Incluye diseños aprobados, descartados, riesgos de precisión/performance y tareas recomendadas.

### 5.6 Tools

- `cognicode_analyze_impact`
- `cognicode_build_graph`
- `cognicode_get_call_hierarchy`
- `cognicode_get_complexity`
- `cognicode_query_symbol_index`
- `cognicode_semantic_search`

---

## 6. rule-test-engineer

### 6.1 Rol

Crear matriz de fixtures y tests para reglas.

### 6.2 Pasos

1. Leer `rules/{batch}/state`, `rules/{batch}/rule-designs`
2. Para cada regla diseñada:
   - Positivos: casos que deben matchear
   - Negativos: casos que no deben matchear
   - Edge cases: archivos vacíos, sintaxis parcial, macros, etc.
   - Falsos positivos conocidos
   - Performance fixture
3. Generar tests usando `#[test_rule]` de `02-rules-as-code.md`
4. Guardar `rules/{batch}/fixture-matrix` y `rules/{batch}/tasks`

### 6.3 Mínimo por Regla

- 2+ positivos
- 2+ negativos
- 1+ edge case
- 1+ falso positivo conocido
- 1+ performance fixture

### 6.4 Retorno

```markdown
status: success|failed|blocked
executive_summary: ...
artifacts: [rules/{batch}/fixture-matrix, rules/{batch}/tasks]
rules_with_fixtures: N
tests_created: N
next_recommended: implementation|stop
```

### 6.5 Tools

- `cognicode_find_usages`
- `cognicode_get_symbol_code`

---

## 7. rule-implementer

### 7.1 Rol

Implementar reglas CogniCode desde diseños aprobados y fixtures listos.

### 7.2 Reglas Duras

- No implementar sin `legal-review` aprobado y `fixture-matrix` existente
- Usar AST/query/visitor antes que regex para patrones estructurales
- Reutilizar `RuleContext`; no reparses archivos
- Usar `Issue::from_node` cuando exista nodo primario
- Definir metadata Axiom completa, `layer()` y `required_keywords()`
- Compilar queries/regex una sola vez con `OnceLock`/`LazyLock`
- Actualizar el registro operacional antes de devolver

### 7.3 Pasos

1. Leer `state`, `rule-designs`, `fixture-matrix`, `tasks` y progreso previo
2. Validar impacto con CogniCode cuando se cambien símbolos existentes
3. Implementar tareas pendientes
4. Ejecutar tests focalizados si el orquestador lo permite
5. Marcar `implemented` o `implementation_failed` con motivo
6. Guardar `rules/{batch}/apply-progress` y actualizar `state`

### 7.4 Multi-capa

Una regla puede operar en múltiples capas (0-3):
- Layer 0: preflight con `required_keywords()`
- Layer 1: structural con AST
- Layer 2: semantic con LCPG
- Layer 3: flow con dataflow/taint

### 7.5 Retorno

Incluye archivos modificados, reglas implementadas, tests ejecutados y fallos.

### 7.6 Tools

- `cognicode_analyze_impact`
- `cognicode_build_lightweight_index`
- `cognicode_find_usages`
- `cognicode_get_symbol_code`
- `cognicode-quality_analyze_file`
- `cognicode-quality_check_code_smell`

---

## 8. rule-benchmark-auditor

### 8.1 Rol

Medir rendimiento de reglas, detectar regresiones.

### 8.2 Métricas

Budgets por archivo:

| Tipo | Budget |
|------|--------|
| regex/token | 1 ms |
| AST query | 3 ms |
| visitor | 5 ms |
| metric | 2 ms |
| call graph | 10 ms |
| taint/data-flow | 25 ms |

### 8.3 Pasos

1. Leer `rules/{batch}/state` y `rules/{batch}/apply-progress`
2. Medir tiempo por archivo para cada regla
3. Comparar contra budgets
4. Reportar regresiones
5. Guardar `rules/{batch}/benchmark-report` y actualizar `state`

### 8.4 Decisión

Si excede budget:
- Se marca pero no se descarta
- El orquestador decide si redesign o acepta con explicación

### 8.5 Retorno

Incluye tiempo medido, budget, delta, archivos saltados y recomendación por regla.

### 8.6 Tools

- `cognicode-quality_analyze_project`
- `cognicode-quality_get_quality_diff`

---

## 9. rule-reviewer

### 9.1 Rol

Revisar reglas implementadas contra precisión, metadata, SARIF, calidad y gates.

### 9.2 Checklist

- Legal/procedencia aprobada
- Tests positivos, negativos, edge y FP cubiertos
- Performance dentro de presupuesto
- `Rule` completa metadata UI, Clean Code, qualities, tags, examples
- `Issue` incluye snippet, entidad, scope, variable y remediación cuando aplique
- `agent_semantics`, fix playbook y review questions presentes
- SARIF descriptor y fingerprints estables
- No duplicado de regla existente

### 9.3 Pasos

1. Leer `state`, `apply-progress`, `test-report` y `benchmark-report`
2. Ejecutar análisis de calidad si procede
3. Clasificar hallazgos: CRITICAL, WARNING, SUGGESTION
4. Marcar `approved` o `review_failed` por regla
5. Guardar `rules/{batch}/review-report` y actualizar `state`

### 9.4 Retorno

Incluye decisión por regla y bloqueos exactos.

### 9.5 Tools

- `cognicode_check_architecture`
- `cognicode_find_usages`
- `cognicode-quality_analyze_project`
- `cognicode-quality_run_quality_gate`

---

## 10. rule-commit-orchestrator

### 10.1 Rol

Planificar commits coherentes para lotes de reglas aprobadas.

### 10.2 Pasos

1. Leer `state`, `review-report`, `benchmark-report` y `apply-progress`
2. Revisar `git status`, diff y commits recientes
3. Agrupar cambios por unidad revisable:
   - infraestructura
   - familia de reglas
   - fixtures/tests compartidos
   - docs/catálogo
   - feedback fixes
4. Advertir si hay secretos o archivos ignorados que requieren `git add -f`
5. Preparar mensajes de commit y resumen PR
6. Guardar `rules/{batch}/commit-plan` y actualizar `state` a `committed` solo si realmente se creó commit

### 10.3 No hace commits

No hace commit a menos que el usuario lo pida explícitamente.

### 10.4 Retorno

Incluye plan de commits, archivos por commit, riesgos y comandos sugeridos.

### 10.5 Tools

- `bash` (git status, diff, log)
- `read`

---

## 11. Tabla Resumen de Tools

| Agente | Tools principales |
|--------|-------------------|
| `rule-knowledge-researcher` | context7_query-docs, webfetch |
| `rule-concept-normalizer` | cognicode_build_lightweight_index, cognicode_semantic_search |
| `rule-legal-auditor` | read, webfetch |
| `rule-designer` | cognicode_analyze_impact, cognicode_build_graph, cognicode_get_complexity |
| `rule-test-engineer` | cognicode_find_usages, cognicode_get_symbol_code |
| `rule-implementer` | cognicode_analyze_impact, cognicode_find_usages, cognicode-quality_analyze_file |
| `rule-benchmark-auditor` | cognicode-quality_analyze_project, cognicode-quality_get_quality_diff |
| `rule-reviewer` | cognicode_check_architecture, cognicode-quality_run_quality_gate |
| `rule-commit-orchestrator` | bash (git) |

---

*Documento creado tras grillado*
*Última actualización: 15 de Mayo de 2026*