# Goals y tareas — TH-DOC-01

La iniciativa sigue abierta hasta que los gates de checkout limpio y aceptación se cierren con evidencia. Las casillas indican tareas, no certifican por sí solas la iniciativa completa.

## THD-G1 — Resolver publicación y ruta canónica

- [x] **THD-0.1** Ruta canónica `test-harness/README.md`, conforme al README local (ubicación raíz del worktree paralelo) y a `PACKAGE-MANIFEST.json` (`installation_subdirectory: test-harness`).
- [x] **THD-0.2** Publicar una copia del paquete en `test-harness/` sin cambiar `.gitignore` ni forzar el staging ignorado `docs/test-harness/`; conservar este último intacto.
- [ ] **THD-0.3** Confirmar que la ruta acordada aparece en un checkout limpio después del commit.

## THD-G2 — Completar un resumen exacto para el lector

- [x] **THD-1.1** Contrastar propósito, consumidores, límites, estado y
  autoridad PipelineK con las fuentes del paquete.
- [x] **THD-1.2** Ajustar README para separar claramente especificación,
  propuestas e implementación disponible.
- [x] **THD-1.3** Evitar duplicar en README las reglas normativas de specs y ADRs.

## THD-G3 — Hacer navegable el paquete

- [x] **THD-2.1** Organizar el índice según tareas del contribuidor/revisor.
- [ ] **THD-2.2** Validar cada enlace relativo desde la ruta canónica acordada y checkout limpio.
- [x] **THD-2.3** Confirmar que auditoría, arquitectura, TH-S01..TH-S06,
  ADRs, contratos, roadmap, UAT y AAT están cubiertos o justificar su omisión.

## THD-G4 — Cerrar integridad y aceptación

- [x] **THD-3.1** Verificar todos los comandos contra la interfaz real; mantener
  los no disponibles como propuestas y no ejecutarlos como si existieran.
- [x] **THD-3.2** Regenerar/verificar digest y tamaño en
  `PACKAGE-MANIFEST.json` después de cualquier edición.
- [ ] **THD-3.3** Ejecutar el recorrido de onboarding en un checkout limpio y
  registrar respuestas y enlaces fallidos.
- [ ] **THD-3.4** Adjuntar el resultado de revisión y cerrar solo con todos los
  criterios requeridos en verde.

## Gates

| Gate | Estado inicial | Evidencia necesaria |
|---|---|---|
| Ruta/publicación en checkout limpio | **BLOCKED** | THD-0.1..0.3 |
| Exactitud y honestidad del estado | NOT_RUN | Revisión contra fuentes + UAT |
| Enlaces y navegación | NOT_RUN | Validación desde la ruta canónica |
| Manifiesto | NOT_RUN | Digest/tamaño calculados sobre el README final |
| Onboarding de contribuidor | NOT_RUN | Recibo del escenario THD-3.3 |

## Trazabilidad requisito → tarea → comprobación

Resultados de auditoría del 2026-10-15 sobre HEAD `6458044891ef`. Los diagnósticos del directorio local ignorado son informativos, no aceptación del paquete publicado.

| Requisito OpenSpec | Tareas | Comprobación observable | Resultado observado |
|---|---|---|---|
| R1 Publicación desde checkout limpio | THD-0.1..0.3 | Revisar `.gitignore`, enumerar `git archive HEAD` y exigir el README en la ruta canónica. | **BLOCKED**: `.gitignore:153` ignora `docs/*`; el archivo no aparece en el archivo limpio. ROADMAP y los cinco artefactos OpenSpec sí aparecen. |
| R2 Estado y capacidades honestos | THD-1.1..1.3, THD-3.1 | Revisar el README final contra fuentes y verificar comandos por la interfaz/UAT real. | **NOT_RUN** para aceptación publicada. El README local se etiqueta como especificación; presenta seis comandos como propuestos y código pendiente. No se ejecutaron comandos propuestos. |
| R3 Navegación trazable | THD-2.1..2.3 | Resolver cada enlace relativo desde la ruta canónica en un checkout limpio y comprobar autoridad/fuente. | **NOT_RUN** para el checkout limpio. Diagnóstico local: 17 enlaces relativos, 0 destinos inexistentes. |
| R4 Integridad del paquete | THD-1.1, THD-3.2 | Contrastar afirmaciones con fuentes y comparar SHA-256/tamaño del README final con el manifiesto. | **NOT_RUN** para el paquete publicado y la auditoría completa de afirmaciones. Diagnóstico local: SHA `5909e3d315f59efa7705a4d84ede976f0fa671872563f0096605f8699dcfe26e` y 5658 bytes coinciden con el manifiesto. |
| R5 Aceptación por un lector nuevo | THD-3.3..3.4 | Completar el recorrido de onboarding desde checkout limpio y registrar respuestas y enlaces. | **BLOCKED / NOT_RUN**: el archivo limpio no contiene el README; no se simuló ni certificó el recorrido. |

No convertir ningún resultado local del directorio ignorado en `PASS` de aceptación. THD-G1 sigue pendiente hasta verificar `test-harness/` desde un archivo de Git posterior al commit.

## Evidencia de resolución de ruta — 2026-10-15

- README local, líneas 5 y 13: declara la instalación bajo `test-harness/` en la raíz del worktree paralelo.
- `PACKAGE-MANIFEST.json`, campo `installation_subdirectory`: `test-harness`.
- `docs/test-harness/` está ignorado por `.gitignore:153`; se conserva como fuente local sin cambios.
- `test-harness/` no existía ni estaba ignorado. Se copió el staging completo, sin enlaces simbólicos: 31 archivos origen y 31 destino, `diff -qr` vacío.

## Validación del README canónico — 2026-10-15

Resultados locales sobre `test-harness/README.md`; la aceptación de checkout limpio se conserva separada hasta el commit y la lectura del archivo Git.

| Requisito | Estado actual | Evidencia observada |
|---|---|---|
| R1 Publicación en checkout limpio | NOT_RUN | Ruta `test-harness/` resuelta y copia preparada; `git archive` se comprobará después del commit. |
| R2 Estado y capacidades honestos | PASS | Revisión del README contra auditoría, arquitectura, ADR-TH-003, UAT/AAT y AGENTS.md. Seis comandos siguen marcados como propuestos; no existen `test-harness/bin/`, `test-harness/src/` ni `test-harness/bin/harness.py`. |
| R3 Navegación trazable | NOT_RUN para checkout limpio | 20/20 enlaces relativos resolubles en el directorio canónico local. Repetir sobre `git archive`. |
| R4 Integridad del paquete | PASS | 30/30 entradas del manifiesto verificadas. README SHA-256 `7b8b0bd5764de0cd4833018d8fd5e1038c5e8e43f5adcaded9d7e97fc4aef4b7`, 6090 bytes. |
| R5 Onboarding de lector nuevo | NOT_RUN | Falta recorrido desde checkout limpio y recibo con respuestas. |

La ausencia de UAT/AAT operativos refleja que el harness aún no está implementado; el onboarding documental sí se intentará desde el paquete archivado.
