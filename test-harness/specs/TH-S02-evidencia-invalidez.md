# TH-S02 — Recibos, huellas, invalidación y gates

**Estado:** PROPOSED. **Dueño:** `test-harness/src/receipts`, `src/reuse`, `src/aggregate`.

## Autoridad

El recibo es un hecho histórico; `ReuseDecision` es evaluación actual. El gate es agregación de obligaciones con evidencia. Una base de datos con últimos estados mutables **no sustituye** al histórico append-only.

## Input fingerprint mínimo (canonizado)

- Suite id, comando argv real, runner y versión, configuración del runner y `policy_version`;
- Compilación: Rust toolchain (`rustc -vV`), `cargo --version`, targets, features, manifest y `Cargo.lock`, `build.rs`, proc-macros, `RUSTFLAGS` relevantes, plataforma y target;
- Snapshot efectivo de fuente: HEAD + contenido de paths relevantes tracked/staged/unstaged/untracked, modo de archivo, ruta relativa normalizada, identidad Git de submódulos cuando aplican; no persistir secretos ni rutas sensibles no necesarias;
- Test sources, fixtures, scripts, config, golden files, pruebas seleccionadas y recuentos esperados; runtime si depende del tiempo/red/PG/Podman;
- Artefactos de binarios preconstruidos consumidos por tests (`sha256`, target, provenance) y `CARGO_TARGET_DIR` derivado vía `cargo metadata`, no mediante conjeturas;
- External state (host services/version/container image digest) o política `NoReuse` cuando imposible capturarlo.

**Key** = hash SHA-256 de serialización canónica versionada (`schema_version` incluido). Ordenar paths, argumentos y conjuntos determinísticamente; no basarse en mtime. Las huellas son un control de equivalencia de entradas, no una afirmación de ausencia de flakiness.

## Reutilización

```text
reuse(receipt, expected):
  if !receipt.complete OR receipt.status != PASS -> NotReusable
  if !receipt.verify_integrity -> Undetermined
  if receipt.policy_revoked -> Invalidated
  if receipt.suite_id/runner/flags/features/case_set differ -> Invalidated
  if receipt.input_fingerprint != expected -> Invalidated(reasoned diff)
  if suite.freshness == AlwaysFresh OR external state unverified -> Execute
  if lane forbids reuse (e.g. release UAT) -> Execute
  else -> Reusable(receipt.id)
```

## Obligaciones y agregación

- `Requirement` define `suite_id`, `profile`, `case_inventory`, `feature_guard`, `required_evidence`, `freshness`, `mode` y `conformance_references`.
- `GateVerdict Satisfied` exige por cada requirement **exactamente** una evidencia pertinente y válida, sin ocultar fallos actuales ni omisiones. El gate contiene *proof set* y hashes de recibos.
- Los fallos reparados pueden conservarse en historial, pero el gate necesita una ejecución nueva PASS para el mismo conjunto que dejó de estar probado.
- Tests `#[cfg]` que producen 0 tests por feature incorrecta no son verdes; requieren `expected_min_cases`/pin de test inventory.
- `known_failures.yaml` se evalúa sobre los resultados **de esa ejecución**; cambios en baseline invalidan la aceptación. Un test arreglado inesperadamente requiere actualizar baseline por revisión, no rediseñar la regresión como PASS.
- Receipts exportables con *redaction*, sin claves, sin datos de usuarios; logs por content digest y referencias seguras.

## Persistencia

Externa a Git: `$XDG_STATE_HOME/cognicode-test-harness/<repo-id>/` con append-only + escrituras atomicas `fsync+rename` y locking interproceso; política de GC separa ejecuciones auditables de caches derivables. Elegir **un único ledger canónico** en H4 (SQLite transaccional o JSONL append-only, comparados con pruebas de crash/lectura concurrente).

## Criterios negativos

- Cambiar fixture, `Cargo.lock`, perfil de features, serialidad, política o source dirty invalida.
- Un archivo de receipt truncado, un run sin terminal event, log que no concuerda, resultado vacuo, permiso insuficiente o volumen sin evidencia => no PASS.
- `cargo nextest -R` no prueba frescura por código; el validador externo mantiene la autoridad.
- Si cambian exactamente los bytes de un archivo irrelevante, no invalida suites no afectadas cuando la clausura está demostrada.

**Schemas:** [`../contracts/receipt.schema.json`](../contracts/receipt.schema.json), [`../contracts/gate-verdict.schema.json`](../contracts/gate-verdict.schema.json).
