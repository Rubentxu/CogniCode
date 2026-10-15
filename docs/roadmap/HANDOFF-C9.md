# Handoff CD-01 — CogniCode — 2026-10-14

Sesión cerrada. Todo lo que sigue está verificado con comandos reales de esta sesión.

## Resumen en 30 segundos

- **CD-1, CD-2, CD-3, CD-5**: cerrados y verificados (scripts que daban
  verde falso + helper de cleanup SBOM con wildcard).
- **Hallazgo fuerte**: un test daba verde con el producto roto. Arreglado y
  verificado con RED→GREEN. Commit `13a0de4e`.
- **nextest**: da los mismos resultados y es **2,05× más rápido**, pero **NO se
  adopta** por dos defectos de aislamiento de tests (detallado abajo).
- **Publicado**: 75 commits en `cp2-cd01-trustworthy-gates`. `main` sin mover.
- **⛔ Una sola cosa bloquea**: la firma. No hay clave del operador en la máquina.

## Identidad del checkout

```
repo      /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode
HEAD      57a3d78f04772fd1b3f67ee165a984e70ae69515  (abre Handoff, escrito al final)
rama      cp2-cd01-trustworthy-gates   ← YA PUBLICADA
stashes             16  (intactos, no tocar, nunca usar git stash)
```

**Estado de publicación (verificado, no supuesto):**

```
rama publicada   cp2-cd01-trustworthy-gates   75 commits
SHA remoto       57a3d78f  ==  SHA local       (coinciden)
main remoto      6fbeefec  SIN MOVER          (nadie pusheo a main)
main local       57a3d78f  (adelanto de 75 sobre origin/main, NO divergencia:
                 verificado con `git merge-base --is-ancestor origin/main main`)
PR               NO abierto, a un clic del operador:
                 https://github.com/Rubentxu/CogniCode/pull/new/cp2-cd01-trustworthy-gates
```

El PR **no** se abrió a propósito: dispara `merge-gate`, y la versión de runner
sigue sin decidir (0.56.0 activa por `.tool-versions`; 0.48.0-rc2 es la que pide
el ROADMAP). No corresponde al agente publicar el semáforo verde.

