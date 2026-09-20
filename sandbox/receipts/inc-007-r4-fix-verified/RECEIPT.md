# INC-007 R4 — RECEIPT de fix verificada

**Fecha**: 2026-09-20T18:39Z
**Operador-decisión**: GO R4 (autorización Opción A del R2 vía TDD).
**HEAD medido**: `e65fbc8d` (cycle 4 E31-B9..B12), 1 commit ahead of `origin/main d2077f02`.
**Binario de producto**: `sandbox-orchestrator` sha256 `5a103c7258e63789087e3fe753708f45d79e6e13eaf6d6e97d2d8911f9f3f818` (4760560 bytes, mtime 2026-09-20T20:31+0200).

---

## 1. Root cause (R1)

Tres causas superpuestas del G4 RED en campañas previas:

1. **Binario pre-H4.1** emitía `truncated: false` mintiendo (carry-forward de bug ya corregido en H4.1).
2. **H4.3 reconstruction funciona** (verificado: `sha256_recon == sha256_disk`, 21209 bytes para anyhow, 25075 para tokio).
3. **Bug semántico del ground_truth**: el `match_code` legacy calculaba **Jaccard similarity** entre el contenido reconstruido completo (~21 KB) y un snippet esperado de 1 línea (~17 bytes). Resultado: similarity ≈ 0.08–0.43 → correctitud < 90 → G4 RED.

Receipt previo: `sandbox/receipts/inc-007-r1-root-cause/RECEIPT.md`.

## 2. Contract decision (R4.1)

Opción A del R2, aplicada sin desviaciones:

- Nuevo campo opcional en `ExpectedCode`: `contains: Option<String>` con `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- `ExpectedCode.content` ahora con `#[serde(default)]` (para manifests que solo declaran `contains`).
- Nuevo campo opcional en `CodeMatchResult`: `error: Option<String>` para reportar ground-truth inválido o ambiguo.
- `match_code` 3-way dispatch: `content` (legacy Jaccard), `contains` (substring presence), `content + contains` (ambiguo → error).
- Modalidad `content` previa **preservada verbatim** — sin cambio de comportamiento para manifests que solo declaran `content`.

### Tabla de semántica `contains`

| Estado | `exact_match` | `content_similarity` | `error` |
|---|---|---|---|
| `contains` Some(fragment), fragment in `returned_content` | true | 1.0 | None |
| `contains` Some(fragment), fragment NOT in content | false | 0.0 | None |
| `contains` Some("") | false | 0.0 | Some("invalid ground_truth.code: `contains` is empty") |
| `content` + `contains` ambos Some | false | 0.0 | Some("ambiguous ground_truth.code: `content` and `contains` are mutually exclusive") |
| `contains` Some, response `truncated=true` | false | 0.0 | Some("truncated response without continuation: cannot verify `contains` over partial content") |
| multi-file `files[]` array | path-matches `expected.file` → substring check | (otherwise: no match) | — |

Garantías explícitas:
- **No Jaccard** en modo `contains` (substr only).
- **Multi-file safe**: fragment debe estar en el archivo cuyo path matches `expected.file`; presencia en archivo sibling NO acredita.
- **Truncation safety**: respuesta truncada sin continuación válida → error, NO acredita (la página parcial podría contener el fragment por casualidad).
- **Empty fragment** = ground-truth inválido (rechazado explícitamente).
- **Ambigüedad** (`content` + `contains`) = error explícito, NO resolución silenciosa.
- **Compatibilidad**: el campo `content` legacy sigue funcionando idénticamente.

NO introducido (per cláusulas contractuales):
- Matcher genérico nuevo.
- Arquitectura paralela de scoring.
- Cambios a `read_file` pagination / `truncated`/`has_more`/`next_token`.
- Cambios al H4.3 reconstructor.
- Cambios a G3/G4 thresholds.
- Cambios a `streak.py` o `V1.0.0-PRE-CUT-CHECKLIST.md`.

## 3. RED tests (R4.2)

8 tests escritos PRIMERO en `crates/cognicode-core/src/sandbox_core/ground_truth.rs` (lines 2199+), usando la constante `ANYHOW_FIRST_LINE = "//! [![github]](https://github.com/dtolnay/anyhow)"` para reproducir el escenario real (no strings artificiales):

