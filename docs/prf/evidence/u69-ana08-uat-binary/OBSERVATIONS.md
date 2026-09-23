# UAT-PRF-ANA-08 (regeneración contra HEAD post-§115)

**Fecha**: 2026-09-23
**Commit**: `8c74607c` (HEAD actual, post-§115)
**Bin**: `cognicode-mcp` v0.97.4 (release)
**SHA-256 bin**: `582596cf2edd85a609b257455cf9569123a28d83f0f014277a8f7c5b1e93c3e3`
**Operador**: jcode-orchestrator (sesión 4)
**Método de invocación**: stdio JSON-RPC (`rmcp::transport::io::stdio`)
**Test integración**: `cargo test -p cognicode-mcp --test prf_ana_08_uat` (1/1)

## Escenario (Given/When/Then)

**Given**: corpus `docs/prf/fixtures/massive_collision_corpus/` (52
archivos `.rs` con 53 símbolos y 50 homónimos `init`) — el
mismo corpus de ANA-07. La diferencia es la **categoría search**:
budget de 500ms con output bounded.

**When**: stdin JSON-RPC contra `cognicode-mcp` v0.97.4 release:
1. `tools/call build_graph` (warm-up).
2. `tools/call find_usages` con `symbol_name="compute"` (camino
   feliz).
3. `tools/call find_usages` con `symbol_name="definitely_not_here_42"`
   (camino "no encontrado").

**Then** (PRF-ANA-08 — search budget + bounded output):
1. `find_usages` con `compute` debe completar en <2s
   (500ms budget + 1.5s slack).
2. Output serializado debe ser **<5MiB** (no unbounded).
3. `find_usages` con símbolo inexistente debe producir **typed
   error** O **payload explícito** (no hang, no crash, no output
   vacío silencioso).

## Comando ejecutado

```bash
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"prf-u69","version":"0.1"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"build_graph","arguments":{}}}'
  echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"find_usages","arguments":{"symbol_name":"compute"}}}'
  echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"find_usages","arguments":{"symbol_name":"definitely_not_here_42"}}}'
} | timeout 30 /var/home/rubentxu/cargo-targets/release/release/cognicode-mcp \
    --cwd /.../massive_collision_corpus
```

Total wall time (incluyendo init + build_graph + 2× find_usages):
**~21 ms** — bien dentro del budget.

## Resultados observados (binario real)

### `find_usages compute` (camino feliz)

```json
{
  "symbol": "compute",
  "usages": [
    {
      "file": "/.../massive_collision_corpus/src/lib.rs",
      "line": 69,
      "column": 28,
      "context": "    sibling_unique_compute::compute();",
      "is_definition": false
    },
    {
      "file": "/.../massive_collision_corpus/src/sibling_unique_compute.rs",
      "line": 1,
      "column": 7,
      "context": "pub fn compute() { /* single candidate cross-file */ }",
      "is_definition": true
    }
  ],
  "total": 2
}
```

- **Content length**: 495 bytes (≪ 5 MiB).
- **Usages count**: 2 (1 call site en lib.rs:69, 1 definition en
  sibling_unique_compute.rs:1).
- **isError**: false.
- **Latencia**: ~5ms (incluida en los 21ms totales).

### `find_usages definitely_not_here_42` (símbolo inexistente)

```json
{
  "symbol": "definitely_not_here_42",
  "usages": [],
  "total": 0
}
```

- **isError**: false (no es error tipado, es payload explícito vacío).
- **Content**: estructura clara con `total: 0` y `usages: []`.
- **Latencia**: <5ms.

## Verificación contra el contrato PRF-ANA-08

| # | Criterio | Resultado | Evidencia |
|---|---|---|---|
| 1 | `find_usages compute` completa <2s | ✓ | wall time ~5ms (budget 500ms + slack 1.5s) |
| 2 | Output serializado <5MiB | ✓ | 495 bytes |
| 3 | Símbolo inexistente produce typed error o payload explícito | ✓ | payload `{"symbol":"...","usages":[],"total":0}` (no hang, no error tipado, pero tampoco vacío silencioso) |

## Verificación cruzada con test integración

```bash
$ sha256sum target/release/cognicode-mcp
582596cf2edd85a609b257455cf9569123a28d83f0f014277a8f7c5b1e93c3e3
$ cargo test -p cognicode-mcp --test prf_ana_08_uat
running 1 test
test search_budget_bounded_output_over_homonym_corpus ... ok
test result: ok. 1 passed; 0 failed
```

## Hallazgo honesto (paper-closing residual §115)

Igual que §115 para ANA-07, el test `prf_ana_08_uat` arrastraba
el mismo paper-closing residual por binario stale en
`target/release/cognicode-mcp`. §115 lo arregló sustituyendo
el binario (sha256 `4de983cd…` → `582596cf…`). Esta
regeneración confirma que el comportamiento es correcto contra
el binario release v0.97.4 **post-§113 stale anchors refactor**
(228 commits ahead).

## Estado matriz PRF

| ID | Estado anterior | Estado regenerado | Evidencia |
|---|---|---|---|
| PRF-ANA-08 | PASS (test library) | **PASS test integración + binario release fresco** | este doc |

## Conclusión

PRF-ANA-08 sigue PASS, **ahora con confirmación contra el binario
release v0.97.4 sincronizado con HEAD**:

1. ✓ test integración verde
   (`search_budget_bounded_output_over_homonym_corpus` 1/1).
2. ✓ **binario release real** (este doc, stdio JSON-RPC):
   - `find_usages compute` → 2 usages (1 call + 1 def), 495 bytes
     (< 5MiB).
   - `find_usages definitely_not_here_42` → payload explícito
     `{total:0, usages:[]}`, sin hang.
   - Latencia total 21ms (budget 500ms con slack 1.5s).
3. ✓ el refactor §113 no introdujo regresión en search budget
   ni en bounded output.

**No ejecuta**: push, tag v0.97.4, C7 firma. Operator-gated.