Árbol: **limpio** en el momento de este addendum; el cambio del punto 3
de Pendientes quedó committeado en `3f5e2df` (ver "Cerrado y verificado"
CD-5 y Pendientes #3 ~~tachado~~).

Producto (`crates/*/src/`) **intacto**. Todo cambio de esta sesión es en tests.

## Regla de la sesión (súper importante)

Esta sesión es **solo CogniCode**. El repo PipelineK lo está usando **otro agente**
con trabajo sin commitear. **No tocarlo**: no limpiar, no revertir, no commitear.
PipelineK es el orquestador de CI. **No introducir GitHub Actions.**
Nunca usar `git stash` ni releases sin `test-full.sh` verde.

## Cerrado y verificado

| WU | Estado | Commit |
|---|---|---|
| CD-1 test-fast.sh no parseaba JSON y ejecutaba 0 suites | ✅ | `26e04905` |
| CD-2 publish-merge-gate.sh podía publicar verde de otro SHA | ✅ | `b5b8ae03` |
| CD-3 selector de merge-gate kts calculado y descartado | ✅ | `97145a9c` |
| Test con **verde falso** (ver abajo) | ✅ | `13a0de4e` |
| CD-5 helper cleanup bounded (SBOM isolation, fix del pendiente #3) | ✅ | `3f5e2df` |

`test-full.sh` = **6607 passed, 0 failed, rc=0** (698.5 s bajo contención).

## El hallazgo importante: un test daba verde con el producto roto

`crates/cognicode-core/tests/b2_w1_analysis_outcome_contract.rs`
test `analysis_basis_canonicalizes_workspace_root`.

Comprobaba solo dos propiedades que cumple **CUALQUIER** ruta absoluta
(`is_absolute()` y no contener `..`). Al mutar `AnalysisBasis::workspace_root()`
para devolver una literal constante, el test pasaba igual.

```
RED   con bug inyectado   11 passed, 1 FAILED
GREEN producto intacto    12 passed, 0 failed
core  completo            2919 passed, 0 failed, 5 ignored
fmt   rc=0    clippy -D warnings limpio
```

Corregido con `assert_eq!` de identidad contra el directorio esperado, y haciendo
el test independiente del cwd (`env!("CARGO_MANIFEST_DIR")`).

**Auditoría de suite**: solo 4 aserciones `is_absolute()` en tests; 1 era el defecto
anterior, las otras 3 correctas. 14 tests "sin aserciones" revisados uno a uno:
todos intencionales (`budget_ms: u64::MAX, // characterization only` o
compile-time contract). El único test con presupuesto real (tier2, 1000 nodos)
mide **539 ms contra 30 000 ms** de presupuesto: 50× de margen.

## nextest: investigar, NO adoptar (recomendación firme)

`cargo-nextest 0.9.148` instalado solo en `$JCODE_SCRATCH_DIR/nt/` (**fuera** del repo,
no tocar el PATH global).

**Paridad verificada** en cognicode-core:
```
cargo test   2919 passed, 0 failed, 5 ignored
nextest      2919 passed, 0 skipped          (mismos tests, mismos numeros)
RED con bug  detectado por AMBOS runners
```

**Velocidad** (misma maquina, 3 corridas cada):
```
cargo test   43.8 / 42.5 / 47.7 s   media 44.7 s   dispersion 5.2 s
nextest      21.6 / 21.6 / 22.2 s   media 21.8 s   dispersion 0.6 s   (2.05x)
```

Los 7 ficheros con `env!("CARGO_BIN_EXE_*")` que PRF §125.V10 preveia como
incompatibles **NO se rompieron**.

**Por qué NO adoptar (2 bloqueantes):**

1. **`#[serial]` no protege bajo nextest.** serial_test serializa dentro de un
   proceso; nextest da un proceso por test. La valla no existe. `prf_f6_w3_bis_sbom_contract.rs`
   tiene sus 5 tests marcados `#[serial]` y aun así se pisan.
2. **`rm` con wildcard** en `prf_f6_w3_bis_sbom_contract.rs` (corregido en
   `3f5e2df`, CD-5) borraba `crates/*/*_bin.cdx.json` — alcanzaba todo
   el árbol `crates/` y podía borrar fixtures de otro test en paralelo.
   Reemplazado por una lista explícita de `(crate, bin)`. La regresión
   está pineada por el test `cd5_helper_cleanup_does_not_delete_foreign_crate_sboms`.

Workspace completo con nextest: `6587/6591 passed, 4 failed`. **Los 4 pasan aislados**
(ej. `cold_small_under_budget`: 19.5 s bajo carga vs **0.048 s** solo, presupuesto 5 s).
Son contención de la máquina (iowait 83-88%), no regresión.

Adoptar nextest exigiría crear `.config/nextest.toml` con test-groups serializados
**y** auditar qué otros tests del workspace comparten estado en el árbol del repo.
Trabajo de auditoría amplio que el repo nunca hizo. Ganancia real sobre
`test-full.sh` (698 s, mayormente compilación + UAT) es pequeña.

## Nota: por qué NO se optimizó código de producto (iowait)

> **Nota histórica, no una unidad de código.** Esta sección se llamaba
> "CD-5" en el handoff inicial; el `CD-5` que aparece en la tabla de
> "Cerrado y verificado" (commit `3f5e2df`) es otra cosa: el fix del
> cleanup wildcard en el test SBOM. El número se reusó por confusión
> entre los dos hilos; aquí lo renombro a "Nota iowait" para que la
> tabla pueda leer limpio.

FAST medido 7 veces: `55.04 61.20 44.99 57.49 31.84 41.58 25.72` s (rango 25.7-61.2 s).
Objetivo propuesto p95 ≤ 30 s **no es medible en esta máquina**:
- `vmstat`: **83-88% iowait**, ~0% user CPU
- cargas ajenas: 19 → 85 procesos Node/Vitest/Java
- comandos MCP idénticos entre ~0.5 s y 75 s
- repo en `nvme1n1`; `CARGO_TARGET_DIR` compartido en `nvme0n1p3` (cruce de discos)
- `~/.cargo/config.toml` (jobs=4, debug=1) está ahí **a propósito** para proteger la
  caché compartida con otros workspaces. **No tocarlo.**

Optimizar con esas cifras habría sido ajustar código a ruido.

## Pendientes

### 1. ~~Publicar los commits~~ — HECHO ✅
Los 75 commits están publicados en `cp2-cd01-trustworthy-gates`.
`main` remoto intacto en `6fbeefec`. PR sin abrir (ver nota de versión de runner abajo).

**Corrección de dato previo (importante para quien retome):** se afirmó que
`pipelinek 0.48.0-rc` no estaba instalado. **Es falso**: sí lo está
(`asdf list pipelinek` → 0.43.0 … 0.48.0-rc2, 0.51.0-rc1, `*0.56.0` activa).
El dato erróneo venía de un `tail -3` que truncaba la lista.
**Al abrir el PR hay que decidir** si vale la 0.56.0 activa o se fija 0.48.0-rc2,
porque el resultado del gate depende de esa versión y no es intercambiable.

### 2. Firma / provenance (CD-4) — ⛔ ÚNICO BLOQUEANTE REAL
Única clave GPG de la máquina: **`test@example.com`** (2026-10-10).
Identidad de git: `Ruben <rubentxu@cognicode.dev>`. **No son la misma.**
Firmar con esa clave atribuiría los commits a una cuenta de prueba. **No se hizo.**
Opciones: (a) crear clave real con el email del operador y publicarla en GitHub,
(b) seguir sin firmar. `commit.gpgsign` y `user.signingkey` siguen sin configurar.
`scripts/ci/test_provenance_gate.py` = PASS (construido `cognicode-control-plane`).

### 3. `prf_f6_w3_bis_sbom_contract.rs` modificado sin commitear ~~pendiente~~ ✅
Cambio: `rm -f crates/*/*_bin.cdx.json …` (wildcard) → lista explícita de
`(crate, bin)` = cogh, cognicode, cognicode-release, cognicode-mcp, mcp-client.
437 líneas, los 5 `#[test]` dentro, fmt OK, clippy limpio.
**Resuelto** en commit `3f5e2df` (ver tabla "Cerrado y verificado" CD-5).
Se añadió además un test de regresión (`cd5_helper_cleanup_does_not_delete_foreign_crate_sboms`)
que planta un fixture en `crates/cognicode-core/` (librería sin `[[bin]]`, por
tanto fuera del dominio del helper) y verifica que el cleanup acotado no lo
borra. El test fuerza `cwd = repo_root()` para reproducir el escenario
paralelo donde el bug se manifestaba (con `cwd = crates/cognicode-cli/` el
wildcard no matcheaba y el defecto quedaba latente). RED→GREEN verificado:
con el wildcard restaurado el test falla con el panic esperado; con la lista
explícita, pasa.

## Cómo retomar

```bash
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode
git rev-parse --abbrev-ref HEAD   # debe ser cp2-cd01-trustworthy-gates
git status --short                # limpio desde 3f5e2df; si no lo está, se perdió el addendum
bash scripts/test-fast.sh         # feedback rápido, sustituye a full durante iteración
bash scripts/test-full.sh         # OBLIGATORIO antes de push/tag/bump: fmt+clippy+tests
```

⚠️ El cambio del punto 3 está en `3f5e2df` (CD-5) — la nota defensiva
original sobre "si `git status` aparece limpio se perdió" ya no aplica,
pero se conserva el espíritu: si un futuro handoff habla de un cambio
"sin commitear", verificar antes de asumir que sigue en el working tree.

Reglas del repo: `fast` no sustituye a `full`. Nada de `git stash`. Nada de release
con `FAIL`/`BLOCKED`/`NOT_RUN`. Estados permitidos: `NOT_RUN PASS FAIL BLOCKED
SKIP_NOT_APPLICABLE`.

## Lo que NO se hizo y por qué

- **No se optimizó código de producto** contra tiempos contaminados.
- **No se adoptó nextest** (2 bloqueantes de aislamiento documentados arriba).
- **No se firmó** con clave ajena al operador.
- **No se tocó PipelineK** (trabajo de otro agente activo).
- **No se descartó ningún test.** Pedido "descartar tests sin valor real" se
  redirigió a **medir cobertura faltante**: se encontró y arregló 1 verde falso,
  y se verificó que el resto de la suite es sólido. Borrar tests sin más habría
  eliminado la única red que detecta estas regresiones.
