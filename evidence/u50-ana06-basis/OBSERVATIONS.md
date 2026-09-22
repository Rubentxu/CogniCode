# PRF-ANA-06 — Basis identity (workspace canónico + config digest + source manifest)

Fecha: 2026-09-22 · HEAD de la unidad (ver JOURNAL §49) · binario real `cognicode-mcp` release

## Requisito (SPEC-ANALYSIS)

> PRF-ANA-06 MUST: para consultas con identidad histórica, `basis` incluye workspace
> canónico + config digest + source manifest/revisión y provenance de provider/graph
> según capacidad; si la identidad no puede establecerse se declara incompleto.

## Implementación

`build_graph` MCP output añade campo `basis` (`BasisDto`, backward-compatible con
`skip_serializing_if`):

- `workspace`: ruta canónica (canonicalize) del directorio del proyecto.
- `config_digest`: SHA-256 sobre la config efectiva relevante (log level hoy; la
  superficie crece con los handlers). Nunca fabricado: si el lock de lectura falla,
  se digiere `"unreadable"`.
- `source_manifest_digest`: SHA-256 sobre pares `(rel_path, content_hash)` ordenados
  del source manifest. Mismas fuentes ⇒ mismo digest; cualquier cambio ⇒ digest distinto.
- `complete`: `true` solo si todos los componentes se establecieron; en caso contrario
  se declara incompleto (sin ausencia silenciosa).

## Verificación

### Library (RED→GREEN, 4 tests nuevos `prf_ana_06_basis_identity_tests`)

- RED confirmado: campo `basis` inexistente → `E0609`.
- GREEN: basis presente con workspace canónico; digest estable ante repetición;
  digest cambia con fuente cambiada; workspace absoluto/canónico ante path symlinkeado.
- Suite completa `cognicode-core` lib: **2145 passed / 0 failed / 27 ignored**.
- `cognicode-mcp` e2e (`continuation_e2e`): 5 passed. `cognicode_lifecycle`: 7 passed.
- `cargo clippy -p cognicode-core --all-targets`: 0 errores. `cargo fmt` aplicado.

### UAT binario real (stdio JSON-RPC)

Corpus temporal con 1 fuente; secuencia initialize → tools/call `build_graph`:

```
workspace: /tmp/ana06-uat
config_digest: de24825c015f3595...
source_manifest_digest: d138605ade98991d...
complete: True
```

Asserts: `basis` presente, `complete == true`, `workspace == realpath` del corpus.
**PASS.**

## Nota honesta

La primera pasada UAT sobre el binario falló porque el binario release estaba stale
(fallo previo de rebuild silencioso al ser reutilizado). Tras rebuild forzado la UAT
pasa. Lección registrada: validar frescura de binario antes de UAT.

## Estado

PASS. Pendiente (no bloqueante): extender config_digest cuando exista superficie de
config de análisis; provenance de provider/graph se declara según capacidad actual
(graph propio, sin providers externos).