| # | Test | Cubre |
|---|---|---|
| 1 | `test_match_code_contains_21kb_file_with_fragment` | 730 líneas (≈21 KB) con fragmento al inicio → 100 |
| 2 | `test_match_code_contains_21kb_file_without_fragment` | 730 líneas sin fragmento → 0 |
| 3 | `test_match_code_contains_fragment_only_in_second_page` | Fragmento en línea 600 (página 2) tras reconstrucción completa → 100 |
| 4 | `test_match_code_contains_truncated_response_does_not_count_as_complete` | Response con `truncated=true, has_more=true` → error "truncated response without continuation" |
| 5 | `test_match_code_contains_empty_fragment_is_invalid_ground_truth` | `Some("")` → error "invalid ground_truth.code: `contains` is empty" |
| 6 | `test_match_code_content_and_contains_simultaneously_is_ambiguous` | Ambos campos Some → error "ambiguous ground_truth.code" |
| 7 | `test_match_code_content_modality_preserved` | Modalidad `content` legacy intacta |
| 8 | `test_match_code_contains_fragment_in_wrong_file_does_not_credit` | Multi-file: fragmento en sibling NO acredita el archivo objetivo |

Plus 2 tests legacy (`test_match_code_exact`, `test_match_code_partial`) ajustados para incluir `contains: None`.

### Verificación RED→GREEN

**Antes de R4.3 implementación** (tests RED):
```
$ cargo test -p cognicode-core --lib sandbox_core::ground_truth::tests::test_match_code
error[E0063]: missing field `contains` in initializer of `ground_truth::ExpectedCode`
error[E0063]: missing field `contains` in initializer of `ground_truth::ExpectedCode`
[8/8 RED tests fail to compile, confirming the contract is enforced]
```

**Después de R4.3 implementación**:
```
running 10 tests
test sandbox_core::ground_truth::tests::test_match_code_contains_empty_fragment_is_invalid_ground_truth ... ok
test sandbox_core::ground_truth::tests::test_match_code_contains_fragment_in_wrong_file_does_not_credit ... ok
test sandbox_core::ground_truth::tests::test_match_code_content_and_contains_simultaneously_is_ambiguous ... ok
test sandbox_core::ground_truth::tests::test_match_code_contains_truncated_response_does_not_count_as_complete ... ok
test sandbox_core::ground_truth::tests::test_match_code_contains_21kb_file_with_fragment ... ok
test sandbox_core::ground_truth::tests::test_match_code_contains_21kb_file_without_fragment ... ok
test sandbox_core::ground_truth::tests::test_match_code_contains_fragment_only_in_second_page ... ok
test sandbox_core::ground_truth::tests::test_match_code_content_modality_preserved ... ok
test sandbox_core::ground_truth::tests::test_match_code_partial ... ok
test sandbox_core::ground_truth::tests::test_match_code_exact ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 2099 filtered out
```

10/10 GREEN (8 nuevos + 2 legacy preservados).

### Integration test (against real reconstructed content)

`crates/cognicode-core/tests/inc007_integration.rs` carga el `reconstructed.json` real de anyhow (21209 bytes) de la campaña y verifica que `score_scenario` retorna `correctitud: Some(100.0)`:

```
running 1 test
correctitud: Some(100.0)
code_match: Some(CodeMatchResult {
    exact_match: true,
    content_similarity: 1.0,
    returned_content: Some("//! [![github]]...(21209 bytes)..."),
    expected_content: Some("//! [![github]]"),
    has_docstring: false,
    error: None,
})
test integration_score_anyhow_contains_with_reconstructed_response ... ok
```

### Tests sin regresión

```
$ cargo test -p cognicode-core --lib sandbox_core
test result: ok. 148 passed; 0 failed; 0 ignored
```

148/148 sandbox_core tests verde — sin regresión en callers.

## 4. Implementation commit (R4.3 + R4.4)

Cambios working tree (sin commit todavía, pendientes de push):

### `crates/cognicode-core/src/sandbox_core/ground_truth.rs`

- `ExpectedCode.content: String` ahora `#[serde(default)]`.
- `ExpectedCode.contains: Option<String>` añadido con `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- `CodeMatchResult.error: Option<String>` añadido.
- `match_code()` extendido con 3-way dispatch (ambiguity → error; contains → substring + truncated gate + multi-file path; content legacy preservado).

### `sandbox/manifests-tier1/tier1_h3_read_source.yaml`

5 scenarios read_source cambiados de `content:` a `contains:`:

| Scenario | Fragment |
|---|---|
| `tier1_serde_read_source` | `//! # Serde` |
| `tier1_ripgrep_read_source` | `/*!` |
| `tier1_anyhow_read_source` | `//! [![github]]` |
| `tier1_tokio_read_source` | `#![allow(` |
| `tier1_clap_read_source` | `// Copyright` |

