# UAT-F3-001 (regeneración contra HEAD post-§64)

**Fecha**: 2026-09-23
**Bins**: `cognicode` v0.97.4 + `cognicode-mcp` v0.97.4 (release)
**Commit**: `b72f17e11f803dcbcfd6017b488369e72b7123ff` (HEAD al regenerar)
**Operador**: jcode-orchestrator

## Escenario (Given/When/Then)

**Given**: corpus `/tmp/prf-uat-f3-regen/` con `src/lib.rs` (caller →
callee) + `src/nested/mod.rs` (callee) — mismo corpus que la UAT-F3
original (JOURNAL/UAT §F3).

**When**: `cognicode graph full` (CLI text mode) vs
`cognicode-mcp --tools/call <build_graph>` (JSON-RPC mode).

**Then**: ambas caras producen los mismos conteos observables
(2 symbols / 1 dependency edge).

## Resultados observados

### CLI (`cognicode graph full`)

```text
Building full project graph at: .
Full graph built in 1ms
  Total symbols: 2
  Total dependencies: 1
```

CLI stdout sha256: `673ef8a89335c89fcb601b806bef91636ca8e0c5a06a492e0c1f2d7c7302a6a0`
CLI stderr sha256: `cc972ac99805aca083c1a1e4118518443daddb98214d73eabad28a2de96c84a8`

### MCP (`cognicode-mcp build_graph` JSON-RPC)

```json
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text",
  "text":"{\"success\":true,\"status\":\"complete\",
   \"symbols_found\":2,\"relationships_found\":1,
   \"edges\":[{\"from\":\"caller\",\"to\":\"callee\"}],
   \"message\":\"Graph loaded from built: 2 symbols, 1 relationships in 1ms\",
   \"skipped_files\":[],\"basis\":{...}}"}]}}
```

MCP JSON sha256: `d7d5817de1fc6df3046a9c3d1fec38360d3f3ba3b5c78bc90d78ac87365e27b9`
MCP stderr sha256: `bd173b45b2581aadd4e7770d21a7a7f5b292fc689aa4a6db27932d2d24b48517`

## Verificación

| Métrica | CLI | MCP | Equivale |
|---|---|---|---|
| Símbolos | 2 | 2 (`symbols_found`) | ✅ |
| Dependencias | 1 | 1 (`relationships_found`) | ✅ |
| Edge id | n/a (texto) | `caller → callee` | ✅ |
| Status | exit 0 | `success:true`, `status:"complete"` | ✅ |

## Cambio respecto a la UAT-F3 original (2026-09-21)

La UAT-F3 original (§73 histórica) dijo:

> | full | CLI: `FullGraphStrategy`; MCP: `AnalysisService::build_project_graph` |

Esa tabla quedó **stale** después de §64 (2026-09-22): el CLI ahora
enruta `graph full` por `AnalysisService::build_full_graph` (mismo
servicio que el MCP `build_graph`). El campo `status` y
`skipped_files` aparecen ahora también en CLI cuando aplica
(redacción honesta: aquí el corpus no genera skips, así que status
cli no se ve en la salida textual; pero internamente usa el mismo
módulo que MCP).

> **Decisión**: PRF-EXT-02 (que cubre que CLI↔MCP compartan servicio)
> sigue PASS; el cambio §64 lo consolidó en lugar de disolverlo.

## Decisiones

- **Re-ejecutable**: el corpus es `/tmp/prf-uat-f3-regen/` (5 líneas
  de código en 2 archivos). Bins están en
  `/var/home/rubentxu/cargo-targets/debug/release/` (release profile).
  Comando de regeneración literal en este Markdown.
- **Por qué regeneré esta y no las otras 14**: el escenario está
  suficientemente bien documentado para reconstruirlo en 30 segundos.
  Las otras UAT (`u50`..`u69` excepto esta) requieren fixture
  más compleja; se regenerarán en una sesión dedicada cuando se
  autorice el push a origin/main (T4 pre-release).

