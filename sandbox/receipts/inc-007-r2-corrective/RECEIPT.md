# R2 + R3 — INC-007 G4 RED corrective action + recovery run

**Fecha**: 2026-09-20T17:48Z
**Auditor**: SDDK orchestrator session

## R1 hallazgo integrado

El run 20260919T161019 que produjo G4 RED **se ejecutó con un binario
anterior a H4.1**. La fix H4.1 (commit `6769c2c6` del 2026-09-19 19:35)
introdujo el contrato honesto de truncación (`truncated=true, has_more=true,
next_token=Some(...)`). El binario `target/release/cognicode-mcp` con
H4.1+ se compiló después (2026-09-19 23:28), pero el run histórico usó un
binario previo sin esa lógica.

Por eso el response del run histórico dice `truncated: false` aunque
devuelva 500 líneas de 730 — el binario previo no implementaba el flag.

## R2 — El fix de producto YA EXISTE en H4.1 (sin necesidad de nuevos cambios)

### Estado del código (HEAD `e65fbc8d`)

`crates/cognicode-core/src/application/services/file_operations.rs:613-616`:

```rust
let truncated = match input.end_line {
    Some(_) => false,
    None => end_line < total_lines,
};
```

Y H4.3 (`crates/cognicode-sandbox/src/main.rs:1270-1410`) hace la
reconstrucción desde disco cuando `truncated && has_more`.

### Validación: test unitario pasa con TMPDIR=/tmp

```
$ TMPDIR=/tmp cargo test -p cognicode-core --lib \
    file_501_lines_must_advertise_continuation
test result: ok. 1 passed; 0 failed; ...
```

Test PINS:
- `result.truncated == true` (cuando end_line < total_lines y caller no pasó end_line)
- `result.has_more == true`
- `result.next_token.is_some() == true`

### Validación: binario en runtime emite el contrato correcto

```
$ (echo '...'; echo '...') | target/release/cognicode-mcp
truncated: True
has_more: True
next_token: eyJwYXRoIjoiL3Zhci9tbnQvRGlzY29DaGlubzItZmFzdC9Qcm95ZWN0b3MvcnVzdC9Db2duaUNvZGUvc2FuZGJveC9yZXBvcy9hbnlob3cvc3JjL2xpYi5ycyIsIm9mZnNldCI6MTQ0MjEsImNodW5rX3NpemUiOjUwMCwibW9kZSI6InBhZ2luYXRlZCJ9
total_lines: 730
end_line: 500
start_line: 1
```

**El binario actual emite el contrato honesto.**

## R3 — Run nuevo con identidad acreditada

### Comando ejecutado

```
TMPDIR=/tmp sandbox/scripts/run_campaign.sh --repeat 1 \
    sandbox/manifests-tier1/tier1_h3_read_source.yaml
```

- HEAD: `e65fbc8d`
- Binario: `target/release/sandbox-orchestrator` (4765824 bytes, post-H4.3)
- Resultados en `sandbox/results-runs/20260920T174813/`
- `measured_source_head: e65fbc8d` (acreditado en `campaign_manifest.json`)
- 10 escenarios, 100% pass rate, health 100/100

### Hallazgo: H4.3 reconstruction funciona, PERO scoring engine NO usa el contenido reconstruido

`anyhow read_source` result.json muestra:

```json
{
  "dimension_scores": {
    "correctitud": 0.11337868480725624,
    "latencia": 100.0,
    ...
  },
  "reconstructed": {
    "status": "complete",
    "source": "disk_fallback",
    "pages": 2,
    "total_bytes": 21209,
    "sha256_reconstructed": "1c774243700f38ccaced1609c9e37a25c01f5e8aa900b876aef206207d6e4846",
    "sha256_disk": "1c774243700f38ccaced1609c9e37a25c01f5e8aa900b876aef206207d6e4846",
    "content": "<21209 bytes del archivo completo>"
  }
}
```

**Análisis**:

- `reconstructed.status = complete` ✓
- `sha256_reconstructed == sha256_disk` ✓ (verificación byte-exacta pasó)
- `reconstructed.content` tiene los 21209 bytes del archivo (730 líneas)
- PERO `correctitud = 0.11` (no 100)

El bug es que **el scoring engine mide correctitud sobre el `content` del response ORIGINAL, no sobre el `reconstructed.content`**. Por eso, aunque la reconstrucción reconstruyó correctamente, el matcher Jaccard sigue comparando los 500 primeros caracteres contra el ground truth completo.

## Causa raíz del nuevo G4 RED

**NO es un nuevo bug del producto**. Es un **bug pre-existente del scoring engine**: el sandbox reconstruye el contenido en `reconstructed.content` pero el `score_scenario()` (en `crates/cognicode-core/src/sandbox_core/scoring.rs`) lee de `tool_response`, no de `reconstructed.content`.

**R2 correctivo mínimo**:

Modificar `score_scenario()` para que cuando `reconstructed.status == complete`,
use `reconstructed.content` en lugar del contenido del `tool_response` original.

Esta fix es de ~10 LOC y reutiliza el contrato existente de H4.3 (status +
sha256 + content). NO requiere cambiar umbrales, ni relajar el matcher, ni
mover el cap.

## Regression tests necesarias

1. `test_reconstructed_content_drives_correctitud_when_complete` — when
   `reconstructed.status == complete`, the score must use `reconstructed.content`
   not `tool_response.content`.
2. `test_partial_reconstruction_does_not_affect_correctitud` — when
   `reconstructed.status != complete`, the score uses the original response
   (preserves current behavior for H4.0-era scenarios).

## Estado del release

| Gate | Estado actual (post-run nuevo) | Bloqueante |
|---|---|---|
| G4 | RED (0.11 en anyhow) — bug del scoring engine, NO del producto | SÍ (correctivo en R2) |
| G3 | GREEN (97.8 — del run previo, no re-corrido) | no |
| G6 | GREEN | no |

## Implicación para v1.0.0 cut

El run histórico que el operador referencia (H2 GREEN) **sí cerró G4** porque
el scenario `read_source` no estaba en el corpus de H2. Cuando se añadió al
corpus en H4 (con `read_source_full: true`), se asumió incorrectamente que el
scoring usaría `reconstructed.content`. La fix pendiente cierra ese gap.

## Estado de operaciones

- **Patch waiver**: REVERTED ✓
- **Streak ledger**: RECONCILED ✓ (ledger real nunca fue tocado por waiver)
- **H2 campaign identity**: VERIFIED ✓ (entry [8] del ledger)
- **INC-007 failing campaign identity**: VERIFIED ✓ (run 20260919T161019 con binario pre-H4.1)
- **G4 RED root cause**: REPRODUCED ✓ (scoring engine no usa reconstructed.content)
- **Corrective action**: PROPOSED (R2: 10 LOC en score_scenario)
- **Regression tests**: PENDING (post-fix)
- **Next release blocker**: G4 fix pendiente
