# Goals y tareas — TH-DOC-01

La unidad se cierra con los recibos R1..R5 de abajo. La aceptación documental no certifica ni implementa el harness operativo.

## THD-G1 — Resolver publicación y ruta canónica

- [x] **THD-0.1** Ruta canónica `test-harness/README.md`, conforme al README local (ubicación raíz del worktree paralelo) y a `PACKAGE-MANIFEST.json` (`installation_subdirectory: test-harness`).
- [x] **THD-0.2** Publicar una copia del paquete en `test-harness/` sin cambiar `.gitignore` ni forzar el staging ignorado `docs/test-harness/`; conservar este último intacto.
- [x] **THD-0.3** Confirmar que la ruta acordada aparece en el archivo distribuible de Git posterior al commit.

## THD-G2 — Completar un resumen exacto para el lector

- [x] **THD-1.1** Contrastar propósito, consumidores, límites, estado y
  autoridad PipelineK con las fuentes del paquete.
- [x] **THD-1.2** Ajustar README para separar claramente especificación,
  propuestas e implementación disponible.
- [x] **THD-1.3** Evitar duplicar en README las reglas normativas de specs y ADRs.

## THD-G3 — Hacer navegable el paquete

- [x] **THD-2.1** Organizar el índice según tareas del contribuidor/revisor.
- [x] **THD-2.2** Validar cada enlace relativo desde la ruta canónica en el archivo distribuible de Git.
- [x] **THD-2.3** Confirmar que auditoría, arquitectura, TH-S01..TH-S06,
  ADRs, contratos, roadmap, UAT y AAT están cubiertos o justificar su omisión.

## THD-G4 — Cerrar integridad y aceptación

- [x] **THD-3.1** Verificar todos los comandos contra la interfaz real; mantener
  los no disponibles como propuestas y no ejecutarlos como si existieran.
- [x] **THD-3.2** Regenerar/verificar digest y tamaño en
  `PACKAGE-MANIFEST.json` después de cualquier edición.
- [x] **THD-3.3** Ejecutar el recorrido de onboarding sobre el README del archivo distribuible de Git y registrar respuestas/enlaces.
- [x] **THD-3.4** Adjuntar resultados y cerrar solo con los criterios documentales en verde. La aceptación no cubre la UAT/AAT operativa del harness aún no implementado.

## Gates

| Gate | Estado inicial | Evidencia necesaria |
|---|---|---|
| Ruta/publicación en checkout limpio | **BLOCKED** | THD-0.1..0.3 |
| Exactitud y honestidad del estado | NOT_RUN | Revisión contra fuentes + UAT |
| Enlaces y navegación | NOT_RUN | Validación desde la ruta canónica |
| Manifiesto | NOT_RUN | Digest/tamaño calculados sobre el README final |
| Onboarding de contribuidor | NOT_RUN | Recibo del escenario THD-3.3 |

## Trazabilidad requisito → tarea → comprobación

Recibo inicial del 2026-10-15 sobre HEAD `6458044891ef`; esta tabla conserva el estado antes de resolver la ruta canónica. Los resultados finales están más abajo.

| Requisito OpenSpec | Tareas | Comprobación observable | Resultado observado |
|---|---|---|---|
| R1 Publicación desde checkout limpio | THD-0.1..0.3 | Revisar `.gitignore`, enumerar `git archive HEAD` y exigir el README en la ruta canónica. | **BLOCKED**: `.gitignore:153` ignora `docs/*`; el archivo no aparece en el archivo limpio. ROADMAP y los cinco artefactos OpenSpec sí aparecen. |
| R2 Estado y capacidades honestos | THD-1.1..1.3, THD-3.1 | Revisar el README final contra fuentes y verificar comandos por la interfaz/UAT real. | **NOT_RUN** para aceptación publicada. El README local se etiqueta como especificación; presenta seis comandos como propuestos y código pendiente. No se ejecutaron comandos propuestos. |
| R3 Navegación trazable | THD-2.1..2.3 | Resolver cada enlace relativo desde la ruta canónica en un checkout limpio y comprobar autoridad/fuente. | **NOT_RUN** para el checkout limpio. Diagnóstico local: 17 enlaces relativos, 0 destinos inexistentes. |
| R4 Integridad del paquete | THD-1.1, THD-3.2 | Contrastar afirmaciones con fuentes y comparar SHA-256/tamaño del README final con el manifiesto. | **NOT_RUN** para el paquete publicado y la auditoría completa de afirmaciones. Diagnóstico local: SHA `5909e3d315f59efa7705a4d84ede976f0fa671872563f0096605f8699dcfe26e` y 5658 bytes coinciden con el manifiesto. |
| R5 Aceptación por un lector nuevo | THD-3.3..3.4 | Completar el recorrido de onboarding desde checkout limpio y registrar respuestas y enlaces. | **BLOCKED / NOT_RUN**: el archivo limpio no contiene el README; no se simuló ni certificó el recorrido. |

Los resultados de esta auditoría inicial no se trasladan a la aceptación final. El staging local ignorado no se usó como sustituto del artefacto distribuible.

## Evidencia de resolución de ruta — 2026-10-15

- README local, líneas 5 y 13: declara la instalación bajo `test-harness/` en la raíz del worktree paralelo.
- `PACKAGE-MANIFEST.json`, campo `installation_subdirectory`: `test-harness`.
- `docs/test-harness/` está ignorado por `.gitignore:153`; se conserva como fuente local sin cambios.
- `test-harness/` no existía ni estaba ignorado. Se copió el staging completo, sin enlaces simbólicos: 31 archivos origen y 31 destino, `diff -qr` vacío.

## Validación del README canónico — 2026-10-15

Resultados finales sobre el artefacto de `git archive` en HEAD `9731d9962b1777c0ce468157fc195b6a26984d4a`.

| Requisito | Estado actual | Evidencia observada |
|---|---|---|
| R1 Publicación en checkout limpio | PASS | `git archive HEAD -- test-harness` contiene `test-harness/README.md` y todos los archivos del paquete. |
| R2 Estado y capacidades honestos | PASS | Revisión del README contra auditoría, arquitectura, ADR-TH-003, UAT/AAT y AGENTS.md. Seis comandos siguen marcados como propuestos; no existen `test-harness/bin/`, `test-harness/src/` ni `test-harness/bin/harness.py`. |
| R3 Navegación trazable | PASS | 20/20 enlaces relativos resuelven a archivos o directorios incluidos en el archivo distribuible. |
| R4 Integridad del paquete | PASS | 30/30 entradas del manifiesto verificadas desde el archivo Git. README SHA-256 `7b8b0bd5764de0cd4833018d8fd5e1038c5e8e43f5adcaded9d7e97fc4aef4b7`, 6090 bytes. |
| R5 Onboarding de lector nuevo | PASS | Recorrido del README extraído desde `git archive`: respuestas correctas a 6/6 preguntas de propósito, estado, navegación, comandos, autoridad y aceptación. Es lectura guiada del agente, no UAT humana independiente. |

La UAT/AAT operativa del harness permanece **NOT_RUN / especificada**, ya que no se implementa el harness en esta unidad. El onboarding documental sí fue aceptado sobre el artefacto del archivo Git.
