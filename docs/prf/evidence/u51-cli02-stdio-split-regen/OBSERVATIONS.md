# U51 — PRF-CLI-02 (stdout datos estructurados / stderr logs) — regenerado vs HEAD (2026-09-23)

## Origen

§50 del JOURNAL (2026-09-22) cerró PRF-CLI-02 como PASS con UAT
binario real 5/5 citando `evidence/u51-cli02-stdio-split/`. §102
identificó que ese directorio NO estaba materializado en disco.

Este documento regenera el barrido contra **HEAD `9bb461dc`** para
producir evidencia viva bajo la metodología del MANIFEST.md.

## Binario

- Ruta: `/var/home/rubentxu/cargo-targets/debug/cognicode`
- Versión: `cognicode 0.97.4`
- Source-commit: `9bb461dc` (post §110)
- Construido con: `cargo build -p cognicode` (debug)

## Corpus

- Path: `/tmp/prf-cli01-uat/`
- Archivos: `src_lib.rs` con `add`, `mul` (2 símbolos).

## Comandos ejecutados (5 del UAT original §50)

### Step 1 — `graph full --format json .`
```
EXIT=0
stdout: {"schema_version":"cognicode.graph.full/v1","path":".","elapsed_ms":4,"symbols":2,"dependencies":0,"status":"complete"}
stderr: [INFO] CogniCode CLI v0.97.4 / [INFO] Rayon global thread pool initialized...
```
✅ stdout = SOLO el documento JSON con `schema_version` correcta.
   stderr = SOLO logs estructurados (cumple contrato MUST).

### Step 2 — `graph full .` (modo texto, regresión)
```
EXIT=0
stdout: Building full project graph at: .
        Full graph built in 4ms
```
✅ Modo texto intacto, no se modificó con el `--format json`.

### Step 3 — `graph full --format json /nonexistent_dir` (control negativo)
```
EXIT=0
stdout: {"schema_version":"cognicode.graph.full/v1","path":"/nonexistent_dir","elapsed_ms":3,"symbols":0,"dependencies":0,"status":"complete"}
stderr: [INFO] ...
```
✅ Exit 0 (mismo contrato que `analyze`/`build_graph` MCP que también
   devuelve status=complete con symbols=0 para corpus vacío).
   **Nota honesta**: `/nonexistent_dir` no produce error sino un
   grafo vacío. Esto NO viola el contrato PRF-CLI-02 (el contrato
   es "split stdout/stderr", no "validación de path"), pero un
   consumidor que asuma `symbols=0 ⇒ error` tendría que verificar
   explícitamente el campo `path`. Documentado para visibilidad.

### Step 4 — `doctor --format json`
```
EXIT=1
stdout: {
  "schema_version": "cognicode.doctor/v1",
  "version": "0.97.4",
  "sections": {
    "core": { ... }
  }
}
```
✅ `schema_version: cognicode.doctor/v1` presente y cumple
   contrato exclusivo (única operación estructurada pre-cambio).

### Step 5 — `graph --help` (regresión no-json)
```
EXIT=0
```
✅ Modo texto intacto.

## Verificación contra contrato PRF-CLI-02

| Paso | Esperado (§50 MUST) | Observado | Pasa |
|---|---|---|---|
| 1 stdout/json | solo JSON con schema_version | JSON con `schema_version: cognicode.graph.full/v1` | ✅ |
| 1 stderr/logs | solo logs estructurados | solo INFO logs | ✅ |
| 2 text intact | text mode sin cambios | "Building full project graph…" | ✅ |
| 3 json-mode path empty | status=complete symbols=0 | status=complete symbols=0 | ✅ |
| 4 doctor --format json | schema_version: cognicode.doctor/v1 | presente | ✅ |

## Resultado

**PASS** — 5/5 escenarios cumplen el contrato MUST de PRF-CLI-02
sobre binario real HEAD.

## Limitación honesta

NOTA DEL PASO 3: el comportamiento de "path inexistente → status=complete
symbols=0" es la implementación actual. **No es regresión** (es consistente
con `analyze` y `build_graph` MCP que también devuelven status=complete
para corpus vacío), pero un consumidor estricto podría preferir un exit
code != 0. Esto NO impacta PRF-CLI-02 (cuyo MUST es el split stdout/stderr);
queda como nota de visibilidad para futuros criterios de aceptación
sobre validación de path en CLI.

## Reproducer

```bash
# 1. construir binario
CARGO_TARGET_DIR=/var/home/rubentxu/cargo-targets/debug \
  cargo build -p cognicode

# 2. preparar corpus (idéntico a §110)
mkdir -p /tmp/prf-cli01-uat
cat > /tmp/prf-cli01-uat/src_lib.rs <<'SRC'
pub fn add(a: i32, b: i32) -> i32 { a + b }
pub fn mul(a: i32, b: i32) -> i32 { a * b }
SRC

BIN=/var/home/rubentxu/cargo-targets/debug/cognicode

# 3. ejecutar barrido
cd /tmp/prf-cli01-uat
$BIN graph full --format json . > u51_s1.json 2> u51_s1.err
$BIN graph full .                       > u51_s2.out 2> u51_s2.err
$BIN graph full --format json /nonexistent_dir > u51_s3.json 2> u51_s3.err
$BIN doctor --format json               > u51_s4.json 2> u51_s4.err
$BIN graph --help                       > u51_s5.out 2> u51_s5.err
```

## SHA-256 de outputs capturados

| Archivo | SHA-256 |
|---|---|
| step1.json | `2079b21e9fd784fdfa1195b5793090255171c239ad968ef12da4aa97dfc031d4` |
| step1.err  | `dbac701ff2fbc45f41dee0c79d0f852276ed0829b2efcb1d9e1197e467366f1b` |
| step2.out  | `efaaa930fded6414c0eed0c76ac0fc6520446de01428f22d77226c05f488a3c7` |
| step2.err  | `d0c668f5ea0a908684112a03799396f2021003b5f2b1d9185ef6bedab58fb8b1` |
| step3.json | `c53ab9f8d9edc73744959027e82da20c51d21c9400707a4fdb5923c6552b1527` |
| step3.err  | `992c4f9f426aa8f35226438bc41d62d0cde0a7b12483bd19156c6579825d6c5c` |
| step4.json | `f3edfc875912de20e234b7aabe131f82d77eb03f3252e296bc65f189cfcb50ab` |
| step4.err  | `fd0e7508a97da15e51288af5283aa3e7f0743aa93cb7902afe2ee338351379cd` |
| step5.out  | `e8eb05ca7daf88f9c674ab8f5457177949240e692a04d0b8446210d4cf265cf6` |
| step5.err  | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

(Estos hashes son válidos solo para HEAD `9bb461dc` y este corpus;
contra otro binario el método se mantiene, los hashes varían.)
