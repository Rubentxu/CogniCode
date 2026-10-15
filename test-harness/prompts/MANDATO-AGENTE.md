# Mandato listo para pasar a agente implementador

> Implementa el evolutivo **CogniCode Test Harness incremental y reanudable** a partir de `test-harness/README.md`, respetando toda la carpeta `test-harness/` y las normas activas de `AGENTS.md` y `docs/roadmap/ROADMAP.md` del repo. Nunca rehagas el roadmap desde cero ni reabras PRF. Sitúate en un worktree paralelo con una rama específica `feat/test-harness-incremental` basada en `origin/main`. Comprueba la identidad Git/SDDK antes de editar; no toques ni sobrescribas cambios ajenos. Si tienes dudas sobre un contrato existente, contrástalo con código y tests, no con un resumen.
>
> **Objetivo**: centralizar la implementación y las especificaciones del sistema de tests bajo `test-harness/`; dejar como únicos restos fuera de él wrappers de compatibilidad o tests Rust obligatoriamente co-localizados en crates, con PipelineK como autoridad CI, y conservar las pipelines de release como fábrica de publicación.
>
> Ejecuta `roadmap/ROADMAP.md` por WUs largas de valor. Cada WU: diagnóstico y tiempos REALES, test RED de caracterización, corrección mínima, test GREEN y mutación/contraprueba, UAT/AAT de la WU, recibos, commit atómico Conventional Commits, PR e integración remota cuando pase gate; cierre con release acorde a políticas del repo y SDDK reconciliado. Si el gate falla, reporta BLOCKED con fallo exacto y retoma sin borrar resultados anteriores; jamás llames DONE a un bloque sin evidencia.
>
> Implementa funciones puras y ADTs para selección, fingerprints, decisiones de reuse y agregación; encapsula efectos Git/Cargo/Nextest/FS/SQLite/Podman/PipelineK en adaptadores. Una única autoridad para catálogos de suites, otra para ledger; no listas duplicadas en shell/Kotlin/Python. Introduce `nextest` solo como adaptador experimental con recording, no como certificador de vigencia. No modificar `~/.cargo/config.toml`, `CARGO_TARGET_DIR` compartido ni introducir GitHub Actions.
>
> **Regla principal**: si un test falla y se corrige, reejecuta el afectado y los dependientes de sus inputs; conserva PASS previos **solamente** cuando las huellas y la política demuestran que siguen siendo válidos. Si no puede demostrarlo, reejecuta. Nunca uses un resultado antiguo para certificar un tarball o instalación real sin verificaciones nuevas.
>
> No cambies `scripts/test-full.sh` ni los gates oficiales de release a modo cacheado hasta certificar paridad de cobertura y obtener aprobación del maintainer para la ADR de política. En migración, conserva `scripts/ci` y los wrappers existentes; no muevas tests Cargo ni rompas discovery. Publica mediciones y `explain` para cada suite reusada o invalidada. Una campaña de sandbox con un worker fallido jamás debe terminar verde.
>
> Prioridad de trabajo W0→W1→W2→W3→W4→W5→W6→W7. Verifica y respeta los paths reales citados en `docs/00-auditoria-codigo.md`, y usa `uat/UAT.md` y `aat/AAT.md` como oráculos, registrando `NOT_RUN` cuando aún no existan. El ZIP son especificaciones, no código ya integrado.

## Handoff esperado por sesión

- Worktree y HEAD base / nuevo SHA; estado SDDK Post-PRF activo.
- WU, contrato y fuente de código modificada.
- RED antes / GREEN después, suite counts, `exit_code` y recibos.
- Medición real y comparación contra W0.
- Qué pruebas siguen siendo válidas y por qué; cuáles están pendientes e invalidadas.
- PR, merge, tag/versión o razón exacta de bloqueo; siguiente WU.
