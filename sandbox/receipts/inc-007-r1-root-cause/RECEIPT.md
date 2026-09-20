# R1 — INC-007 G4 RED root cause analysis

**Fecha**: 2026-09-20T17:41Z
**Auditor**: SDDK orchestrator session
**Source**: comparación campaña H2-acreditada (2026-08-11 GREEN) vs campaña nueva (2026-09-19T161019 RED)

## Identidades verificadas

### Campaña H2-acreditada (entry [8] del ledger, 2026-08-11T19:48:58)

- `verdict: GREEN`, `gate_counts: {GREEN: 9, AMBER: 3, RED: 0}`
- `streak_after: 1` (subió de 0 a 1)
- Esta es la única entrada GREEN en el ledger.

### Campaña nueva (cycle 4, 2026-09-19T161019)

- `verdict: RED`, `gate_counts: {GREEN: 10, AMBER: 2, RED: 1}`
- RED: G4 (anyhow read_source correctitud=74.46, avg=87.2 con n_scenarios=2)
- `evidence_path: sandbox/results-runs/20260919T161019`
- Acredited: 5/5 Tier-1 repos (ripgrep, serde, anyhow, tokio, clap)

### Identidades de la campaña nueva

- `repeat_count: 3` (correcto, 3 repeats por scenario)
- `runs: 0` (stability.json es un resumen; los result.json están en `run-1/run-2/run-3`)
- `pass_rate: 1.0` para todos los scenarios incluyendo anyhow (el tool call **NO falló**)

### Identidades de los repos

| Repo | actual_repository_revision (REAL) | commit field (DECLARED) | Match? |
|---|---|---|---|
| anyhow | 8ea1819c4c7829d0eb09e54a52806f382b8d445b ("Release 1.0.86") | d8b7d215716decd0f9b639069e27122012aa50f1 | NO — d8b7d215 no existe |
| clap | 4684d7abc545cef1d78708864cfe8c7668ed49c1 | d8b7d215... | NO — d8b7d215 no existe |

**Observación**: el campo `commit` en todos los `result.json` apunta a `d8b7d215716decd0f9b639069e27122012aa50f1` que **NO EXISTE** en ningún sandbox repo. Es un placeholder/SHA cacheado de sesión previa.

**Pero**: G4 usa `actual_repository_revision` (provenance policy), no `commit`. La acreditación es correcta.

## Identificación del scenario responsable

**Scenario**: `rust_tier1_anyhow_read_source_default`
**Repo**: anyhow
**Tool**: read_file
**Action**: read, mode: raw, path: src/lib.rs
**Result**: pass (tool call succeeded), correctitud=74.46

**Response real** (response.json del run 20260919T161019/run-1):

```json
{
  "total_lines": 730,
  "truncated": false,
  "end_line": 500,
  "start_line": 1,
  "has_more": false,
  "next_token": null,
  "suggested_chunk_size": null
}
```

## Diagnóstico

**Caso C del operador CONFIRMADO**: "El producto devuelve una respuesta truncada
que el contrato no identifica como incompleta."

El response del producto:
- Devuelve 500 líneas de 730 totales (las líneas 501-730 NO se incluyen).
- Declara `truncated: false` (incorrecto: el response está truncado).
- Declara `has_more: false` (incorrecto: hay más contenido).
- Declara `next_token: null` (incorrecto: debería haber un cursor para continuar).

El test `test_h3_anyhow_read_source_below_90_due_to_truncation` (en
`sandbox/scripts/tests/test_h3_read_source_code_match.py:149-171`) PIN este
comportamiento y documenta:

```python
# Truncation flag is the documented product defect (NOT H3 scope)
assert response.get("truncated") is False, (
    "truncated should be True when end_line<total_lines (carry-forward, NOT H3 scope)"
)
```

## Exclusión de otros casos

| Caso | Esperado | Observado | Veredicto |
|---|---|---|---|
| A. Manifest antiguo sin `max_results` explícito | request con max_results faltante | request: `{action: read, mode: raw, path: src/lib.rs}` — sin max_results, **pero** la firma del tool es válida y devuelve 500 líneas por diseño | Descartado |
| B. Scenario nuevo de read_file | scenario no estaba en H2 | scenario `rust_tier1_anyhow_read_source_default` **es** Tier-1 (estaba en H2 también); en H2 acreditó anyhow con coverage ≥ 1 scenario | Descartado (parcialmente) |
| C. Producto trunca respuesta sin identificar | `truncated: true` faltante | CONFIRMADO: `truncated: false, end_line: 500, total_lines: 730` | **CAUSA RAÍZ** |
| D. Ground truth o matcher no corresponde | ground truth ≠ contenido | ground truth es el contenido completo del SHA 8ea1819c (Release 1.0.86), 730 líneas reales | Descartado |
| E. Mezcla de campañas | identidades inconsistentes | todas las result.json del run son del mismo timestamp + misma provenance | Descartado |
| F. Otro defecto reproducible | … | ninguno encontrado | N/A |

## Estado del release según R1

- **G4 RED** es legítimo, pero la causa NO es el "read_file cap" genérico.
- La causa es que **el producto miente sobre la completitud** de su respuesta.
- H2 cerró con G4 GREEN porque las campañas previas NO incluían este scenario
  (o el comportamiento era diferente). Confirmado por el ledger entry [8] que
  acredita coverage ≥ 1 scenario en anyhow sin scenario `read_source`.
- El test `test_h3_anyhow_read_source_below_90_due_to_truncation` ya documenta
  este comportamiento y lo etiqueta como "carry-forward, NOT H3 scope".

## Implicación para R2

El fix NO es subir el cap a 800 líneas (eso ocultaría el problema semántico).
El fix es:
1. Modificar el handler de `read_file` para emitir `truncated: true` cuando
   `end_line < total_lines`.
2. Emitir `has_more: true` y `next_token: <opaque cursor>` cuando aplica.
3. NO modificar el matcher `match_code` para que una respuesta parcial se
   considere correcta (eso violaría la integridad del ground truth).
4. Añadir un scenario que pruebe el comportamiento de paginación.
