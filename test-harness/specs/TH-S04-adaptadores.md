# TH-S04 — Adaptadores de ejecutores y paridad de cobertura

**Estado:** PROPOSED. **Dueño:** `test-harness/adapters`.

## Principio de diseño

El orquestador de pruebas no conoce la sintaxis interna de cada runner. Recibe una `SuiteSpec`/`ExecutionProfile` y utiliza un adaptador para ejecutar y recuperar `RunObservation` tipado. El runner no decide si un resultado antiguo es reutilizable.

## Matrícula de adaptadores

| Adaptador | Superficie vigente | Reglas |
|---|---|---|
| `cargo` | `cargo test`, `cargo clippy`, `cargo fmt`, `cargo metadata`, `cargo llvm-cov` | No cambiar perfiles globales ni `CARGO_TARGET_DIR`; conservar features, doctests y casos especiales |
| `nextest` opt-in | `cargo nextest run`, grabación/reanudación | Verificar compatibilidad y *case parity*; nunca cachear sólo porque `-R` omitió tests; no pretende ejecutar doctests |
| `contracts` | `scripts/ci/run-all-contracts.sh` + `scripts/run_contract_tests.py` | Descubrimiento por glob `test_*.py`; vacío es error; tipos de resultado homogéneos; no listas manuales |
| `baseline` | `scripts/check_known_failures.py` + `scripts/known_failures.yaml` | Aceptar salida de la MISMA ejecución exacta si es compatible; baseline actualizado exige nueva verificación |
| `sandbox` | `sandbox/scripts/run_campaign.sh`, `crates/cognicode-sandbox/src/main.rs`, `sandbox/manifests/*.yaml` | Id escenario, binario probado, fixtures, imagen Podman, cleanup, estados; no reciclar estado exterior sin prueba |
| `shell` | `scripts/ci/*.sh`, reportes de seguridad, supply chain | Preservar argv, cwd, timeout, stderr/stdout separados y exit real; no `|| true` no declarado |
| `pipelinek` | `*.pipeline.kts` que ejecutan gates | Invoca adaptador y verifica resultado; no nueva autoridad; test de wiring |

## Reglas Cargo y Nextest

1. Los tests unitarios/integración continúan dentro de `crates/`; el harness nunca recompone `Cargo.toml` sólo para centralizarlos.
2. `cargo test --workspace --all-targets` cubre binarios/tests/benches/examples que correspondan, **no doctests**. Mantener doctests cuando la lane los exige.
3. `cargo test --workspace` sin targets considera doc-tests. Evitar un `--doc` idéntico posterior si queda demostrado que el target y perfil no cambian; comparar primero el inventario real.
4. La diferencia `--features evidence-kernel`, `--all-features`, `--no-default-features`, `--test-threads=1` es semántica y requiere perfiles distintos.
5. Nextest `-R <run-id>` sólo tras configurar recording y ligar la grabación al `SuiteSpec` y al fingerprint. En primer hito: pruebas piloto, no default global.
6. Si cambian `cargo test` y `nextest` en timings, orden y procesos, el plan debe conservar tests sensibles al estado en adaptador Cargo hasta demostrar equivalencia de comportamiento.
7. Build de release binario consumido por test exige procedencia del artefacto y arquitectura de destino; discovery del target via `scripts/ci/target-dir.sh` o `cargo metadata`.
8. Capturar `started_at/finished_at`, exit code, número y nombres de tests descubiertos/ejecutados, failures, skipped, `stdout/stderr` por separado y logs con sanitización.
9. Paralelizar sólo suites *resource independent*, no tests que mutan env, DB o repos compartidos. `ResourceLock` declarado y probado.

## Sandbox

- Refactor gradual: el orquestador que prepara repos y Podman permanece disponible detrás del adaptador, no duplicar runtime de contenedores.
- `sandbox run` comprueba `result.json` por escenario y su checksum; un worker que sale rc=0 sin resultados de algunos escenarios produce `MissingEvidence`, no PASS.
- La identidad incluye digest del binario de orquestador, binario `cognicode-mcp`, manifiesto, fixture, fuente de repo externa o commit pín, imagen, variables allowlisted, y prueba de cleanup.
- **No reutilizar** suites de red, plugins externos, perf/stability o retry flakiness sin permiso de política: `AlwaysFresh` o TTL justificado.
- `sandbox/scripts/run_campaign.sh` debe propagar exit status de worker/caso. Si se ejecutó todo pero falló algo: `COMPLETED_WITH_FAILURES` y exit!=0. Si se abortó: `INCOMPLETE`.
- Reducir copias de fixtures y `npm install` por escenario mediante snapshots de solo lectura y restauración CoW donde sea seguro; UAT prueba independencia contra mutación del fixture por una prueba.

## Paridad

Por cada adaptación emitir *parity receipt*: `{legacy_command, new_command, discovered, run, passed, failed, ignored, features, env, exceptions}`. Bloquear migración si falta una prueba o existe diferencia de estado no explicada.

## Mutaciones

La suite de contratos debe matar: perder `--features evidence-kernel`, omitir doctests, perder `--test-threads=1`, no propagar exit de subprocess, ignorar fallo de worker, perder `--bin`, perder filename con espacios y ejecutar un wrapper obsoleto.
