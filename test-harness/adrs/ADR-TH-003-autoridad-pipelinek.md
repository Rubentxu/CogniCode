# ADR-TH-003 — PipelineK sigue siendo la autoridad

- **Estado:** PROPOSED.
- **Decisión:** el harness no crea ni sustituye CI; proporciona `plan/run/resume/status/explain` y resultados verificables que las stages PipelineK consumen. En gate de merge/release PipelineK decide ejecutar/autorizar y comprueba agregados.
- **Fundamento:** `docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md` (ACCEPTED); no se introducen GitHub Actions, Jenkins ni runners remotos como nuevas autoridades.
- **Política:** hasta aceptar una ADR que cambie la semántica de `full` en `AGENTS.md`, el modo `certify` funciona **fresh-full** para publicación. La reutilización de `full` es `opt-in` experimental y NO produce certificación oficial.
- **Compatibilidad:** conservar `merge-gate.pipeline.kts`, `integration.pipeline.kts` y `certification.pipeline.kts` como puertas existentes, adaptando primero sólo shell `sh(...)`. Las pipelines `release-candidate` y `release` conservan etapas que verifican payload real.
- **Prueba:** cortar/alterar una obligación debe provocar gate rojo, incluso si el CLI del harness devuelve JSON bien formado.
