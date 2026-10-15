# TH-S05 — Gates PipelineK, release candidate y política de frescura

**Estado:** PROPOSED. Referencia normativa existente: `docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md` (ACCEPTED). El contenido aquí NO cambia automáticamente ese ADR ni la disciplina `AGENTS.md`.

## Niveles de confianza

| Lane | Objetivo | Reutilización recomendada inicialmente |
|---|---|---|
| `change` | feedback local y TDD | Sí, suites validadas por fingerprint |
| `merge` | conformidad de cambio | Piloto shadow; oficial mantiene gate actual hasta aceptar paridad |
| `integration` | matriz exhaustiva, docs, coverage, E2E y features | Piloto con *receipts*; gate oficial full mientras no se acuerde cambio |
| `certification` | sandbox real Podman, estabilidad, métricas, seguridad | `AlwaysFresh` en casos no deterministas o con estado externo; puede reusar metadata/config verificada |
| `release-candidate` | construir artefactos inmutables y smoke real | Fresh para build/verify/install y todo test que dependa de bytes finales |
| `release` | publicar exactamente lo previamente certificado | Mantener re-verificación de staging y descarga del release publicado, sin rebuild |

## Integración sin big bang

- Las lanes raíz siguen siendo `merge-gate.pipeline.kts`, `integration.pipeline.kts`, `certification.pipeline.kts`, `release-candidate.pipeline.kts`, `release.pipeline.kts`.
- Primero añadir pasos de observación `harness plan --shadow`; no alterar semántica ni reemplazar `cargo test`.
- Después ejecutar las mismas suites desde adaptadores y cotejar resultados en paralelo controlado, sin doble corrida inacabable en el gate cotidiano: registrar muestreo experimental separado de gate.
- Sustituir una etapa únicamente tras prueba de paridad, manteniendo su nombre de obligación en el catálogo; modificación de `scripts/ci/pipeline_authority.py` + tests correspondientes cuando sea necesario.
- `certify` sólo retorna gate oficial Satisfied si `policy.allow_cached_release == true` es una decisión mantenida por ADR nueva, con demostración completa de invalidación y *gates anti-falsos-verdes*.
- Los recibos utilizados por gate deben estar asociados al mismo `candidate_digest` o fuente efectiva, políticas idénticas y entorno permitidos; los snapshots de trabajo sucio NO acreditan candidatos firmados sin vínculo de contenido.
- *No inventar un Green por sumar salidas*: se requiere prueba explícita de presencia de cada stage que el sistema anterior exigía.

## Publicación inmutable

`release-candidate.pipeline.kts`: sigue produciendo `dist/`, `release/`, checksums, manifests y UAT de instalación. `release.pipeline.kts`: sigue leyendo y verificando lo ya construido y validado, nunca lo reconstruye. `test-harness` NO mueve esas carpetas de salida ni sustituye `cognicode-release verify`.

## Cobertura

Mantener inventario por clases, siempre separado:

- sintaxis (`fmt`); lint (`clippy`), licencias/advisories;
- unit, integración por crate, doctests, targets/binarios;
- matriz feature default/no-default/all/evidence-kernel/ladybug;
- regresiones CI-contract y baseline conocido;
- CLI/MCP subprocess, serialización, seguridad read-only, timeouts;
- cobertura medida CR-09 bajo herramienta propia (cache distinto);
- sandbox Podman y campañas repetidas, performance y UAT públicas;
- candidate package/supply chain, validación negativa, publicación y consumo real.

## Gate status propuesto

`SATISFIED`, `UNSATISFIED`, `INCONCLUSIVE`. No usar `SKIPPED` para ocultar un test obligatorio; exceptions requieren ID, motivo, aprobador, fecha de caducidad y cobertura alternativa. Los advisory actuales siguen advisory hasta decisión explícita, pero se contabilizan y muestran.

## Condición de cierre

La release del nuevo harness puede etiquetarse y fusionarse en bloques independientes sin concederle autoridad sobre gate de release. **No** confundir publicación del módulo harness con publicación de una nueva versión producto CogniCode; seguir SemVer y la política actual del repositorio para versionar e integrar.
