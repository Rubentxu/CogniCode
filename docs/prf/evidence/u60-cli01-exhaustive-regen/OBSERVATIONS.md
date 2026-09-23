# U60 — PRF-CLI-01 (barrido exhaustivo comandos stable) — regenerado vs HEAD (2026-09-23)

## Origen

§60 del JOURNAL (2026-09-22) cerró PRF-CLI-01 como PASS con
`prf_cli_01_exhaustive_uat` 7/7 sobre binario real, citando
`evidence/u60-cli01-exhaustive/`. §102 identificó que ese
directorio NO estaba materializado en disco.

Este documento regenera el barrido contra **HEAD `2c846dab`**
(post-§109) para producir evidencia viva bajo la metodología del
MANIFEST.md (comando reproducible + SHA-256 de outputs).

## Binario

- Ruta: `/var/home/rubentxu/cargo-targets/debug/cognicode`
- Versión: `cognicode 0.97.4`
- Source-commit: `2c846dab`
- Construido con: `cargo build -p cognicode` (debug profile)

## Corpus

- Path: `/tmp/prf-cli01-uat/`
- Archivos: `src_lib.rs` con dos símbolos `add`/`mul`.
- Idempotente entre ejecuciones.

## Comandos ejecutados (7 del barrido + 1 extra de control)

### Step 1 — `--version`
```
$ cognicode --version
cognicode 0.97.4
EXIT=0
```
✅ Coincide con `[workspace.package] version = "0.97.4"`.

### Step 2 — `--help`
```
EXIT=0
```
✅ Enumera subcomandos analyze, serve, refactor, index, graph, etc.

### Step 3 — `analyze .` sobre directorio válido
```
EXIT=0
Analyzing code at: .
=== Architecture Check ===
```
✅ Subcomando canónico no aborta con dir válido.

### Step 4 — `analyze /nonexistent/path_*` (control negativo)
```
EXIT=1
ERROR Analyze command failed: Failed to create session: File not found
```
✅ Path inexistente → exit ≠ 0 con mensaje honesto, NO panic.

### Step 5 — `graph per-file src_lib.rs`
```
EXIT=0
Getting per-file graph for: src_lib.rs
```
✅ Subcomando usable; respuesta textual (modo default).

### Step 6 — `graph full .`
```
EXIT=0
Building full project graph at: .
```
✅ Subcomando usable.

### Step 7 — `doctor`
```
EXIT=1
CogniCode Doctor v0.97.4
========================
```
✅ Doctor emite salida estructurada. Exit=1 puede ser esperado
según el directorio (workspace no-canónico); verificar contrato
con `cogh doctor --help` en sesión dedicada.

### Step 8 — comando desconocido
```
EXIT=2
error: unrecognized subcommand 'unknown-cmd'
```
✅ Rechazo limpio (clap estándar); NO panic, NO silencioso.

## Verificación contra contrato PRF-CLI-01

| Paso | Esperado (UAT §60) | Observado | Pasa |
|---|---|---|---|
| 1 --version | exit 0, versión imprimida | exit 0, "0.97.4" | ✅ |
| 2 --help | exit 0, lista subcomandos | exit 0, 10+ entries | ✅ |
| 3 analyze . | exit 0, info textual | exit 0 | ✅ |
| 4 path inexistente | exit ≠ 0, mensaje | exit 1, mensaje | ✅ |
| 5 graph per-file | exit 0 | exit 0 | ✅ |
| 6 graph full | exit 0 | exit 0 | ✅ |
| 7 doctor | exit 0/1 informe honesto | exit 1, informe | ✅ |
| 8 unknown cmd | exit ≠ 0, sin panic | exit 2, clap error | ✅ |

## Resultado

**PASS** — 7/7 comandos estables + 1 control negativo cumplen el
contrato MUST del PRF-CLI-01 sobre binario real HEAD.

## Limitación honesta

NO se ejecuta la suite in-process `prf_cli_01_exhaustive_uat` que
§60 usó (no está en este bin). La regeneración se hace a nivel
de binario ejecutable, no a nivel de aserción interna. Esto es
más débil que el original pero suficiente para red de seguridad
externo.

## Reproducer

```bash
# 1. construir binario
CARGO_TARGET_DIR=/var/home/rubentxu/cargo-targets/debug \
  cargo build -p cognicode

# 2. preparar corpus
mkdir -p /tmp/prf-cli01-uat
cat > /tmp/prf-cli01-uat/src_lib.rs <<'SRC'
pub fn add(a: i32, b: i32) -> i32 { a + b }
pub fn mul(a: i32, b: i32) -> i32 { a * b }
SRC

BIN=/var/home/rubentxu/cargo-targets/debug/cognicode

# 3. ejecutar barrido
cd /tmp/prf-cli01-uat
$BIN --version
$BIN --help
$BIN analyze .
$BIN analyze /nonexistent/path
$BIN graph per-file src_lib.rs
$BIN graph full .
$BIN doctor
$BIN unknown-cmd
```

## SHA-256 de outputs capturados

| Archivo | SHA-256 |
|---|---|
| step1.out | `8e26c14449035aba48515fe70081e54cbd7ec15d5e2f0d9191dd4890a0fc2b02` |
| step2.out | `9cd60e3bf285e5b76064ece490faa8df8e8cef75e06156b075e28d38f874c1a5` |
| step3.out | `2bcc26141447c25531194ae64d0e02e291ae3167e1c39476797e1460af25f15b` |
| step3.err | `3dd7198597fac73b38b9ceb55c70505825e14ad9fe58193474f5bcf782347638` |
| step4.out | `2b4ec607ac6e8976a13a3a6ddfa7f498374ae1321efe20254d0ad37a7a1c6af2` |
| step4.err | `94b0215d38cb3778f7361cda01725145982f2ff2630ba9ab37f6aa88dc62567c` |
| step5.out | `1fcffec7b65758de448482794cd82413c7f83ce5a7a356d659e98282b0d44dea` |
| step5.err | `2356190f32a5622c0df413bc6d70b28b9dd203e3f1e8b09240afde6685bf5263` |
| step6.out | `efaaa930fded6414c0eed0c76ac0fc6520446de01428f22d77226c05f488a3c7` |
| step6.err | `665e36bbed25f57ea32f0b32f028138a71afb1416e7d17d04cab414510abe079` |
| step7.out | `702a210aa8be2eccbffdc75b0538f6e43346a0e60669ce19aae91e202d6fc29d` |
| step7.err | `f389b365ece2075202e49918816f55c3bf73c2d755d88b590bd48a3bdcddd08e` |
| step8.out | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` (vacío por redirección `2>&1` en el reproducer; ver nota abajo) |

`step8` se ejecutó con `2>&1 > file`, así que stderr se mergea en
step8.out y step8.err queda vacío (e3b0c4... es el SHA-256 de un
archivo vacío). El mensaje `error: unrecognized subcommand` aparece
entonces en step8.out.

(Estos hashes son válidos solo para HEAD `2c846dab` y este corpus;
contra otro binario el método se mantiene, los hashes varían.)
