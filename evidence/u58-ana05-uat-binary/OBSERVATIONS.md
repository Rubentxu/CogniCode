# PRF-ANA-05 — UAT binario real (stdio JSON-RPC)

Fecha: 2026-09-22 · binario: target/release/cognicode-mcp (rebuild fresco
post-fix, SHA verificado por tamaño/timestamp) · corpus:
docs/prf/fixtures/massive_collision_corpus (51 homónimos)

## RED inicial (defecto REAL encontrado)

3 llamadas idénticas a build_graph sobre la misma workspace producían
payloads que diferían: el array `edges` llegaba en orden distinto entre
ejecuciones (`init,compute` vs `compute,init`). La iteración del grafo no
es estable entre builds. Los digests basis SÍ eran estables.

## Fix

`handle_build_graph` (cognicode-core/src/interface/mcp/handlers/mod.rs):
orden canónico de edges por (from, to) antes de publicar el payload.

## GREEN

`crates/cognicode-mcp/tests/prf_ana_05_uat.rs`: 1/1 PASS sobre el binario
real — 3 runs idénticos en vista semántica (status=complete, symbols=53,
relationships=2, edges canónicos, skipped=[], config_digest y
source_manifest_digest presentes e iguales).

## Regresión

- handlers lib: 153/0
- workspace_isolation (evidence-kernel): 2/0
- continuation_e2e: 5/0
- fmt limpio; clippy core+mcp 0 errores

## Disposición

ANA-05: el pino de reproducibilidad ahora cubre library, handler Y binario
real → PASS.
