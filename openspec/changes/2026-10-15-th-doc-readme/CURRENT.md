# CURRENT — TH-DOC-01

- **Estado**: OPEN / IN PROGRESS; ruta canónica `test-harness/` y README revisados en el worktree.
- **Unidad**: completar y aceptar el README de entrada del Test Harness.
- **Baseline**: branch `worktree/test-harness`, HEAD inicial `efbc2a3ec6114550584766698606e5d4a95e0596`.
- **HEAD antes del commit de publicación**: `a25a1a6e78b8de712a3755e20bc27cdf0d23d59e`.
- **Artefactos normativos**: `proposal.md`, `specs/test-harness-readme/spec.md`, `tasks.md`.
- **Ruta/publicación**: THD-0.1/0.2 completos. Paquete en `test-harness/`; staging `docs/test-harness/` preservado, ignorado e intacto. THD-0.3 **NOT_RUN** hasta validar archivo Git posterior al commit.
- **README/manifiesto**: SHA-256 `7b8b0bd5764de0cd4833018d8fd5e1038c5e8e43f5adcaded9d7e97fc4aef4b7`, 6090 bytes; 30/30 entradas de manifiesto pasan. README fuente ignorado no se modificó.
- **Checks locales**: 20/20 enlaces relativos resuelven; estado/claims y comandos revisados; no existen `test-harness/bin`, `src` ni `bin/harness.py`; PipelineK ADR y merge-gate existen. Ratchet PASS en HEAD `a25a1a6e78b8de712a3755e20bc27cdf0d23d59e`.
- **Gates pendientes**: clean archive, THD-0.3, lectura de onboarding desde checkout limpio y revisión final R1/R3/R5.
- **Siguiente WU**: commit del paquete/punteros y verificación `git archive`; luego ejecutar onboarding documental.
- **Límite**: no implementar el harness ni cambiar `.gitignore`, PipelineK, scripts o código.
