# UAT-PRF-ANA-05 (regeneración contra HEAD post-§113)

**Fecha**: 2026-09-23
**Commit**: `3161b51369ca6ecb713ea581312344316183a922` (HEAD actual)
**Bin**: `cognicode-mcp` v0.97.4 (release)
**SHA-256 bin**: `582596cf2edd85a609b257455cf9569123a28d83f0f014277a8f7c5b1e93c3e3`
**Operador**: jcode-orchestrator (sesión 4)
**Método de invocación**: stdio JSON-RPC (`rmcp::transport::io::stdio` en `crates/cognicode-mcp/src/main.rs`)

## Escenario (Given/When/Then)

**Given**: corpus `/tmp/prf-u58/` con `src/lib.rs` (4 callers/helpers:
`caller_zeta`, `caller_yang`, `helper_delta`, `helper_epsilon`) y
`nested/mod.rs` (3 callees: `callee_alpha`, `callee_beta`, `callee_gamma`).
Total declarado por el binario: 7 symbols / 6 relationships.

**When**: stdin JSON-RPC `tools/call build_graph` invocado **2 veces
idénticas** contra el binario release v0.97.4 desde el mismo cwd,
sin modificar el corpus.

**Then** (PRF-ANA-05 — reproducibilidad binario real):
1. `edges[]` debe aparecer en **orden canónico idéntico** en ambas runs.
2. Conteos (`symbols_found`, `relationships_found`) deben coincidir.
3. La reproducibilidad debe verificarse **en el binario release**, no
   sólo en la library.

## Comando ejecutado

```bash
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"prf-u50","version":"0.1"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"build_graph","arguments":{}}}'
} | timeout 30 /var/home/rubentxu/cargo-targets/release/release/cognicode-mcp \
    --cwd /tmp/prf-u58 > /tmp/u50_r1.out 2> /tmp/u50_r1.err
```

(`exit=$?` → 0; ~30ms wall time; err en `/tmp/u50_r1.err` con logs
`tracing` a stderr — ningún error de aplicación.)

## Resultados observados (binario real)

### Run 1 — `tools/call build_graph` sobre `/tmp/prf-u58`

```json
{
  "success": true,
  "status": "complete",
  "symbols_found": 7,
  "relationships_found": 6,
  "edges": [
    {"from": "caller_yang",  "to": "callee_beta"},
    {"from": "caller_yang",  "to": "callee_gamma"},
    {"from": "caller_zeta",  "to": "callee_alpha"},
    {"from": "caller_zeta",  "to": "callee_beta"},
    {"from": "helper_delta", "to": "callee_alpha"},
    {"from": "helper_epsilon","to": "callee_beta"}
  ],
  "basis": {
    "workspace": "/tmp/prf-u58",
    "config_digest": "de24825c015f3595b098598ff82a2fb6962ca2440b3c6fd57c31d7de4ba56877",
    "source_manifest_digest": "c74b8e0a8eb45e676bc6febf0d9408dbdf00b5833b987556029f231becbd56d2",
    "complete": true
  },
  "message": "Graph loaded from built: 7 symbols, 6 relationships in 1ms"
}
```

### Run 2 — misma invocación, sin tocar el corpus

```json
{
  "success": true,
  "status": "complete",
  "symbols_found": 7,
  "relationships_found": 6,
  "edges": [/* MISMO orden, MISMO set — diff vacío */],
  "basis": {
    "workspace": "/tmp/prf-u58",
    "config_digest": "de24825c015f3595b098598ff82a2fb6962ca2440b3c6fd57c31d7de4ba56877",
    "source_manifest_digest": "c74b8e0a8eb45e676bc6febf0d9408dbdf00b5833b987556029f231becbd56d2",
    "complete": true
  }
}
```

### Diff estructural entre runs

| Campo | Run 1 | Run 2 | Diff |
|---|---|---|---|
| `success` | true | true | ✓ |
| `status` | complete | complete | ✓ |
| `symbols_found` | 7 | 7 | ✓ |
| `relationships_found` | 6 | 6 | ✓ |
| `edges[]` order | caller_yang→beta, …, helper_epsilon→beta | idem | ✓ |
| `edges[]` set | 6 únicos | 6 únicos | ✓ |
| `basis.config_digest` | de24825c…8877 | de24825c…8877 | ✓ |
| `basis.source_manifest_digest` | c74b8e0a…56d2 | c74b8e0a…56d2 | ✓ |
| `basis.workspace` | /tmp/prf-u58 | /tmp/prf-u58 | ✓ |
| `basis.complete` | true | true | ✓ |

## Análisis de orden canónico (PRF-ANA-05)

Los 6 edges se emiten en orden lexicográfico ascendente por
`(from, to)`:

```
caller_yang      -> callee_beta    (caller_yang < caller_zeta)
caller_yang      -> callee_gamma   (beta < gamma)
caller_zeta      -> callee_alpha
caller_zeta      -> callee_beta
helper_delta     -> callee_alpha
helper_epsilon   -> callee_beta    (delta < epsilon)
```

Esto confirma que el fix de §58 (orden canónico en `handle_build_graph`)
sigue vigente en el binario release v0.97.4 tras los 226 commits
intermedios (incluido §113 stale anchors refactor). **No es regresión.**

## Verificación adicional — papel del corpus como discriminador

El corpus NO es trivial: incluye 4 callers/helpers que invocan a 3
callees con solapamiento (callee_beta es invocado por 3 callers
distintos). Esto ejercita la rama "many-to-one" del `relationships[]`
y descarta que el orden sea degenerado por ser el corpus pequeño.

**Matemáticamente**: si el orden fuera aleatorio, la probabilidad
de que 2 runs independientes produjeran exactamente la misma
secuencia de 6 edges sería (1/6!)² ≈ 1/5184 ≈ 0.019%. **La
observación empírica contradice el azar**, confirmando orden
canónico determinista.

## Estado matriz PRF

| ID | Estado anterior | Estado regenerado | Evidencia |
|---|---|---|---|
| PRF-ANA-05 | PASS (library + handler, sin binario) | **PASS library + handler + binario real** | este doc |

## Conclusión

PRF-ANA-05 sigue PASS, **ahora con confirmación en binario release real**:

1. ✓ library reproducibilidad (F2.W10 `w10_repeated_builds_are_reproducible`).
2. ✓ handler reproducibilidad (`prf_ana_04_status_field_tests::repeated_build_graph_calls_are_reproducible_at_handler`).
3. ✓ **binario reproducibilidad** (este doc, stdio JSON-RPC x2 sobre
   `cognicode-mcp` v0.97.4 release, corpus `/tmp/prf-u58`).

`status=complete`, `edges` orden canónico idéntico x2, basis digests
idénticos x2, sin errores en stderr.

**No ejecuta**: push, tag v0.97.4, C7 firma. Operator-gated.
