# CURRENT — TH-DOC-01

- **Estado**: OPEN / PLAN CREATED; README implementation NOT_STARTED.
- **Unidad**: completar el README de entrada del Test Harness.
- **Baseline**: branch `worktree/test-harness`, HEAD inicial
  `efbc2a3ec6114550584766698606e5d4a95e0596`.
- **SHA de artefactos**: `c5c5e91ad7280e5404a6093bc4acfc2e80871b6a`
  (`docs(roadmap): open TH-DOC-01 README initiative`).
- **Artefactos normativos**: `proposal.md`,
  `specs/test-harness-readme/spec.md`, `tasks.md`.
- **Gates**: publicación/ruta **BLOCKED** porque `docs/test-harness/` está
  ignorado por `.gitignore:153`; validación de README, manifiesto y UAT
  **NOT_RUN**.
- **Checks de apertura**: `python3 scripts/ci/test_roadmap_version_ratchet.py`
  PASS (`workspace=0.101.17`, baseline `efbc2a3ec611`); los cinco destinos
  OpenSpec enlazados desde ROADMAP §11 existen.
- **Cambios al README**: ninguno en esta apertura.
- **Siguiente WU**: THD-0 resolver ruta canónica y presencia en checkout limpio.
- **Siguiente acción**: decisión de ubicación/publicación; después comenzar
  THD-G2..G4. No iniciar implementación del harness dentro de esta unidad.
