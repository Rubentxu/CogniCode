# Proposal — Value Delivery Harness + Functional Core (VDH)

**Status: PROPOSED.** OpenSpec `spec-driven`; no `apply` performed. Backlog owner: CogniCode roadmap Post-PRF (`docs/roadmap/ROADMAP.md`). Cross-links: `docs/roadmap/initiatives/VDH-2026-10/ROADMAP-ADDENDUM.md`, `AGENTS.md`.

## Problema observable

1. `moldql/compile.rs::run_petgraph_plan` responde `Ok(total=0, items=[])` sin consulta. `views.rs` transforma errores de `QualityStore` en `[]`/`default` y `None(GraphQueryPort)` en cero. Una respuesta sintácticamente válida no tiene por qué ser conocimiento válido.
2. `scripts/explorer/common.sh` exige PostgreSQL aunque el runtime `explorer-api` usa LadybugDB. Las suites Playwright de Explorer arrancan `dev:mock`; pueden aprobar mientras el backend real está roto.
3. `AnalysisService`, handlers, views y sandbox contienen efectos y decisiones mezclados y concentraciones de responsabilidades. Ya existen puertos y mejoras ST-01/02: extender **solo los seams que demuestren valor**.
4. UAT/E2E/AAT de campaña quedan acumulados y reejecutan el mundo; hay además código viejo de Dashboard y scripts de crates archivados. Reducir tiempo a feedback y coste de mantenimiento sin retirar contratos de seguridad/compatibilidad.
5. v0.101.17 en workspace vs latest v0.101.16 (3 assets) en snapshot 2026-10-15: falta consumer proof de publicación, independientemente del estado de PR.

## Resultados deseados

- Estados explícitos y representaciones duraderas con origen/snapshot/cobertura, sin engaños `Unknown -> 0/OK`.
- Núcleo de transformaciones puras, I/O en adaptadores y composición; DTOS/ADTs no dependientes de MCP/HTTP.
- Al menos una vertical Explorer real desde fuente→LadybugDB→API→UI, verificable desde distribución.
- Harness orquestado y selectivo: UAT/E2E/AAT efímeros por ciclo (borrados antes de merge), contratos de regresión permanentes, inventorios/recibos mínimos y autoridad de gates única.
- Cada bloque culmina con release consumible verificada y merge remoto.

## No objetivos

No reabrir PRF; no construir motor de grafos paralelo; no mover todos los tests unitarios a un directorio arbitrario; no borrar 106 archivos sin análisis; no convertir TypeScript/Rust en Haskell literal; no garantizar pureza formal desde el compilador Rust; no sustituir `docs/roadmap/ROADMAP.md` como autoridad.

## Impacto y compatibilidad

- Código: `crates/cognicode-core`, `cognicode-explorer`, `cognicode-runtime`, `cognicode-mcp`, `apps/explorer-ui`, `scripts/explorer`, pipelines PipelineK, `test-harness/`.
- Si cambian output schemas públicos, preservar shape/versioning o ejecutar política PRF-MCP-07 antes del release.
- Efectos principales: semántica observable (revisión y error), backends reales en pruebas, menor duplicación de oráculos.
- Rollback: revert de implementación por hito (sin reescribir historia), evitar migraciones destructivas de almacenamiento y preservar releases previas; probar instalación/rollback.

## Decisiones por ratificar

ADR-VDH-001 (resultados semánticos), ADR-VDH-002 (functional core + ports), ADR-VDH-003 (consumer proof), ADR-VDH-004 (efimeridad y gate). El agente puede preparar PR y evidencia, **no declarar ACCEPTED por sí mismo** si el proyecto exige decisión humana.
