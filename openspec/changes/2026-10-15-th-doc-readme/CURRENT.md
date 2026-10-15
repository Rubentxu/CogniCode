# CURRENT — TH-DOC-01

- **Estado**: OPEN / PLAN CREATED; README implementation NOT_STARTED.
- **Unidad**: completar el README de entrada del Test Harness.
- **Baseline**: branch `worktree/test-harness`, HEAD inicial
  `efbc2a3ec6114550584766698606e5d4a95e0596`.
- **SHA de artefactos auditados**: `fefc64f883377fb430b370193b7c53c345fc6e84` (`docs(roadmap): trace README initiative acceptance`), incluyendo matriz R1..R5 de tarea, comprobación y estado.
- **Artefactos normativos**: `proposal.md`,
  `specs/test-harness-readme/spec.md`, `tasks.md`.
- **Gates**: publicación/ruta **BLOCKED** porque `docs/test-harness/` está
  ignorado por `.gitignore:153`; aceptación final del README y manifiesto
  publicado **NOT_RUN**; onboarding desde checkout limpio bloqueado.
- **Checks de apertura/auditoría**: ratchet PASS en HEAD
  `387f788aff11641a3e19576e39f344fbe476aca8` (`workspace=0.101.17`); los cinco
  enlaces de ROADMAP §11 resuelven a destinos versionados. `git archive`
  incluye la iniciativa, pero no el README objetivo.
- **Diagnóstico local no autoritativo**: README ignorado con 17/17 enlaces
  relativos resolubles y SHA/bytes coincidentes con su manifiesto. No cierra
  R1..R5 ni certifica aceptación desde un checkout distribuible.
- **Cambios al README**: ninguno.
- **Siguiente WU**: THD-0, resolver ruta canónica y presencia en checkout limpio.
- **Siguiente acción**: decisión de ubicación/publicación; después comenzar
  THD-G2..G4. No iniciar implementación del harness dentro de esta unidad.
