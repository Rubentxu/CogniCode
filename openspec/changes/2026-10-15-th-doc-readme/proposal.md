# Iniciativa TH-DOC-01 — Completar README del Test Harness

## Intent

Convertir el README del paquete Test Harness en un punto de entrada útil para
contribuidores y revisores, con afirmaciones verificables sobre propósito,
límites, madurez y navegación. El README actual dice correctamente que el
paquete es una especificación, no una implementación, y que los comandos
mostrados son propuestas. La iniciativa debe preservar esa honestidad.

## Goals

| ID | Goal | Criterio de aceptación |
|---|---|---|
| **THD-G1** | Resolver la ruta canónica y el estado de publicación antes de aceptar el README. | Un checkout limpio contiene el README en la ruta acordada; no se considera entregable una copia local ignorada ni se cambia `.gitignore` por inferencia. |
| **THD-G2** | Explicar propósito, audiencia, estado real, límites y autoridad de PipelineK en una lectura inicial. | Una revisión de aceptación distingue claramente especificación de implementación y no atribuye comandos o capacidades inexistentes. |
| **THD-G3** | Ofrecer navegación por tareas con enlaces coherentes a la auditoría, arquitectura, specs, ADRs, contratos, roadmap, UAT y AAT. | Cada enlace relativo resuelve desde la ubicación canónica y el README no duplica las especificaciones normativas. |
| **THD-G4** | Mantener integridad y evidencia del paquete. | Las afirmaciones operativas se contrastan con sus fuentes; el manifiesto refleja el README final; una persona nueva puede encontrar qué leer y qué no está implementado. |

## Scope

### In scope

- Resolver primero la ruta de publicación: hoy `docs/test-harness/` está
  excluido por `.gitignore:153` y sus archivos no pertenecen a `HEAD`.
- Completar `README.md` como mapa de entrada, usando como fuentes la auditoría,
  arquitectura, ontología, specs TH-S01..TH-S06, ADRs, contratos, roadmap, UAT,
  AAT y `PACKAGE-MANIFEST.json`.
- Validar enlaces, estado de los comandos de ejemplo y el hash/tamaño del
  manifiesto cuando el contenido cambie.
- Hacer una aceptación de lectura desde un checkout limpio una vez resuelto
  el gate de publicación.

### Out of scope

- Implementar el ejecutor, selector, ledger, adaptadores, CLI o cambios W0..W7.
- Cambiar PipelineK, scripts, gates, autoridad CI o política de reutilización.
- Forzar al índice el directorio ignorado o modificar `.gitignore` sin una
  decisión explícita de ubicación/publicación.
- Presentar las interfaces propuestas como disponibles o reclamar mejoras de
  velocidad/paridad que no hayan sido medidas.

## Approach

Resolver la publicación como primer work item; después auditar las afirmaciones
y fuentes, ajustar el README sin duplicar los contratos detallados, actualizar
el manifiesto y ejecutar validación de enlaces más una lectura guiada desde un
checkout limpio.

## Quality Intent

- Production surfaces: `docs/test-harness/README.md` o su ruta canónica acordada.
- Changed public APIs: None.
- Readiness dimensions: exactitud, navegabilidad, integridad de manifiesto,
  accesibilidad desde checkout limpio y honestidad sobre estado operativo.
- Required real boundaries: checkout limpio del repositorio; no basta con que
  los enlaces funcionen únicamente en el directorio local ignorado.

## Architecture Impact

- Level: none.
- Evidence: cambio documental solamente; se preserva PipelineK como autoridad.
- Architecture intent: N/A.

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `docs/test-harness/README.md` | Modified, after THD-0 | Punto de entrada cuya ruta canónica debe resolverse primero. |
| `docs/test-harness/PACKAGE-MANIFEST.json` | Modified, if needed | Refrescar el digest/tamaño del README final. |
| `docs/roadmap/ROADMAP.md` | Modified | Puntero ejecutivo a TH-DOC-01. |
| `openspec/changes/2026-10-15-th-doc-readme/` | New | Contrato, goals, tareas y recibos de esta unidad. |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| README inaccesible en un clone porque `docs/*` lo ignora. | High | THD-G1 bloquea aceptación/publicación hasta acordar una ruta incluida en el checkout. |
| Drift entre README, manifiesto y docs de soporte. | Medium | THD-G3/G4 exigen validar enlaces, afirmaciones, hash y tamaño. |
| Comandos propuestos interpretados como implementados. | Medium | Mantener etiqueta explícita y no reportar ejecución hasta superar UAT real. |

## Rollback Plan

Revertir el puntero de TH-DOC-01 en ROADMAP y archivar este OpenSpec como
cancelado. No borrar ni reescribir el README local ignorado; ningún código,
pipeline o dato de ejecución se modifica en esta iniciativa.

## Dependencies

- Mantener accesible el paquete fuente local mientras se decide su ruta
  canónica.
- Decisión de publicación/ubicación antes de cerrar THD-0.
- Los documentos y contratos de soporte deben seguir siendo la fuente
  normativa, no el README.

## Success Criteria

- [ ] THD-G1..G4 satisfechos con evidencia documentada.
- [ ] README disponible desde un checkout limpio en la ruta canónica acordada.
- [ ] Todos los enlaces relativos resuelven y `PACKAGE-MANIFEST.json` coincide
      con el archivo final.
- [ ] La aceptación de lectura confirma que el estado propuesto/implementado
      se entiende sin afirmar capacidades no verificadas.
