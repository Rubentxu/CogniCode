# CURRENT — TH-DOC-01

- **Estado**: CLOSED / COMPLETE para el entregable documental, 2026-10-15.
- **Unidad**: completar y aceptar el README de entrada del Test Harness.
- **Baseline**: branch `worktree/test-harness`, HEAD inicial `efbc2a3ec6114550584766698606e5d4a95e0596`.
- **Commit del paquete canónico**: `9731d9962b1777c0ce468157fc195b6a26984d4a` (`docs(test-harness): publish and complete README`).
- **Ruta**: `test-harness/README.md`, consistente con el README fuente y `installation_subdirectory` del manifiesto. `docs/test-harness/` continúa ignorado e intacto.
- **README/manifiesto**: SHA-256 `7b8b0bd5764de0cd4833018d8fd5e1038c5e8e43f5adcaded9d7e97fc4aef4b7`, 6090 bytes; las 30 entradas del manifiesto pasan.
- **Aceptación desde `git archive`**: paquete 31 archivos; enlaces 20/20; manifiesto 30/30; lectura guiada 6/6 preguntas; `bin/harness.py` ausente y descrito como propuesta.
- **Ratchet**: PASS en HEAD `9731d9962b1777c0ce468157fc195b6a26984d4a` (`workspace=0.101.17`).
- **Fuera de alcance**: implementación y UAT/AAT operativa del harness **NOT_RUN / especificada**, no certificada.
- **Cambios prohibidos evitados**: no se modificaron `.gitignore`, PipelineK, scripts, código ni el staging ignorado.
- **Siguiente acción**: nueva unidad Post-PRF si se autoriza implementar el harness W0..W7.
