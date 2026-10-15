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
