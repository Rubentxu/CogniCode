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
| **THD-G1** | Resolver ruta canónica y publicación antes de aceptar el README. | La ruta `test-harness/README.md`, declarada por README y `PACKAGE-MANIFEST.json`, aparece en checkout limpio; `docs/test-harness/` queda como fuente local ignorada, sin force-add ni cambios a `.gitignore`. |
| **THD-G2** | Explicar propósito, audiencia, estado real, límites y autoridad de PipelineK en una lectura inicial. | Una revisión de aceptación distingue claramente especificación de implementación y no atribuye comandos o capacidades inexistentes. |
| **THD-G3** | Ofrecer navegación por tareas con enlaces coherentes a la auditoría, arquitectura, specs, ADRs, contratos, roadmap, UAT y AAT. | Cada enlace relativo resuelve desde la ubicación canónica y el README no duplica las especificaciones normativas. |
| **THD-G4** | Mantener integridad y evidencia del paquete. | Las afirmaciones operativas se contrastan con sus fuentes; el manifiesto refleja el README final; una persona nueva puede encontrar qué leer y qué no está implementado. |

## Scope

### In scope

- Publicar el paquete en `test-harness/` en la raíz de este worktree de evolución. Esa ruta coincide con `installation_subdirectory` del manifiesto y con la ubicación indicada en el README local. Conservar sin cambios el staging ignorado `docs/test-harness/`.
- Completar `test-harness/README.md` como mapa de entrada, usando como fuentes la auditoría, arquitectura, ontología, specs TH-S01..TH-S06, ADRs, contratos, roadmap, UAT, AAT y `PACKAGE-MANIFEST.json`.
- Validar enlaces, estado de los comandos de ejemplo y el hash/tamaño del manifiesto cuando el contenido cambie.
- Hacer una aceptación de lectura desde un checkout limpio una vez que el paquete canónico esté versionado.

### Out of scope

- Implementar el ejecutor, selector, ledger, adaptadores, CLI o cambios W0..W7.
- Cambiar PipelineK, scripts, gates, autoridad CI o política de reutilización.
- Modificar `.gitignore` o forzar al índice el staging ignorado `docs/test-harness/`; el paquete canónico vive bajo `test-harness/`.
- Presentar las interfaces propuestas como disponibles o reclamar mejoras de
  velocidad/paridad que no hayan sido medidas.

## Approach

Copiar sin pérdida el paquete local al destino canónico `test-harness/`; después auditar las afirmaciones y fuentes, ajustar el README sin duplicar los contratos detallados, actualizar el manifiesto y ejecutar validación de enlaces más una lectura guiada desde un checkout limpio.

## Quality Intent

- Production surfaces: `test-harness/README.md`.
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
| `test-harness/README.md` | Modified | README canónico publicado y entrada al paquete. |
| `test-harness/PACKAGE-MANIFEST.json` | Modified, if needed | Refrescar el digest/tamaño del README final. |
| `test-harness/**` | New | Copia versionada de los artefactos del paquete declarados por su manifiesto. |
| `docs/test-harness/**` | Preserved, ignored | Staging local de origen, no modificado ni incluido en el índice. |
| `docs/roadmap/ROADMAP.md` | Modified | Puntero ejecutivo a TH-DOC-01. |
| `openspec/changes/2026-10-15-th-doc-readme/` | New | Contrato, goals, tareas y recibos de esta unidad. |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| README inaccesible en un clone porque `docs/*` lo ignora. | High | Publicar en `test-harness/`, ruta indicada por el manifest y el README, sin alterar la política global de ignore. |
| Drift entre README, manifiesto y docs de soporte. | Medium | THD-G3/G4 exigen validar enlaces, afirmaciones, hash y tamaño. |
| Comandos propuestos interpretados como implementados. | Medium | Mantener etiqueta explícita y no reportar ejecución hasta superar UAT real. |

## Rollback Plan

Revertir el puntero de TH-DOC-01 y retirar únicamente los archivos versionados bajo `test-harness/` añadidos por esta unidad. No borrar ni reescribir el staging local ignorado `docs/test-harness/`; ningún código, pipeline o dato de ejecución se modifica en esta iniciativa.

## Dependencies

- Mantener accesible el staging fuente local `docs/test-harness/` durante la copia y la revisión.
- La ruta publicada se fija en `test-harness/` por `README.md` y el campo `installation_subdirectory` de `PACKAGE-MANIFEST.json`; no se modifica `.gitignore`.
- Los documentos y contratos de soporte deben seguir siendo la fuente
  normativa, no el README.

## Success Criteria

- [x] THD-G1..G4 satisfechos con evidencia documentada en `tasks.md` y `JOURNAL.md`.
- [x] README disponible desde el artefacto distribuible Git en `test-harness/README.md`.
- [x] Los 20 enlaces relativos resuelven y los 30 hashes/tamaños del `PACKAGE-MANIFEST.json` coinciden.
- [x] El recorrido de lectura en el artefacto distribuible distingue diseño propuesto de implementación y no afirma capacidades no verificadas.
