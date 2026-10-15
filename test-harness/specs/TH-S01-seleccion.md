# TH-S01 — Planificación incremental y selección de suites

**Estado:** PROPOSED. **Dueño:** `test-harness/src/planner`, adaptador `git` y Cargo metadata.

## Entrada

`repo_root`, `target_ref`, `base_ref` (para ramas), inventario de tracked/staged/unstaged/untracked, Cargo metadata, selección de features, reglas de inclusión/exclusión, definición de lane y catálogo de obligaciones.

## Contrato

- `plan` construye `ChangeSet` usando **merge-base** de rama y base (no `HEAD~1` como base por defecto); integra working tree y staged **sin perder filenames con espacios** (formatos `-z`). En checkout sin base confiable responde `UnknownChangeSet`, nunca `noop`.
- Descubre dependencias de crates desde `cargo metadata --format-version 1` con features y *reverse dependents*; incluye pruebas co-localizadas, fixtures y scripts de generación. No confiar sólo en una regla `core -> core` si `mcp/cli` consumen core.
- Se preserva el selector CR-08 como oráculo de compatibilidad mientras migra; comparar sus decisiones antes de cambiar la cobertura. Toda ruta desconocida hace *fallback conservador*, no `noop`.
- Un cambio en CI, política, manifest Cargo, lock, toolchain, .cargo, build.rs, proc-macros o catálogo obliga a reevaluación amplia según impacto; no siempre ejecutar **todo** si existe prueba precisa de clausura, pero por defecto ALL.
- Diferenciar `metadata-only` verificado (readme/licencia) de `unknown`. `docs/` puede afectar análisis/fixtures, no asumir noop universal.
- `SelectedSuite` incluye el porqué, inputs y dependientes, riesgos de ambiente, run mode y expected-discovered-tests.
- Diferenciar etapas de **solo compile**, tests, lint, seguridad, cobertura, regresión, E2E externo, publicación y certificación final.

## Política de descubrimiento

**Fuente de verdad de suites:** construir inventario desde comandos de stages PipelineK, `cargo metadata`, `cargo test -- --list` / list-compatible y globs de contrato existentes; declarar sólo metadatos que no sean derivables: restricciones `serial`, `external`, `resource_locks`, `freshness`, `required_for_lane`. Evitar inventario paralelo manual de objetivos, y añadir *parity checker* contra gates vigentes.

## Criterios de aceptación

- Caso X: cambiar una función core usada por MCP => incluir core y suites MCP afectadas (mínimo conservador), aunque la ruta modificada pertenezca sólo a core.
- Caso Y: modificar `crates/cognicode-cli/tests/a015_licenses_gate.rs` => ejecutar cobertura contractual CLI, sin todas las pruebas del sandbox si no hay dependencia.
- Caso Z: branch clean diferente de base => plan no vacío cuando los commits de la rama tocan código.
- Untracked nuevo test => detectarlo; borrado de test => change invalidator y recuento de cobertura, no porcentaje vacío.
- Cualquier indeterminación no puede disminuir obligaciones; plan imprime `fallback_reason`.
- Plan reproducible: misma entrada canonizada => misma salida y orden estable.

## Ejecución de referencia (propuesta)

`harness plan --lane change --base origin/main --format json`; `harness explain --suite <id>`; ninguno existe todavía.

## Tests / mutación

Tests de tabla para diffs múltiples, renombrado, espacios, submódulos, worktrees, symlink, lockfile, cambio Cargo features, branch sin upstream, dependencia transitiva y ruta desconocida; matar mutante que omite dependientes o interpreta diff vacío como noop.
