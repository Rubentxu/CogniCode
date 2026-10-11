# B2.W2 — Single source of truth for the tool catalog

> **Estado**: ACTIVE 2026-10-11. Desbloqueado por B2.W1.
> Padre: [`b2-shared-analysis-capability-contracts.md`](b2-shared-analysis-capability-contracts.md).

## Valor

`cognicode capabilities --json` (CLI) y MCP `tools/list` derivan del mismo
catalogo tipado en disco. La lista paralela en
`rmcp_adapter::build_all_tools` (76 inline `Tool::new(...)` con su `cognicode_meta`)
queda eliminada: cada `ToolDescriptor` se carga una vez y se proyecta dos veces.

## Work units

- [x] **T1** — `ToolDescriptor` struct + `load_tool_catalog()` en
  `crates/cognicode-core/src/interface/catalog.rs`. Loader con la
  misma logica de anchor que `resolve_product_assets_root`
  (cwd + parents + exe + parents, bounded 32 hops).
- [x] **T2** — Red `parity test` (`b2_w2_catalog_parity.rs`) que
  demuestra el drift actual: set de nombres del catalog ≠ set del MCP.
- [x] **T3** — `build_capabilities_doc` en `commands.rs` pasa a
  serializar `Vec<ToolDescriptor>` directamente. Preserva el shape
  JSON del archivo en disco (mismas 13 claves por tool).
- [x] **T4** — `build_all_tools()` en `rmcp_adapter.rs` itera el
  catalogo tipado y proyecta a `Tool` MCP (5-field meta
  `cognicode` block). Feature gating explicito via campo `feature`.
- [x] **T5** — Anadir `interproc_summary`, `list_evidence`,
  `search_evidence` a `product/tools.json` con sus
  schemas/cognicode_meta. Marcar las 5 tools con feature gating
  explicito (`persistence`, `program-analysis-server`,
  `evidence-cli-ladybug`).
- [x] **T6** — Gate: paridad, clippy `-D warnings`, fmt, fast + full.

## Acceptance criteria

- `set(load_tool_catalog().feature_filtered().names) == set(build_all_tools().names)`
  en build default y (donde aplique) sin features.
- Para cada tool, `descriptor.authority == "mutating"` ssi
  el nombre aparece en `mutating_tool_names()`.
- `cognicode capabilities --json` no cambia el shape JSON para los 73
  tools preexistentes: 13 campos por tool, mismo orden de campos
  en la salida (serde conserva el orden de declaracion del struct).
- `bash scripts/test-full.sh` exit 0.
- `cargo clippy -p cognicode-core --tests -- -D warnings` sin findings.
- `cargo fmt --all --check` limpio.

## Restricciones (heredadas)

- 73 tools preexistentes mantienen su nombre publico.
- 3 tools MCP-only (`interproc_summary`, `list_evidence`,
  `search_evidence`) se anaden a `tools.json` en lugar de
  eliminarse del MCP.
- `mutating_tool_names()` sigue derivandose de `build_all_tools()`,
  sin lista paralela (PRF-MCP-05).
- Comportamiento `--json` fail-closed: si el archivo no se
  encuentra, error envelope en lugar de `tools: 0` silencioso.

## Decisiones

- **`requirements`** se deserializa como `serde_json::Value` (no
  como struct tipado) para preservar exactamente el shape actual
  `{graph: bool, persistence: bool, cache: null, network: null}`.
- **`feature: Option<String>`** se serializa solo cuando `Some`;
  los 71 tools no-gated no ganan una clave `"feature": null`.
- **Filtrado de feature**: `tool_feature_enabled()` resuelve via
  `cfg!(feature = "...")` por nombre. Una feature desconocida
  omite el tool (fail-closed).
- **3 tools MCP-only** se aniaden con campo `feature` para que
  el filtro las omita cuando su feature no esta activa.
- **`detect_drift` y `reparse_on_edit`** reciben `"feature":
  "persistence"` para preservar la simetria con los cfg-gates
  en el dispatch arm de `call_tool_handler`.

## Cambios

| Archivo | Tipo | Notas |
| --- | --- | --- |
| `crates/cognicode-core/src/interface/catalog.rs` | NEW | ~120 lineas |
| `crates/cognicode-core/src/interface/mod.rs` | +1 | `pub mod catalog;` |
| `crates/cognicode-core/src/interface/cli/commands.rs` | edit | `build_capabilities_doc` carga via catalog |
| `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` | edit | `build_all_tools` itera catalog |
| `crates/cognicode-core/tests/b2_w2_catalog_parity.rs` | NEW | 5 tests de paridad |
| `product/tools.json` | edit | +3 tools, +5 campos `feature` |

## Recibo

| Fecha | WU | SHA | Resultado |
|---|---|---|---|
| 2026-10-11 | B2.W2 | _pendiente_ | Paridad verde; gates full verde. |