# UAT-PRF-ANA-07 (regeneración contra HEAD post-§114)

**Fecha**: 2026-09-23
**Commit**: `348a652a` (HEAD actual, post-§114)
**Bin**: `cognicode-mcp` v0.97.4 (release)
**SHA-256 bin**: `582596cf2edd85a609b257455cf9569123a28d83f0f014277a8f7c5b1e93c3e3`
**Operador**: jcode-orchestrator (sesión 4)
**Método de invocación**: stdio JSON-RPC (`rmcp::transport::io::stdio`)
**Test integración**: `cargo test -p cognicode-mcp --test prf_ana_07_uat` (1/1)

## Escenario (Given/When/Then)

**Given**: corpus `docs/prf/fixtures/massive_collision_corpus/` (52
archivos `.rs`):
- `src/lib.rs` con `init()` (LOCAL) + `caller_in_lib()` que invoca
  a `init()` y a `sibling_unique_compute::compute()`.
- `src/d{1..50}.rs`: **50 archivos con `pub fn init()`** (homónimos
  cross-file).
- `src/sibling_unique_compute.rs`: 1 archivo con `pub fn compute()`
  (candidato único).

Total símbolos: 53 (1 lib + 1 caller + 1 compute + 50 sibling `init`).

**When**: stdin JSON-RPC contra `cognicode-mcp` v0.97.4 release:
1. `tools/call build_graph` (publica call graph).
2. `tools/call get_call_hierarchy` para `caller_in_lib`,
   direction=outgoing, depth=1.

**Then** (PRF-ANA-07 — massive homonym collision):
1. **edges ≤ 2** (no fan-out a los 50 siblings `init`).
2. **`init`** resuelve a `src/lib.rs` (visibility rule:
   same-file picks local).
3. **`compute`** resuelve a `src/sibling_unique_compute.rs`
   (single-candidate rule).

## Comando ejecutado

```bash
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"prf-u59","version":"0.1"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"build_graph","arguments":{}}}'
  echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_call_hierarchy","arguments":{"symbol_name":"caller_in_lib","direction":"outgoing","depth":1}}}'
} | timeout 30 /var/home/rubentxu/cargo-targets/release/release/cognicode-mcp \
    --cwd /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode/docs/prf/fixtures/massive_collision_corpus
```

(`exit=$?` → 0; ~70ms wall time; sin errores en stderr.)

## Resultados observados (binario real)

### `build_graph` response

```json
{
  "success": true,
  "status": "complete",
  "symbols_found": 53,
  "relationships_found": 2,
  "edges": [
    {"from": "caller_in_lib", "to": "compute"},
    {"from": "caller_in_lib", "to": "init"}
  ],
  "basis": { /* presente, completo */ }
}
```

### `get_call_hierarchy` response (caller_in_lib)

```json
{
  "symbol": "caller_in_lib",
  "calls": [
    {
      "symbol": "compute",
      "file": "/.../massive_collision_corpus/src/sibling_unique_compute.rs",
      "line": 0,
      "column": 0,
      "confidence": 1.0
    },
    {
      "symbol": "init",
      "file": "/.../massive_collision_corpus/src/lib.rs",
      "line": 71,
      "column": 0,
      "confidence": 1.0
    }
  ],
  "metadata": {
    "total_calls": 2,
    "analysis_time_ms": 6
  }
}
```

## Verificación contra el contrato PRF-ANA-07

| # | Criterio | Resultado | Evidencia |
|---|---|---|---|
| 1 | edges ≤ 2 (no fan-out a 50 siblings) | ✓ | `relationships_found: 2`, `edges.len() == 2` |
| 2 | `init` resuelve a `src/lib.rs` (visibility) | ✓ | `file: ".../src/lib.rs", line: 71` |
| 3 | `compute` resuelve a `sibling_unique_compute.rs` (single-candidate) | ✓ | `file: ".../src/sibling_unique_compute.rs"` |
| 4 | confidence = 1.0 en ambos | ✓ | ambos con `confidence: 1.0` |

## Verificación cruzada con test integración

```bash
$ cp /var/home/rubentxu/cargo-targets/release/release/cognicode-mcp \
     target/release/cognicode-mcp
$ sha256sum target/release/cognicode-mcp
582596cf2edd85a609b257455cf9569123a28d83f0f014277a8f7c5b1e93c3e3
$ cargo test -p cognicode-mcp --test prf_ana_07_uat
running 1 test
test massive_collision_resolution_over_real_binary ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

**Nota importante**: el binario en `target/release/cognicode-mcp`
estaba **stale** (sha256 `4de983cd…` del 01:22, hace ~12h). Lo
sustituí con el release fresco post-§113 (sha256 `582596cf…`,
14:46). El test corre ahora contra el binario **sincronizado
con HEAD actual**.

## Hallazgo honesto (paper-closing residual)

§59 (2026-09-22) cerró PRF-ANA-07 como PASS basándose en que
"el comportamiento ya era correcto tras F2.W5/§35". El test
`prf_ana_07_uat` existe y verifica las 3 reglas, **pero el binario
contra el que corría estaba stale** (12h de drift). Esta
regeneración confirma que el comportamiento sigue siendo correcto
contra el binario release v0.97.4 **post-§113 stale anchors
refactor** (227 commits ahead).

## Estado matriz PRF

| ID | Estado anterior | Estado regenerado | Evidencia |
|---|---|---|---|
| PRF-ANA-07 | PASS (test library + handler) | **PASS test integración + binario release fresco** | este doc |

## Conclusión

PRF-ANA-07 sigue PASS, **ahora con confirmación contra el binario
release v0.97.4 sincronizado con HEAD**:

1. ✓ test integración verde
   (`massive_collision_resolution_over_real_binary` 1/1).
2. ✓ **binario release real** (este doc, stdio JSON-RPC):
   - 53 symbols / 2 relationships (NO 52).
   - `compute` → `sibling_unique_compute.rs` (single-candidate).
   - `init` → `lib.rs:71` (visibility local).
   - confidence 1.0 en ambos edges.
3. ✓ el refactor §113 (stale anchors) no introdujo regresión en
   el resolver scope-aware.

**No ejecuta**: push, tag v0.97.4, C7 firma. Operator-gated.