Header del manifest documenta el R4.x INC-007 schema.

### `crates/cognicode-core/tests/inc007_integration.rs` (new, untracked)

Integration test que verifica end-to-end el fix contra el `reconstructed.json` real de anyhow.

### Diff stats

```
$ git diff --stat HEAD -- crates/cognicode-core/src/sandbox_core/ground_truth.rs sandbox/manifests-tier1/tier1_h3_read_source.yaml
crates/cognicode-core/src/sandbox_core/ground_truth.rs               | 443 ++++++++++++++++++++-
sandbox/manifests-tier1/tier1_h3_read_source.yaml                     |  24 +-
2 files changed, 459 insertions(+), 8 deletions(-)
```

## 5. Fresh runtime evidence (R4.4)

### Build del binario de producto

```
$ touch crates/cognicode-runtime/src/main.rs && cargo build -p cognicode-runtime --release --bin explorer-api
    Finished `release` profile [optimized] target(s) in 1m 48s

$ sha256sum /var/home/rubentxu/cargo-targets/release/explorer-api
491265568bd734ec6b41165978042bdd23daef707a0ad1d8359794867336fc15

# El binario que ejecuta el scoring (match_code) es sandbox-orchestrator, ya reconstruido en R4:
$ sha256sum /var/home/rubentxu/cargo-targets/release/sandbox-orchestrator
5a103c7258e63789087e3fe753708f45d79e6e13eaf6d6e97d2d8911f9f3f818
```

### Escenario anyhow aislado, desde el binario real, contra SHA fijado

```
$ /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode/target/release/sandbox-orchestrator \
    -v run /tmp/inc007-r4-anyhow-only.yaml --results-dir /tmp/inc007-r4-anyhow

[1/1] Running scenario: rust_tier1_anyhow_read_source_default
  [H4.3] Following continuation chain for rust_tier1_anyhow_read_source_default
  [H4.3] Reconstruction status: complete (2 pages, 21209 bytes, 0 ms)
  [SCORING] read_file corr=100.0 lat=100.0 esc=100.0 con=95.0 rob=100.0
  Result written to: /tmp/inc007-r4-anyhow/rust_tier1_anyhow_read_source_default/20260920T183823/result.json
```

Verificación de los 8 criterios R4.4 sobre el anyhow aislado:

```
truncated         = True         (expected: True)
has_more          = True         (expected: True)
reconstruction    = complete     (expected: complete)
pages             = 2            (expected: 2)
total_bytes       = 21209        (expected: 21209)
sha256_recon      = 1c774243700f38ccaced1609c9e37a25c01f5e8aa900b876aef206207d6e4846
sha256_disk       = 1c774243700f38ccaced1609c9e37a25c01f5e8aa900b876aef206207d6e4846
sha256_match      = True         (expected: True)
fragment_in_recon = True         (expected: True)
correctitud       = 100.0        (expected: 100)
outcome           = pass         (expected: pass)

ALL_R4_4_CRITERIA  = True
```

### Los otros 4 Tier-1 read_source, desde el binario real

```
$ /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode/target/release/sandbox-orchestrator \
    -v run /tmp/inc007-r4-others.yaml --results-dir /tmp/inc007-r4-others

[1/4] Running scenario: rust_tier1_serde_read_source_default
  [SCORING] read_file corr=100.0 lat=100.0 esc=100.0 con=95.0 rob=100.0
[2/4] Running scenario: rust_tier1_ripgrep_read_source_default
  [SCORING] read_file corr=100.0 lat=100.0 esc=100.0 con=95.0 rob=100.0
[3/4] Running scenario: rust_tier1_tokio_read_source_default
  [H4.3] Following continuation chain for rust_tier1_tokio_read_source_default
  [H4.3] Reconstruction status: complete (2 pages, 25075 bytes, 0 ms)
  [SCORING] read_file corr=100.0 lat=100.0 esc=100.0 con=95.0 rob=100.0
[4/4] Running scenario: rust_tier1_clap_read_source_default
  [SCORING] read_file corr=100.0 lat=100.0 esc=100.0 con=95.0 rob=100.0
```

### Tabla R4.4 completa (5/5 Tier-1 read_source)

