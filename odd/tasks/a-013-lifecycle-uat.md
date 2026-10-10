# Feature: A-013 — Black-box lifecycle UAT (startup→shutdown PASS)

> **Goal:** Cubrir el ciclo de vida end-to-end del binario `cognicode-mcp` con un test contractual strict TDD que pine startup → tools/list → call read-only → shutdown limpio. P0 CP2.7, dep A-009 ya cerrado. Bloquea A-024 (MCPB), A-026 (OCI/GHCR), A-041 (Public Beta).

## Estado al inicio

- **HEAD:** `dbea6fdf6b648b0e42638f21dd1ed722ade792d5` (rama `docs/cp2-a012-closure`, 3 ahead de origin/main).
- **Tests preexistentes** que cubren piezas del lifecycle (no el contrato end-to-end):
  - `prf_sec_05_shutdown_recovery_uat.rs` — shutdown + corrupt cache.
  - `prf_f5_w3_signal_cancel.rs` — SIGINT/SIGTERM.
  - `a009_agent_safe_profile.rs` — read-only profile.
  - `prf_cli_04_two_process_uat.rs` — two-process handshake.
  - `prf_mcp_03_network_off_uat.rs` — sin red.
- **Drift detectado (regla 4):** 3 structs `Session` paralelos:
  - `common::McpSession` (canónico, McpSession).
  - `a009::Session` (específico, con read-only).
  - `prf_sec_05::Session` (variante con Option<ChildStdin>).
  - Scope de A-013 = NO consolidarlos. Registrar el drift como descubrimiento y seguir usando el canónico.

## Disciplina: Strict TDD

1. **RED explícito** — escribir cada test que pina un aspecto del lifecycle; si el binario lo satisface ya, el test es GREEN desde el inicio (eso es OK siempre que la mutación esté bien plantada y el test tenga poder de detección).
2. **GREEN** — el test pasa; commit atómico.
3. **TRIANGULATE** — añadir mutaciones que revelan gaps: workspace inválido, signal externo, etc.
4. **REFACTOR** — solo si las triangulaciones revelan duplicación o código mejorable.

## Tests a entregar (mínimo)

| # | Test | Tipo | Mutación |
|---|---|---|---|
| 1 | `a013_lifecycle_uat_startup_returns_tools_list_with_canonical_73_tools` | GREEN baseline | real repo |
| 2 | `a013_lifecycle_uat_call_readonly_tool_mid_lifecycle` | GREEN baseline | call `tree_query` o `graph_query` |
| 3 | `a013_lifecycle_uat_shutdown_via_stdin_eof_exits_cleanly` | GREEN baseline | stdin EOF, wait, no panic |
| 4 | `a013_lifecycle_uat_invalid_workspace_reports_clean_error` | RED triangulate T1 | workspace que no existe |
| 5 | `a013_lifecycle_uat_readonly_rejects_mutating_call_mid_lifecycle` | RED triangulate T2 | `--read-only` + call mutating |
| 6 | `a013_lifecycle_uat_signal_term_shuts_down_cleanly` | RED triangulate T3 | SIGTERM al proceso |

## Tasks

Ver `todo` projection.

## Out of scope

- Consolidar los 3 structs `Session` en uno (scope creep).
- Modificar tests preexistentes (regla 4).
- Promover A-013 a required check separado del `merge-gate`.
- Cuestiones de release: SEMVER no bumpea (es test-only, aditivo).

## Riesgos

- Si el binario `cognicode-mcp` no arranca en este entorno, los tests fallan. Mitigación: verificar build y arrancar manualmente antes de empezar.
- El test con SIGTERM requiere Unix (signal). Mitigación: usar `#[cfg(unix)]` para T3.
- Si el test depende de tools específicas (`graph_query`), el test depende de un tool estable. Mitigación: usar un tool trivial como `tools/list` o validar que el tool existe con assertion genérica.

## Recibo (al cierre)

| Fecha | Estado | Evidencia |
|---|---|---|
| 2026-10-10 | **DONE en código** | `cargo test -p cognicode-mcp --test a013_lifecycle_uat` → 9/9 verde. Lifecycle end-to-end (startup → tools/list → call read-only → shutdown limpio) pineado por el test contractual; los 6 RED triangulates (T1..T3 + los 3 GREEN baseline) se materializaron como tests reales. Wired en `merge-gate.pipeline.kts:cli-a013-lifecycle-uat` (stage que corre `cargo test -p cognicode-mcp --test a013_lifecycle_uat --quiet`). Sin código de producción cambiado. Doc-debt cleanup: este estado al cierre se añade al goal doc, no al ledger SDDK (storage sigue roto per `sddk-ledger-reconciliation.md`). |