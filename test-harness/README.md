# CogniCode — Test Harness Evolutivo, Incremental y Reanudable

**Estado: especificación únicamente.** No incluye un *harness* ejecutable. Los seis comandos de la sección de uso son propuestas, no interfaces disponibles.

AAT sigue siendo especificación. No hay mediciones de velocidad o paridad. La
clase UAT se retiró del repositorio el 2026-10-15 por decisión del operador
(ADR-TH-006): este paquete ya no incluye un plan de UAT.

- **Fecha de auditoría**: 2026-10-15
- **Repositorio destino**: https://github.com/Rubentxu/CogniCode
- **Ubicación del paquete**: `test-harness/` en la raíz del worktree de evolución paralelo de CogniCode.

## Objetivo del diseño

El objetivo propuesto es reducir el tiempo de retroalimentación y evitar volver a ejecutar pruebas previamente superadas **solo si su evidencia continúa siendo válida** después de corregir una falla. La reducción de tiempo todavía no se ha medido.

El diseño busca preservar la certificación exhaustiva, la fiabilidad y la autoridad de **PipelineK**, y reunir la implementación futura del *harness* y su gobierno en un solo subárbol versionado.

## Propuesta: qué centralizar y qué no

| Previsto bajo `test-harness/` cuando se implemente | Permanece en ubicación contractual |
|---|---|
| Documentación, contratos, tests del harness y componentes futuros (planificador, selector, ejecutores, adaptadores y ledger) | Tests Rust junto a sus crates; Cargo manifests; `AGENTS.md`; PipelineK como autoridad CI; pipelines y artefactos de release |
| Futuros adaptadores del sandbox y *drivers* de campañas | Manifiestos y fixtures históricos en `sandbox/` durante la migración, hasta probar compatibilidad |

En esta propuesta, centralizar significaría tener **un punto único de gobierno/ejecución**. No se moverían físicamente tests de crates por estética: Cargo los descubre según su modelo.

Las rutas antiguas solo se eliminarían cuando no quede consumidor y exista una prueba de migración.

## Navega por tarea

| Si necesitas... | Empieza aquí |
|---|---|
| Ver hallazgos y rastrear evidencia del repositorio | [Auditoría de código](docs/00-auditoria-codigo.md) y [rutas y referencias](docs/06-rutas-y-referencias.md). |
| Entender arquitectura, estados o migración | [Arquitectura](docs/01-arquitectura.md), [ontología](docs/02-ontologia-y-estados.md) y [compatibilidad](docs/03-migracion-y-compatibilidad.md). |
| Implementar selección, validez o reanudación | Specs [TH-S01](specs/TH-S01-seleccion.md), [TH-S02](specs/TH-S02-evidencia-invalidez.md) y [TH-S03](specs/TH-S03-ejecucion-resume.md). |
| Integrar adaptadores, gates u operación segura | Specs [TH-S04](specs/TH-S04-adaptadores.md), [TH-S05](specs/TH-S05-gates-release.md) y [TH-S06](specs/TH-S06-operacion-seguridad.md). |
| Consultar decisiones, contratos y entregas previstas | [ADRs](adrs/), [contratos JSON](contracts/README.md), [criterios](docs/04-metrica-y-criterios.md), [decisiones pendientes](docs/05-decision-pendientes.md) y [roadmap](roadmap/ROADMAP.md). |
| Aceptar el diseño o buscar falsos verdes | [AAT](aat/AAT.md). Es un plan/especificación, no un resultado ejecutado. El plan de UAT que acompañaba a este paquete se retiró el 2026-10-15 (ADR-TH-006). |
| Preparar instrucciones para un agente implementador | [Mandato del agente](prompts/MANDATO-AGENTE.md). |

El README sirve de índice. Las reglas normativas viven en las specs y ADRs; este mapa no las reemplaza.

Diagrama: [arquitectura](diagrams/arquitectura.mmd).

## Meta de uso — comandos PROPUESTOS (aún no disponibles)

```shell
python3 test-harness/bin/harness.py plan --lane change --base origin/main
python3 test-harness/bin/harness.py run --lane change
python3 test-harness/bin/harness.py status
python3 test-harness/bin/harness.py resume --run-id <id>
python3 test-harness/bin/harness.py explain --suite core-lib
python3 test-harness/bin/harness.py certify --lane integration --candidate <sha>
```

No afirmar que esos comandos existen hasta superar una verificación real contra el repositorio. `bin/harness.py` es una **propuesta de interfaz**, sustituible si el diseño demuestra más adecuado un binario Rust.

El planificador no ejecuta otro motor CI: PipelineK sigue iniciando y autorizando los gates.

## Propuesta de ubicación de estado de ejecución (no implementada)

La especificación propone guardar el estado fuera del checkout:
`$XDG_STATE_HOME/cognicode-test-harness/<repo-slug>-<repo-id>/` (por defecto
`~/.local/state/cognicode-test-harness/…`). Los resultados derivables
voluminosos podrían ir a `$XDG_CACHE_HOME` con recibos separados. Los worktrees
serían identidades distintas de ejecución; los artefactos solo serían
portables si existe prueba de equivalencia de entrada y entorno.

## Restricciones no negociables

- **Fail-closed:** missing, stale, partial, cancelled, quarantined y unknown **no equivalen** a PASS.
- Un PASS anterior es reutilizable únicamente si la función de validez devuelve `Reusable` y se emite una **justificación trazable**.
- `cargo test --workspace --all-targets` no ejecuta doctests; la matriz debe conservar `--doc` donde corresponda. `cargo test --workspace` (sin targets) sí considera doctests.
- Nextest `-R` rerun experimental **no invalida automáticamente** por cambios de código; nunca constituye por sí solo la prueba de vigencia.
- No sobrescribir la configuración global Cargo, no aplicar `CARGO_BUILD_JOBS` ni `[profile.test]` a todo el workspace sin autorización, y no destruir `CARGO_TARGET_DIR` compartido.
- No introducir GitHub Actions. PipelineK sigue siendo la autoridad vigente en `docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md`.
- Preservar los recibos y los artefactos exactos del candidato release; publicar únicamente bytes verificados. Las instalaciones desde los tarballs finales requieren una instalación real, no una simulada.
- No reabrir PRF histórico; ubicar cada avance en el roadmap Post-PRF existente y conservar la disciplina SDDK de `AGENTS.md`.

## Estado

**Diseño listo para evaluación, implementación del harness pendiente.** La
presencia de estos documentos en Git no demuestra que exista una interfaz
ejecutable, ni certifica ejecución de tests, ahorro de tiempo o paridad. AAT
sigue siendo una especificación.
