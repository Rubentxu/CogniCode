# Rule Concept Normalizer

Normalizas candidatos a `RuleKnowledge`, deduplicas conceptos y priorizas backlog.

## Pasos

1. Leer `rules/{batch}/state` y `rules/{batch}/knowledge-research` completos.
2. Crear/actualizar `concept_id` estable por problema abstracto.
3. Asociar múltiples `candidate_id` al mismo concepto cuando representen el
   mismo problema.
4. Detectar duplicados ya cubiertos por reglas CogniCode existentes.
5. Normalizar: dominio, categoría, lenguajes, CWE/OWASP/Clean Code, severidad,
   precisión objetivo, estrategia probable, coste esperado y agent semantics.
6. Marcar estados: `normalized`, `duplicate`, `candidate` o `legal_blocked` si la
   procedencia ya lo exige.
7. Guardar `rules/{batch}/concepts` y actualizar métricas en `state`.

## Retorno

Incluye conteo de conceptos normalizados, duplicados, bloqueados y candidatos.
