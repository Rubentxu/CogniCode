# JOURNAL — TH-DOC-01

## Recibo de apertura — 2026-10-15

- **Baseline**: `efbc2a3ec6114550584766698606e5d4a95e0596` en
  `worktree/test-harness`.
- **Resultado**: iniciativa y goals creados; no se editó
  `docs/test-harness/README.md` ni se modificó SDDK, scripts o pipelines.
- **Base de la iniciativa**: el README local declara el paquete como
  especificación, marca sus comandos como propuestos y deja la implementación
  pendiente. `PACKAGE-MANIFEST.json` registra el README local, pero el árbol
  `docs/test-harness/` está ignorado por `.gitignore:153` y no forma parte de
  `HEAD`.
- **Gates iniciales**: publicación/ruta `BLOCKED`; revisión de contenido final,
  enlaces desde checkout limpio, manifiesto posterior a cambios y UAT
  `NOT_RUN`.
- **Siguiente WU**: THD-0, resolver ruta canónica y su presencia en checkout
  limpio antes de aceptar cambios al README.

## Recibo de validación del plan — 2026-10-15

- `python3 scripts/ci/test_roadmap_version_ratchet.py` → **PASS**
  (`workspace=0.101.17`, snapshot baseline `efbc2a3ec611`). El fallo inicial
  por snapshot de 11 commits de antigüedad quedó resuelto en el puntero.
- Los cinco destinos Markdown del puntero de ROADMAP §11 existen en el
  checkout: proposal, spec, tasks, CURRENT y JOURNAL.
- `git diff --check` → exit 0 para el diff rastreado en ese momento.
- README, manifiesto final y UAT → **NOT_RUN**, porque THD-0 no está resuelto
  y esta unidad solo abre el contrato/goals.
- No se modificó ni se añadió al índice el directorio ignorado
  `docs/test-harness/`.

## Recibo de commit inicial — 2026-10-15

- **Commit de iniciativa/goals**: `c5c5e91ad7280e5404a6093bc4acfc2e80871b6a`
  (`docs(roadmap): open TH-DOC-01 README initiative`).
- `git diff --cached --check` → exit 0; los siete archivos esperados quedaron
  versionados, sin incluir `docs/test-harness/README.md`.
- `python3 scripts/ci/test_roadmap_version_ratchet.py` → **PASS** en HEAD
  `c5c5e91ad728` (snapshot a un ancestro inmediato).
- `git status --short --branch` → limpio después del commit inicial.
- **Siguiente WU**: THD-0, resolver publicación/ruta. La aceptación del README
  permanece bloqueada y `NOT_RUN` hasta que el paquete esté en el checkout.

## Recibo de auditoría de aceptación y trazabilidad — 2026-10-15

- **Baseline auditado**: HEAD `6458044891ef669254fcbe88a4b12f3622f584bb`.
- **Feedback loop de la iniciativa**: la ruta pública de ROADMAP §11 permite
  llegar a los cinco artefactos de propuesta, spec, tareas, estado y journal.
  Comprobación normalizada contra `git ls-files`: 5/5 destinos presentes.
- **Boundary de distribución**: `git archive --format=tar HEAD | tar -tf -`
  incluye ROADMAP y los cinco artefactos OpenSpec, pero cero entradas
  `docs/test-harness/README.md`. Resultado funcional: iniciativa publicada,
  README objetivo ausente en checkout limpio; THD-G1 sigue `BLOCKED`.
- **Ratchet**: `python3 scripts/ci/test_roadmap_version_ratchet.py` → **PASS**
  (`workspace=0.101.17`, HEAD `6458044891ef`).
- **Diagnóstico no autoritativo del paquete local ignorado**: 17 enlaces
  relativos, 0 destinos inexistentes; README SHA-256
  `5909e3d315f59efa7705a4d84ede976f0fa671872563f0096605f8699dcfe26e`, 5658
  bytes; ambos valores coinciden con `PACKAGE-MANIFEST.json`. El directorio
  sigue fuera de `HEAD`, por lo que estas observaciones no son aceptación.
- **Trazabilidad**: tasks.md relaciona R1..R5 con sus tareas, checks y resultado.
  R2 y R4 siguen `NOT_RUN` para aceptación final; R3 y R5 no se aceptan desde
  checkout limpio; onboarding no se intentó.
