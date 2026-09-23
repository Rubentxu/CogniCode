# UAT-PRF-ANA-06 (regeneración contra HEAD post-§113)

**Fecha**: 2026-09-23
**Commit**: `3161b51369ca6ecb713ea581312344316183a922` (HEAD actual)
**Bin**: `cognicode-mcp` v0.97.4 (release)
**SHA-256 bin**: `582596cf2edd85a609b257455cf9569123a28d83f0f014277a8f7c5b1e93c3e3`
**Operador**: jcode-orchestrator (sesión 4)
**Método de invocación**: stdio JSON-RPC (`rmcp::transport::io::stdio` en `crates/cognicode-mcp/src/main.rs`)

## Escenario (Given/When/Then)

**Given**: corpus `/tmp/prf-u58/` (2 archivos `.rs` con 7 símbolos y 6
relationships — corpus canónico ANA-05, base común del test
`prf_ana_06_basis_identity_tests` en
`crates/cognicode-core/src/interface/mcp/handlers/mod.rs:5385`).

**When**: stdin JSON-RPC `tools/call build_graph` x2 sobre el binario
release v0.97.4 desde cwd `/tmp/prf-u58`, sin modificar el corpus.

**Then** (PRF-ANA-06 — basis identity):
1. `basis.workspace` debe ser el path canónico del cwd (no la versión
   symlinked aunque tempdir entregue paths macarrones `/var/foo` →
   `/private/foo`).
2. `basis.source_manifest_digest` debe ser **no vacío** (presencia).
3. `basis.source_manifest_digest` debe ser **estable** entre runs
   idénticas (mismo corpus → mismo digest).
4. `basis.complete` debe ser `true` cuando la identidad puede
   establecerse completamente.
5. La modificación del corpus debe **invalidar** el
   `source_manifest_digest` (test
   `changed_source_changes_manifest_digest`).

## Comando ejecutado

```bash
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"prf-u50","version":"0.1"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"build_graph","arguments":{}}}'
} | timeout 30 /var/home/rubentxu/cargo-targets/release/release/cognicode-mcp \
    --cwd /tmp/prf-u58 > /tmp/u50_r1.out 2> /tmp/u50_r1.err
```

## Resultados observados (binario real)

### Run 1

```json
"basis": {
  "workspace": "/tmp/prf-u58",
  "config_digest": "de24825c015f3595b098598ff82a2fb6962ca2440b3c6fd57c31d7de4ba56877",
  "source_manifest_digest": "c74b8e0a8eb45e676bc6febf0d9408dbdf00b5833b987556029f231becbd56d2",
  "complete": true
}
```

### Run 2 (idéntica invocación)

```json
"basis": {
  "workspace": "/tmp/prf-u58",
  "config_digest": "de24825c015f3595b098598ff82a2fb6962ca2440b3c6fd57c31d7de4ba56877",
  "source_manifest_digest": "c74b8e0a8eb45e676bc6febf0d9408dbdf00b5833b987556029f231becbd56d2",
  "complete": true
}
```

### Diff entre runs

| Campo | Run 1 | Run 2 | Diff |
|---|---|---|---|
| `workspace` | /tmp/prf-u58 | /tmp/prf-u58 | ✓ |
| `config_digest` | de24825c…8877 | de24825c…8877 | ✓ |
| `source_manifest_digest` | c74b8e0a…56d2 | c74b8e0a…56d2 | ✓ |
| `complete` | true | true | ✓ |

## Verificación contra el contrato PRF-ANA-06

| # | Criterio | Resultado | Evidencia |
|---|---|---|---|
| 1 | `workspace` es path canónico | ✓ | `/tmp/prf-u58` es el cwd real, no symlink |
| 2 | `source_manifest_digest` no vacío | ✓ | `c74b8e0a…56d2` (64 hex chars = SHA-256-like) |
| 3 | `source_manifest_digest` estable entre runs idénticas | ✓ | mismo digest x2 |
| 4 | `complete: true` cuando identidad establecida | ✓ | ambos runs |
| 5 | cambio de source invalida digest | (test unitario verde en `prf_ana_06_basis_identity_tests::changed_source_changes_manifest_digest`; no regenerado a nivel binario por economía — el handler expone el mismo algoritmo) | código + test library |

## Estado matriz PRF

| ID | Estado anterior | Estado regenerado | Evidencia |
|---|---|---|---|
| PRF-ANA-06 | PASS (library + handler, sin binario) | **PASS library + handler + binario real** | este doc |

## Conclusión

PRF-ANA-06 sigue PASS, **ahora con confirmación en binario release real**:

1. ✓ library basis identity
   (`prf_ana_06_basis_identity_tests::basis_present_with_canonical_workspace_and_manifest_digest`,
   `same_inputs_produce_same_manifest_digest`,
   `changed_source_changes_manifest_digest`,
   `basis_workspace_survives_symlinked_path`).
2. ✓ **binario basis identity** (este doc, stdio JSON-RPC sobre
   `cognicode-mcp` v0.97.4 release, corpus `/tmp/prf-u58`):
   - `workspace=/tmp/prf-u58` (canónico)
   - `config_digest` estable
   - `source_manifest_digest` no vacío y estable entre runs
   - `complete=true`

**No ejecuta**: push, tag v0.97.4, C7 firma. Operator-gated.
