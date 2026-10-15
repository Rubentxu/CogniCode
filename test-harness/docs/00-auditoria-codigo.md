# Auditoría de implementación existente — 2026-10-15

Revisión de código publicado en `Rubentxu/CogniCode` (rama por defecto), NO benchmark local. Confirmar SHA de `main` y reconciliar cambios antes de implementar; la siguiente tabla es un punto de partida verificable, no estado inmutable.

| Id | Evidencia concreta | Riesgo / intervención |
|---|---|---|
| A01 | `scripts/test-all.sh` nombra `cognicode-axiom` y `cognicode-quality`; `Cargo.toml` los declara *parked* | Entrada histórica no puede representar la batería vigente. Reemplazar por shim explícito o retirar tras auditar consumidores. |
| A02 | `scripts/test-full.sh` corre fmt, clippy, `cargo test --workspace`, `cargo test --workspace --doc` y `check_known_failures.py` sin persistencia | No reanuda; detecta solapes. El segundo `--doc` repite doctests ya considerados por el `--workspace` sin especificar targets. Demostrar cobertura antes de cambiar. |
| A03 | `scripts/test-fast.sh` detecta paths respecto a HEAD; en checkout limpio usa `HEAD~1` | Diff de rama y ficheros no rastreados insuficientes; seleccionar con base explícita y estado del trabajo. |
| A04 | `scripts/ci/select-suites.sh` agrupa crates con reglas por path; desconocido/fallback; `merge-gate.pipeline.kts` invoca selector con `PATHS="scripts/ci/run-all-contracts.sh"` | El ejemplo no sustituye al uso real del selector en la lane; mantener su test de contrato y conectar planificación verdadera. |
| A05 | `scripts/check_known_failures.py` ejecuta otro `cargo test` para comparar lista de fallos | Evitar duplicado mediante resultado tipado de una ejecución equivalente; comparar también tests ausentes, incompletos y error de harness. |
| A06 | `integration.pipeline.kts`: `all-targets`, `cli-serial`, `release-binary-suite` y matriz separada de features | Solape material de objetivos; las variantes de features y la serialización necesitan cobertura independiente. |
| A07 | `sandbox/scripts/run_campaign.sh`: `FAILURES` se acumula, pero el camino `REPEAT=1` hace `exit 0` y el multirepeat termina con `_mark_campaign_terminal complete 0` | Falso éxito posible: exigir propagación y diferenciación *completed* vs *passed*. Requiere mutación demostrable. |
| A08 | `sandbox/scripts/run_campaign.sh` fija binario en `target/{release,debug}` pese al resolver `scripts/ci/target-dir.sh` | Binario compartido en `CARGO_TARGET_DIR` alternativo no se descubre. Resolver con única autoridad Cargo. |
| A09 | `sandbox/scripts/run_campaign.sh` fija `public_release_baseline: v0.97.1` / commit `735388d1`, y `orchestrator_version: 0.5.0` | Datos históricos embebidos en la instrumentación; deben derivarse de un catálogo con autoridad/versiones explícitas, nunca ser presentados como actuales sin comprobación. |
| A10 | `scripts/ci/run-all-contracts.sh` descubre `scripts/ci/test_*.py` por glob y ejecuta `scripts/run_contract_tests.py` | Preservar detección automática y fallo si cero; migrar por delegación, no listas duplicadas. |
| A11 | `merge-gate.pipeline.kts` documenta fallos de ejecución con stdout capturado y workaround en `cli-unrestricted-ladybug` | No suponer capacidad nativa de reanudar tras fallo semántico en PipelineK. Separar *retry* de infraestructura de reutilización después de cambiar código. |
| A12 | `scripts/ci/test_serial_env_contract.py` recoge interferencias de tests con env global | Concurrencia máxima indiscriminada es insegura. Aislar clases de tests y permisos de paralelismo. |
| A13 | `AGENTS.md` exige fast/full y el full para merge/tag; `docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md` fija autoridad PipelineK | La reutilización certificadora requiere decisión explícita y contratos nuevos: no puede introducirse como atajo oculto. |
| A14 | `release-candidate.pipeline.kts` construye/valida artefactos y `release.pipeline.kts` sólo publica el candidato | Prohibido reutilizar recibos de pruebas en lugar de comprobar bytes de release e instalación final; no alterar identidad de candidato. |

## Comprobaciones que el implementador medirá ANTES del primer cambio

- `git rev-parse HEAD`, branch, base y `git status --porcelain=v1 -z`.
- `pipelinek validate` de cada lane y tests contractuales que la citan.
- Métricas *wall time*, tiempo de compilación/test por target, CPU, RSS, espera Cargo lock, cache hits en `CARGO_TARGET_DIR` sin alterarlo, número de tests y tasas por gate.
- Nombre de test binario, features, flags, filtros, serialidad y entorno para cada ejecución.
- Tests que figuran en 2+ etapas con el mismo comando **y** la misma clausura de inputs, y tests solo aparentes (cfg feature vacía).
- Reproducción en laboratorio del fallo de campaña: un worker falla y el shell invocante informa rc=0. No convertir observación previa en test PASS si no se repite.

## Fuentes de autoridad (archivos existentes, no inventados)

- `AGENTS.md`; `Cargo.toml`; `scripts/test-{fast,full,all}.sh`;
  `scripts/ci/select-suites.sh`; `scripts/check_known_failures.py`;
  `scripts/ci/run-all-contracts.sh`; `scripts/run_contract_tests.py`;
  `scripts/ci/target-dir.sh`; `scripts/ci/test_serial_env_contract.py`;
  `scripts/ci/test_fast_full_wrappers.py`; `scripts/ci/test_select_suites.py`;
  `sandbox/scripts/run_campaign.sh`; `sandbox/scripts/run_scenario.sh`;
  `sandbox/justfile`; `crates/cognicode-sandbox/src/main.rs`;
  `merge-gate.pipeline.kts`; `integration.pipeline.kts`;
  `certification.pipeline.kts`; `release-candidate.pipeline.kts`;
  `release.pipeline.kts`; `scripts/ci/pipeline_authority.py`;
  `docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md`;
  `docs/roadmap/ROADMAP.md`; `docs/roadmap/MAINTENANCE.md`.

## Qué no hemos demostrado

No se dispone de duración de la batería actual en la máquina del usuario, ni de un inventario de todos los casos realmente ejecutados, ni de velocidad de Nextest sobre este repositorio. Por tanto ninguna cifra de ahorro es un hecho medido.
