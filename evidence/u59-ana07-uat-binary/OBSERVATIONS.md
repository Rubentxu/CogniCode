# PRF-ANA-07 — UAT binario real (stdio JSON-RPC, corpus 51 homónimos)

Fecha: 2026-09-22 · binario fresco (post-§58) · corpus
docs/prf/fixtures/massive_collision_corpus

## Resultado GREEN a la primera (comportamiento ya correcto)

- build_graph: status=complete, exactamente 2 relationships (ninguno de
  los 50 `d*::init()` se filtra como target).
- get_call_hierarchy outgoing de caller_in_lib:
  - `init` → `src/lib.rs` (regla de visibilidad local gana sobre 50
    homónimos sibling), confidence 1.0
  - `compute` → `src/sibling_unique_compute.rs` (regla de candidato
    único), confidence 1.0

## Verificación

`crates/cognicode-mcp/tests/prf_ana_07_uat.rs`: 1/1 PASS sobre binario
real. Sonda manual previa (python stdio) confirmó el mismo resultado.

## Disposición

ANA-07: library (F2.W5) + corpus stress (§35) + binario real (esta UAT)
→ PASS.