```
scenario                            outcome   corr  sha_match pages   bytes
--------------------------------------------------------------------------------
tier1_serde_read_source             pass     100.0 single_page     1   13612
tier1_ripgrep_read_source           pass     100.0 single_page     1   11698
tier1_anyhow_read_source            pass     100.0       True     2   21209
tier1_tokio_read_source             pass     100.0       True     2   25075
tier1_clap_read_source              pass     100.0 single_page     1    1666
```

5/5 pass + correctitud=100. Anyhow y tokio con `sha256_recon == sha256_disk` y reconstruction de 2 páginas. Serde, ripgrep, clap en single_page (no requieren cadena de continuación; la primera página contiene el archivo entero).

## 6. Corrected G4 verdict (R4.4)

```
$ python3 sandbox/scripts/release_scorecard.py --runs /tmp/inc007-r4-combined --output /tmp/g4_r4.json

Release Readiness Scorecard
  Generated: 2026-09-20T18:39:04Z
  Gates: 13
  GREEN: 9  AMBER: 4  RED: 0
```

G4 detail:

```json
{
  "id": "G4",
  "name": "Corpus Quality / Correctitud",
  "status": "GREEN",
  "measured": "100.0 avg across 5/5 tier1 repos",
  "budget": 90.0,
  "evidence_path": "/tmp/inc007-r4-combined",
  "evidence_text": "acredited=5/5 tier1_repos; candidates=5; unverified=0; anyhow: avg=100.0 (n_scenarios=1, threshold=90); clap: avg=100.0 (n_scenarios=1, threshold=90); ripgrep: avg=100.0 (n_scenarios=1, threshold=90); serde: avg=100.0 (n_scenarios=1, threshold=90); tokio: avg=100.0 (n_scenarios=1, threshold=90)"
}
```

**G4 GREEN** verificado con evidencia fresh runtime (binario real, Tier-1 repos acreditados). No queda ningún RED adicional en la población obligatoria de G4.

### Tests de contrato G4

```
$ pytest sandbox/scripts/tests/test_a1a_g4_contract.py
24 passed in 0.62s

$ pytest sandbox/scripts/tests/test_preflight_v1.py
14 passed in 0.30s
```

### Lint

```
$ cargo clippy -p cognicode-core --lib -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 09s
```

Clean.

## 7. Remaining blockers

### Para v1.0.0 cut

- **Push a origin**: el commit `e65fbc8d` (cycle 4 E31-B9..B12) y el fix R4 están pendientes de push. El operador autorizó "push fast-forward una vez comprobado que son compatibles con el estado remoto y que no incluyen el waiver revertido" — pero esta autorización es para cuando el operador decida, **no se ejecuta en este ciclo**.
- **`--record` para streak**: la cláusula R4.5 lo prohíbe explícitamente ("No crear release ni incrementar el streak"). El contador queda en `current_streak: 0, verdict: RESET`.
- **Tag bump a v1.0.0**: bloqueado por la política existente que requiere streak counter (pre-cut checklist). Se autoriza una nueva campaña para validar el fix end-to-end antes del bump.
- **G2/G5/G6/G8 AMBER**: pre-existentes (coverage_matrix.yaml missing, no latency data for other families, no stability.json, no g8-probe). Sin relación con INC-007. No son bloqueadores de INC-007.

### Estado del ledger (R4.7)

```
current_streak: 0
verdict: RESET
```

Sin cambios desde R0. No se ha ejecutado `--record` durante R0–R4.

## 8. Cierre de INC-007

**INC-007 puede cerrarse como FIXED**:

- Root cause reproducido y documentado (R1).
- Contract decision tomada (R4.1).
- 8 RED tests escritos primero + 1 integration test (R4.2).
- Implementación mínima (R4.3).
- Fresh runtime evidence desde el binario real, contra Tier-1 repos (R4.4).
- G4 GREEN verificado con scorecard diagnóstico (R4.4).

La corrección se aplicó **sin waiver**, sin relajar gates, sin tocar paginación/H4.3/thresholds, y el binario de producto incluye el fix.

---

**NO se hace push en este ciclo** (autorización del operador es "una vez comprobado que son compatibles con el estado remoto", pendiente de su decisión).

**NO se hace `--record`** (cláusula R4.5 explícita).

**NO se inicia otro ciclo automáticamente** (cláusula STOP del operador: "El siguiente trabajo será una campaña nueva, completa e independiente, utilizando los binarios corregidos y los contratos definitivos. Su autorización se decidirá a partir del cierre de INC-007").
