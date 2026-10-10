# Feature: A-014 — `cognicode capabilities --json`

> **Goal:** Subcommand CLI que devuelve el estado actual de capabilities (tools, profiles, mutating set runtime) en JSON schema-versionado. Habilita el discovery machine-readable para A-033..037 (skills) y para integraciones externas. P1 CP2.4, dep A-003 ya cerrado.

## Estado al inicio

- **HEAD:** `48a8980cc936e6cf6d9b4e529c849fce8e10de83` (rama `docs/cp2-a012-closure`, 5 ahead de origin/main).
- **Tests preexistentes** que cubren artefactos:
  - `crates/cognicode-cli/tests/qw03_bin_tracking_guard.rs` — guard de bin tracking.
  - `crates/cognicode-mcp/tests/a012_structured_output.rs` — output schemas para tools.
  - `crates/cognicode-mcp/tests/a013_lifecycle_uat.rs` — lifecycle UAT.
- **Drift detectado (regla 4):** Ninguno en scope. El `build_graph` authority drift (Lesson 95) está documentado como debt P2 separado.

## Disciplina: Strict TDD

1. **RED** — escribir tests que pine el output shape (schema_version, source_commit, tools array, profiles array, mutating_tools_runtime, exit 0).
2. **GREEN** — implementar el subcommand.
3. **TRIANGULATE** — añadir mutaciones que revelan drift o regresiones:
   - T1: `--json` produce JSON parseable por `serde_json::from_str`.
   - T2: `--json` no imprime nada a stderr (clean stdout).
   - T3: el output es estable entre dos invocaciones consecutivas (excepto `source_commit` que puede cambiar entre commits).
   - T4: `cli_version` coincide con `env!("CARGO_PKG_VERSION")`.
4. **REFACTOR** — solo si necesario.

## Tests a entregar (mínimo)

| # | Test | Tipo | Mutación |
|---|---|---|---|
| 1 | `a014_capabilities_json_emits_v1_schema_with_tools_profiles_runtime` | GREEN baseline | shape completo |
| 2 | `a014_capabilities_json_parses_with_serde_json` | GREEN baseline | JSON valid |
| 3 | `a014_capabilities_json_does_not_write_to_stderr` | RED triangulate T2 | clean stderr |
| 4 | `a014_capabilities_json_carries_cli_version_and_source_commit` | RED triangulate T4 | freshness |
| 5 | `a014_capabilities_json_mutating_tools_runtime_matches_posture` | RED triangulate T5 | contract/runtime invariant |

## Tasks

Ver `todo` projection.

## Out of scope

- Subcommand no-JSON (`--text`); `--json` es el único formato publicado (machine-readable es el acceptance del action register).
- Modificar tests preexistentes (regla 4).
- Consolidar los 3 Session structs paralelos (Lesson 96; fuera de scope).
- Arreglar el `build_graph` authority drift (Lesson 95; P2 deuda separada).
- Promover A-014 a required check separado (decisión de governance).
- Cuestiones de release: SEMVER no bumpea automáticamente (decisión del operador, regla 6).

## Riesgos

- Si el output schema cambia, los consumers (skills, integraciones) se rompen. Mitigación: schema versionado en el campo `schema_version`.
- Si `product/tools.json` o `product/profiles.json` no existen en el checkout, el comando debe fallar limpio. Mitigación: error reportable en JSON con campo `error`.
- Si `source_commit` se computa vía `git` (subprocess), eso añade dependencia. Mitigación: leer `.git/HEAD` directamente o `git rev-parse` (subprocess sí; aceptable para CLI).

## Recibo (al cierre)

| Fecha | Estado | Evidencia |
|---|---|---|
| 2026-10-10 | **DONE en código** | `cargo test -p cognicode-cli --test a014_capabilities_json` → 17/17 verde. `cognicode capabilities --format json` emite schema `cognicode.capabilities/v1` con `cli_version`, `profiles[]` (id, components, description, mutating, install, stability, skill_bundles, evidence), y los 5 RED triangulates (T1..T5) materializados. Wired en `merge-gate.pipeline.kts:cli-a014-capabilities-json`. Caveat en la doc original: el goal doc asumía que `--json` no estaba implementado y proponía añadirlo; inspección del estado real mostró que el subcommand y el formato JSON ya existían (PR #309 mergeado per v1015 §9.4). El test contractual `a014_capabilities_json` pinea el contrato byte-a-byte. Sin código de producción cambiado. Doc-debt cleanup. |