- **Diagnóstico de enlaces**: un primer parser manual no normalizó `..` y dio
  falsos missing para los cinco enlaces; al normalizar rutas correctamente, la
  comprobación pasó 5/5. No se alteró ningún archivo por ese diagnóstico.
- **No se cambió** README, manifiesto, `.gitignore`, scripts ni PipelineK.
- **Límite de esta evidencia**: valida el entregable solicitado (iniciativa y
  goals) y su visibilidad versionada, no completa el README ni satisface THD-G1.

## Recibo de corrección del snapshot y revalidación — 2026-10-15

- La primera ejecución del ratchet tras el commit `d0d762378691` falló: el
  snapshot `CURRENT.md` quedó con SHA abreviado `fefc64f8`; el test buscó el SHA
  histórico `2df856fce7a5` y rechazó su distancia frente al HEAD.
- Se corrigió la cabecera con SHA completo `d0d7623786918a789cda28cfe7254741f2d5d9b4`
  en el commit `387f788aff11641a3e19576e39f344fbe476aca8`.
- En HEAD `387f788aff11641a3e19576e39f344fbe476aca8`:
  `python3 scripts/ci/test_roadmap_version_ratchet.py` → **PASS**
  (`workspace=0.101.17`).
- En el mismo HEAD, el archivo `git archive` incluyó ROADMAP y los cinco
  artefactos TH-DOC-01; `docs/test-harness/README.md` estuvo ausente. THD-G1
  permanece **BLOCKED**, no es una aceptación fallida atribuible al README.
- No se cambió README, manifiesto, `.gitignore`, scripts ni PipelineK. La batería
  de workspace y el onboarding siguen **NOT_RUN**.

## Recibo de resolución de ruta y publicación local — 2026-10-15

- Baseline: HEAD `a25a1a6e78b8de712a3755e20bc27cdf0d23d59e` en `worktree/test-harness`.
- Evidencia de ruta: README fuente declara `test-harness/` en la raíz del worktree paralelo (líneas 5 y 13); el manifiesto fija `installation_subdirectory: test-harness`.
- El staging ignorado `docs/test-harness/` se mantiene intacto. No se modifica `.gitignore` y no se fuerza al índice.
- `cp -a docs/test-harness test-harness`: 31 archivos de origen y destino; `diff -qr` sin diferencias, cero symlinks. Verificación completa del manifiesto en destino: 30 entradas, 0 mismatch.
- `test-harness/` no existe en HEAD todavía; `git archive`/checkout limpio queda **NOT_RUN** hasta el commit de publicación. THD-0.3 sigue pendiente.

## Validación de contenido del README canónico — 2026-10-15

- README revisado y editado en `test-harness/README.md`, no en el staging ignorado. Se eliminaron referencias obsoletas al ZIP/no-Git, se hicieron explícitos el estado de solo especificación, los comandos propuestos y la ausencia de mediciones, y se convirtió el índice en navegación por tareas.
- Afirmaciones contrastadas con la auditoría, arquitectura y ontología del paquete, ADR-TH-003, UAT/AAT, `AGENTS.md` y `docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md`.
- Evidencia operativa: no existen `test-harness/bin/`, `test-harness/src/` ni `test-harness/bin/harness.py`; `merge-gate.pipeline.kts` existe y `.github/workflows/` está ausente. Los ejemplos permanecen marcados como propuestas.
- Índice local: 20 enlaces relativos, 0 destinos inexistentes.
- Manifiesto canónico: 30/30 entradas válidas, sin archivos no declarados salvo el propio `PACKAGE-MANIFEST.json`. README SHA-256 `4b55bf00047949bbeb79a7c70bf23640bf8b0bc246b48f470aee12443775a028`, 6088 bytes.
- Staging ignorado preservado: `docs/test-harness/README.md` conserva SHA `5909e3d315f59efa7705a4d84ede976f0fa671872563f0096605f8699dcfe26e`, 5658 bytes; `.gitignore` no cambió.
- Estados: exactitud/estado y manifiesto **PASS** a nivel de revisión local; publicación de archivo y enlaces desde checkout limpio **NOT_RUN** hasta commit; onboarding documental **NOT_RUN** hasta ejecutar ese recorrido.
