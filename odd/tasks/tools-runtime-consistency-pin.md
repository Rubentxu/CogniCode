# Feature: A-016 tools/runtime contract consistency pin

> **Goal:** Auto-pin contra el tipo de drift que Lesson 95 (N+32) apuntaba a prevenir, independientemente de que el caso particular de `build_graph` NO fuera real. Pinea la reciprocidad entre `product/tools.json::authority` (public contract) y `CogniCodeHandler::MUTATING_TOOLS` (runtime). Test-only WU.

## Estado al inicio

- **HEAD:** `e4010983199c0f2196b7e4096ebe4df40634d38c` (rama `docs/cp2-a012-closure`, 9 ahead de origin/main).
- **Estado actual coherente:** tools.json tiene 3 tools mutating (`write_file`, `edit_file`, `reparse_on_edit`); MUTATING_TOOLS tiene los mismos 3. Las 3 reciprocidades pasan en el primer intento.
- **Drift detectado (Lesson 95):** falso positivo — `build_graph` declarado `authority: read` en tools.json, `mutates_workspace: false` en runtime_metadata, MUTATING_TOOLS runtime NO incluye `build_graph`. Las tres declaraciones son coherentes porque el cache es in-memory.
- **Lesson 96 debt (separada):** 4 forks de `Session` paralelos al `common::McpSession` canónico. NO scope de este WU.

## Disciplina: Strict TDD

1. **RED** — escribir las 3 reciprocidad tests (contract→runtime, runtime→contract, authority valid).
2. **GREEN** — el sistema actual ya está coherente; las 3 tests pasan en el primer intento. No requiere cambios de producto.
3. **TRIANGULATE** — las 3 tests cubren las 3 mutaciones posibles (añadir tool a runtime sin contract; cambiar authority sin actualizar runtime; dejar authority null/mal formado). Cualquier drift futuro falla en CI con diff explícito.
4. **REFACTOR** — no necesario.

## Tasks

Ver `todo` projection.

## Out of scope

- Consolidar los 4 forks de `Session` paralelos (Lesson 96 queda como deuda separada). 
- Tocar `product/tools.json` ni `MUTATING_TOOLS` runtime — no hay drift que arreglar.
- Resolver `RUSTSEC-2024-0437` (protobuf advisory) — requiere OTel 0.28 migration; fuera de scope.
- Reorganizar workflows CI — fuera de scope.
- Promover el pin a required check separado del `merge-gate` — decisión de governance separada.
- Cualquier bump de SemVer — test-only WU, regla 6 sin bump.

## Riesgos

- Si el path `product/tools.json` se mueve en el futuro, el pin falla con un error de path en vez del drift que pretende pinear. Mitigación: el panic message es claro (`product/tools.json must be readable`).
- Si `MUTATING_TOOLS` se mueve a otro módulo, el test pin falla por compilación. Mitigación: use statement `use cognicode_core::interface::mcp::rmcp_adapter::CogniCodeHandler;` es estable mientras la constant esté en `rmcp_adapter`; un cambio de ubicación requeriría migrar el test (esperado).

## Cambios

- `crates/cognicode-mcp/tests/a016_tools_runtime_consistency.rs` (new, 162 líneas) — 3 reciprocidad tests + helper `load_tools_json()` + helper `product_tools_json_path()`.

## Recibo (al cierre)

| Fecha | Estado | Evidencia |
|---|---|---|
| 2026-10-10 | **DONE en código** | `cargo test -p cognicode-mcp --test a016_tools_runtime_consistency` → verde. Las 3 reciprocidades (contract→runtime, runtime→contract, authority valid) pineadas; las 3 triangulaciones (añadir tool a runtime sin contract; cambiar authority sin actualizar runtime; authority null/mal formado) RED-detectan drift futuro con diff explícito. Wired en `merge-gate.pipeline.kts:cli-a016-tools-runtime-consistency` (etapa que corre el test). Lesson 96 (4 forks de Session) queda como deuda separada, fuera de scope de este WU. Doc-debt cleanup: el goal doc ya describía el estado como "coherente" sin commit formal; este recibo lo materializa. |