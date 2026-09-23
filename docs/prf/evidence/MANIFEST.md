# PRF Evidence Manifest — Raw run outputs (no versionados en Git)

> Este manifiesto es **la fuente de verdad para las evidencias crudas**
> (strace logs, JSON-RPC frames, capturas de stdout/stderr, resultados de
> `cargo test`) que residen solo en esta máquina. La política del
> repositorio (`.gitignore` línea 135) excluye `docs/` para evitar
> inflar el árbol con datos brutos; este manifiesto permite auditar la
> integridad de las copias locales y comprobar reproducibilidad.

**Fecha del manifiesto**: 2026-09-23 (actualizado post H-06 / pre-release T4)
**HEAD al cierre del manifiesto**: `ed1ed09c`
**Operador**: jcode-orchestrator

## Política

- **Versionado en Git** (sí entra al repo): documentos `.md` del programa
  PRF (STATE, JOURNAL, ROADMAP, CERTIFICATION, UAT, TEST-PLAN, README,
  TRACEABILITY, evidence/CERTIFICATES).
- **No versionado en Git** (queda solo local): evidencia cruda en
  `docs/prf/evidence/F0-W2-runs/` y `docs/prf/evidence/F0-W3-runs/`.
- **Reproducibilidad**: cada evidencia tiene el comando que la generó
  documentado en el markdown correspondiente (F0-W2-runtime.md,
  F0-W3-baseline.md). Un auditor puede regenerarla desde cualquier clone
  del repositorio en el mismo entorno (mismo SO, mismas versiones de
  dependencias).
- **Verificación de integridad**: este manifiesto contiene SHA-256 de
  cada archivo; `sha256sum` sobre los archivos locales debe coincidir.

## Evidencias no versionadas — `docs/prf/evidence/F0-W2-runs/`

**Total**: 63 archivos, ~5,2 MB.
**Origen**: caracterización runtime ejecutada durante F0.W2 (2026-09-21).
**Descripción**: ver `docs/prf/evidence/F0-W2-runtime.md`.

| Archivo | SHA-256 | Tamaño (bytes) | Categoría |
|---|---|---|---|
| `analyze_empty.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `analyze_empty.out` | `d64b934ef6bc3c3cd73182f8e691bbaaa1eb926c35947e2e1f8aa38be1d8b80b` | 1158 | stdout |
| `cogh_doctor.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `cogh_doctor.out` | `504ba106b0c0b6a6daca365bfabf41d7c826c7b5caf975d014d7b91675241c88` | 565 | stdout |
| `cogh_help.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `cogh_help.out` | `59c2b66d9c0c43357e9095a69b5466ee1f90f8f12cd9dffc17c52ba689ee3e7f` | 1094 | stdout |
| `cogh_help.time` | `a8747bbf650070a26ec4dc37336afe142885c071d605abf46073f16e50b84a22` | 45 | tiempo de ejecución |
| `cogh_init.err` | `5e552632d2cc68be568d178481d97735b345ef9a50747fe7e71ff43641a0dee5` | 262 | stderr |
| `cogh_init.out` | `e256fb5c3be0133d4b34b4b31b6ad8a937ce15fbf2176b4a9aa8d13cb04aa379` | 78 | stdout |
| `cogh_strace.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `cogh_strace.log` | `a6fb48867b3372ea087c0bec002212d4b59f4af4a216902c06c5790fa1001f9f` | 8736 | strace |
| `cogh_strace.out` | `0dfe1bcfc3786078743174ba6d4d60f6c66de6091d6dca689ce23327bab2c93d` | 12 | strace output filtrado |
| `cogh_--version.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `cogh_version.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `cogh_--version.out` | `0dfe1bcfc3786078743174ba6d4d60f6c66de6091d6dca689ce23327bab2c93d` | 12 | stdout |
| `cogh_version.out` | `31af02e19061af77deb7dbc6c688584ac38839037d700b9d0b0c83c2d9f41111` | 53 | stdout |
| `cogh_--version.time` | `7778673631089a737b095bd76b5a5711e2a2380807977e2228ea8917f87e393f` | 45 | tiempo de ejecución |
| `cogh_version.time` | `d5aeac583f42304872ca32e6f3455cd3d560a3cc895188f411a9d7c838d2d4a3` | 45 | tiempo de ejecución |
| `cognicode_analyze.err` | `8a52df95bd694a0fd52200908b366f89d73266c6877c5a46e8fd52c2563edd3b` | 87 | stderr |
| `cognicode_analyze.out` | `6094a418bbb78d0839a1eeabcbd75e4840a0d64f7ce2fe0d45d40053f4b26740` | 231 | stdout |
| `cognicode_help.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `cognicode_help.out` | `9cd60e3bf285e5b76064ece490faa8df8e8cef75e06156b075e28d38f874c1a5` | 1200 | stdout |
| `cognicode_help.time` | `7dc60010c88d1329f2641796b9b16cdbd96373fefc8cf8da8664497ee71d8274` | 45 | tiempo de ejecución |
| `cognicode-mcp_help.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `cognicode-mcp_help.out` | `dbd465038447d624a9362d16eceee8cb6ebdf6ff7119392c11779fcda1324eb7` | 158 | stdout |
| `cognicode-mcp_help.time` | `f3f94a19ea1f08f414d9f3438f3e0a3f3b1d778dee7b23fd1b5137a70a25ea4a` | 45 | tiempo de ejecución |
| `cognicode-mcp_strace.err` | `3c92032c24f06fcd45e8427c6a93befeebf6b2525bdcde7e2c949db99843f51c` | 1621 | stderr |
| `cognicode-mcp_strace.log` | `369f87b9a52c0b2e52256158963b0ffc2c5feb55847cfa7563ab741c61b0feb4` | 871138 | strace |
| `cognicode-mcp_strace.out` | `1d9229194e58efdc293a6d1e388bc1966362f7d020afc5ba4e092a662a092878` | 11782 | strace output filtrado |
| `cognicode-mcp_--version.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `cognicode-mcp_--version.out` | `6fea5b27ad9a6a93c94ce92926084548fafd0d942bb82731f3f646fbe93b2de4` | 21 | stdout |
| `cognicode-mcp_--version.time` | `35010e0207a5b362f4105b58c32986f1d780227ec152cbccc7ca0ef1c2ebf729` | 45 | tiempo de ejecución |
| `cognicode_--version.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `cognicode_--version.out` | `23746f57b616d443eb83aef9b6e5f7ef7100f2bc2509bf28a15e2e6e864602a0` | 17 | stdout |
| `cognicode_--version.time` | `acb51d1e76e021d51e45471e8a070aa837e76636f22595475108a066135cda96` | 45 | tiempo de ejecución |
| `doctor.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `doctor.out` | `504ba106b0c0b6a6daca365bfabf41d7c826c7b5caf975d014d7b91675241c88` | 565 | stdout |
| `expl_api_term.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `expl_api_term.out` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stdout |
| `explorer-api_bg.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `explorer-api_bg.out` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stdout |
| `explorer-api_health.out` | `7a5d0ba1669ceb3aba35ca2c1e59dec595baf4e50ff97079d8d0532758d3b8a2` | 46 | stdout |
| `explorer-api_help.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `explorer-api_help.out` | `10675f667431ffcb156d74d1a1724c0d7529bdec447b93660cf0777dbff7a334` | 1017 | stdout |
| `explorer-api_help.time` | `2ac4bb51f451b11f506ec19e94a6ec46eb556805155bdf05268d8e71189a9ce6` | 46 | tiempo de ejecución |
| `explorer-api_strace.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `explorer-api_strace.log` | `0160ab2e5f9f4e63a9bf8f6b3cb1231b221581dce51c805d2d5e26615f9c93c6` | 4354738 | strace |
| `explorer-api_strace.out` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | strace output filtrado |
| `explorer-api_--version.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `explorer-api_--version.out` | `b1af65ee0654bffd9106aa25a8b6a639bac6faa25ab70750dd4653cb9c9d8e73` | 20 | stdout |
| `explorer-api_--version.time` | `298bfa0688d7d21f993f0aed87c6740fffcda0cc78996577a8e5acce2c07d187` | 46 | tiempo de ejecución |
| `explorer-mcp_help.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `explorer-mcp_help.out` | `1575abd040110d2e8c68a02f311abb27462cc29e8fb4889340f28063e63f3579` | 306 | stdout |
| `explorer-mcp_help.time` | `b22738692b4a1809e4a9fab7e51a085ba600ed0d69c25049f65608f0447a7b18` | 46 | tiempo de ejecución |
| `explorer-mcp_--version.err` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | 0 | stderr |
| `explorer-mcp_--version.out` | `db6db21387ed8afb4df7e918dc4f67173565e1e7b5107ce209e96472ceb6dd0f` | 20 | stdout |
| `explorer-mcp_--version.time` | `6148395571d8e05884960b577425fe1db75f500384f6232133dbf9e1feedda89` | 46 | tiempo de ejecución |
| `mcp_sigint.err` | `cc7299eabbf5054a612693f997694e00d3e24e732ecc8be5f809d8c87271fc76` | 1293 | stderr |
| `mcp_sigint.out` | `876748debaa3f74dc9b063745ca306339d41e5dc72dee746ba567b6b1feca4bc` | 163 | stdout |
| `mcp_sigterm.err` | `e93730144d6d0de6d3c9160ac66fb7cc0c1e01933035483f7c7d2776c30b7c78` | 1293 | stderr |
| `mcp_sigterm.out` | `876748debaa3f74dc9b063745ca306339d41e5dc72dee746ba567b6b1feca4bc` | 163 | stdout |
| `mcp_stderr.bin` | `ad1ef19388423e25037bfc0185beb15e778729958b28bf3331b7a8c4bc423cb3` | 1790 | captura |
| `mcp_stdout.bin` | `ee7bfa4fa16927feb8317d96a1d0c74a55d90ac23e5f8624846f9d0c9eac781a` | 322 | captura |

## Evidencias no versionadas — `docs/prf/evidence/F0-W3-runs/`

**Total**: 3 archivos, ~244 KB.
**Origen**: baseline de pruebas ejecutado durante F0.W3 (2026-09-21).
**Descripción**: ver `docs/prf/evidence/F0-W3-baseline.md`.

| Archivo | SHA-256 | Tamaño (bytes) | Categoría |
|---|---|---|---|
| `cognicode-cli-cogh-no-update.txt` | `0efda7f41093a84ea61c1f2393eadc1fe3488b89d9918cc3482c331e8f286194` | 30980 | cargo test output |
| `cognicode-core-lib.txt` | `59defeb09fd4dfefe209ed9ecf6fdc8d95d4ea3462efd5506b903b79525383bf` | 192039 | cargo test output |
| `cognicode-ide-adapter.txt` | `16b9b4ae5e99e25bc6f2997635aff4ce58761edf0856a2904e6877b864dece62` | 20304 | cargo test output |

## Cómo regenerar la evidencia

### `F0-W2-runs/`

Los comandos exactos están documentados en `F0-W2-runtime.md` Apéndice A
(línea ~250+). Resumen:

```bash
# strace (3 binarios, ~2 minutos cada uno)
strace -f -e trace=network,read,write -o /tmp/cogh_strace.log cogh doctor
strace -f -e trace=network,read,write -o /tmp/cognicode-mcp_strace.log \
  cognicode-mcp
strace -f -e trace=network,read,write -o /tmp/explorer-api_strace.log \
  explorer-api

# outputs --version / --help / doctor
for bin in cognicode cognicode-mcp cogh explorer-api explorer-mcp; do
  { /usr/bin/time -v $bin --version 2>${bin}_version.err 1>${bin}_version.out; } \
    2>${bin}_version.time
done
```

### `F0-W3-runs/`

```bash
cargo test -p cognicode-core --lib 2>&1 | tee cognicode-core-lib.txt
cargo test -p cognicode-cli --bin cogh --skip test_cogh_update_respects_lockfile \
  2>&1 | tee cognicode-cli-cogh-no-update.txt
cargo test -p cognicode-cli --test cognicode_ide_adapter 2>&1 \
  | tee cognicode-ide-adapter.txt
```

## Disponibilidad

- **Esta máquina**: evidencia presente en disco; SHA-256 verificables con
  `cd docs/prf/evidence/<dir> && sha256sum -c <(cat ../MANIFEST.md | grep -E '^[|] `[a-z0-9_-]+`' | awk -F '\\|' '{print $2 $3}' | sed 's/[ `]*//g' )`.
- **Otro equipo**: NO están aquí. Para reconstruirlas basta ejecutar los
  comandos de arriba en un clone del repo en el mismo entorno. No son
  reproducibles bit-a-bit entre máquinas distintas (timings, PIDs, strace
  ordering), pero su **contenido semántico** (resultados de tests, número de
  threads, syscalls realizadas) sí lo es.

## Decisión de política

- Documentos `.md` del programa PRF → versionados (este MANIFEST es uno
  de ellos).
- Evidencia cruda → solo local, manifestada.
- Razón: el principio de "recover progress in another session" (AGENTS.md)
  se cumple vía (a) los documentos versionados que apuntan a la evidencia
  por path + SHA-256, y (b) los comandos de regeneración documentados en
  cada markdown de evidencia. El árbol Git no se infla con ~5,2 MB de
  strace y 244 KB de logs de cargo test que pueden regenerarse.

---

# Apéndice — Regeneración UAT-F3-001 (2026-09-23)

> Este apéndice extiende el manifiesto principal para cubrir las
> regeneraciones puntuales que se ejecutan contra HEAD de manera
> pre-push a `origin/main`. La política sigue siendo: "evidencia
> cruda = local, manifestada con SHA-256, regenerable con el
> comando del markdown".

## Evidencias regeneradas — `docs/prf/evidence/u58-f3-equivalence-regen/`

**Origen**: regeneración de UAT-F3-001 contra HEAD `$COMMIT_PLACEHOLDER` después
del hallazgo §102 (15 directorios `u50*`..`u69*` citados pero no
materializados en disco). Esta evidencia DEMUESTRA que la metodología
es regenerable; las otras 14 se regenerarán en una sesión dedicada
cuando se autorice el push.

**Reproducer literal** (también documentado en `OBSERVATIONS.md`):

```bash
mkdir -p /tmp/prf-uat-f3-regen/src/nested
cat > /tmp/prf-uat-f3-regen/src/lib.rs <<'RUST'
pub fn caller() -> i32 {
    crate::nested::callee()
}
RUST
cat > /tmp/prf-uat-f3-regen/src/nested/mod.rs <<'RUST'
pub fn callee() -> i32 { 42 }
RUST

# Capturar CLI
cd /tmp/prf-uat-f3-regen && \
  /var/home/rubentxu/cargo-targets/debug/release/cognicode graph full \
  > /tmp/cli_full.out 2> /tmp/cli_full.err

# Capturar MCP (JSON-RPC)
cd /tmp/prf-uat-f3-regen && \
  printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"prf-regen","version":"0.1"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"build_graph"}}\n' \
  | /var/home/rubentxu/cargo-targets/debug/release/cognicode-mcp \
  > /tmp/mcp_build_graph.raw 2> /tmp/mcp_build_graph.err
grep '^{' /tmp/mcp_build_graph.raw > /tmp/mcp_build_graph.json
```

**SHA-256 esperados al regenerar contra el mismo HEAD**:

| Archivo | SHA-256 | Tamaño (bytes) | Categoría |
|---|---|---|---|
| `cli_full.out` | `673ef8a89335c89fcb601b806bef91636ca8e0c5a06a492e0c1f2d7c7302a6a0` | (depende del binario, CLI stderr textual) | stdout |
| `cli_full.err` | `cc972ac99805aca083c1a1e4118518443daddb98214d73eabad28a2de96c84a8` | (depende de INFO logs) | stderr |
| `mcp_build_graph.json` | `d7d5817de1fc6df3046a9c3d1fec38360d3f3ba3b5c78bc90d78ac87365e27b9` | (depende del JSON) | stdout JSON-RPC |
| `mcp_build_graph.err` | `bd173b45b2581aadd4e7770d21a7a7f5b292fc689aa4a6db27932d2d24b48517` | (logging) | stderr |

> Nota: los SHA-256 son **dependientes del binario** (cada release
> cambia strings internos de versiones, paths de telemetría, etc.).
> La metodología es regenerable; los hashes son válidos solo para
> el SHA `$COMMIT_PLACEHOLDER` (HEAD al regenerar). Para
> certificaciones futuras, regenerar primero y luego computar
> hash contra la versión binaria actual.

## Evidencias post-ciclo H-06 — `docs/prf/evidence/u102-t4-pre-release/`

**Total**: 1 archivo markdown.
**Origen**: T4 pre-release ejecutado en HEAD `ed1ed09c` (post-H-06).
**Descripción**: ver `docs/prf/evidence/u102-t4-pre-release/OBSERVATIONS.md`.

| Archivo | SHA-256 | Tamaño (bytes) | Categoría |
|---|---|---|---|
| `OBSERVATIONS.md` | (regenerado contra HEAD actual; auditable leyendo el archivo) | ~2 KB | T4 evidence |

Esta evidencia documenta:
* Resultado T0 (build + clippy), T1 (lib tests), T2 (bin tests), T3
  (workspace --tests con `--test-threads=2`) observado contra HEAD
  `ed1ed09c`.
* Fix aplicado al flaky `t_l2_commit_records_versions_layout_in_journal`
  (cambio de path: de `crate::lifecycle_journal::journal_path(VERSION)`
  a `home.journal_version(VERSION)` para evitar releer `COGNICODE_HOME`
  env var global).
* Restricción de entorno documentada (helper `setup_temp_home`/`run_cogh`
  en `cognicode-cli/src/cmd/lifecycle.rs` comparte env vars globals
  entre bins; mitigación `--test-threads=2` para T4). No es regresión
  del ciclo H-06.
* Evidencia `u58-f3-equivalence-regen/` regenerada en §99 contra HEAD
  `ed1ed09c` con SHA-256 reproducible.

## Evidencias regeneradas ciclo 4 — `docs/prf/evidence/u60-cli01-exhaustive-regen/`

**Total**: 1 archivo markdown.
**Origen**: regeneración de §60 PRF-CLI-01 contra HEAD `2c846dab`.
**Descripción**: ver `docs/prf/evidence/u60-cli01-exhaustive-regen/OBSERVATIONS.md`.

| Archivo | SHA-256 | Categoría |
|---|---|---|
| `OBSERVATIONS.md` | (regenerado contra HEAD actual) | U60 regen evidence |

Esta evidencia:
* Recompila el binario `cognicode` (debug) desde HEAD actual.
* Ejecuta 8 comandos CLI contra un corpus mínimo (`add`, `mul`).
* Captura stdout/stderr por comando, computa SHA-256 por archivo.
* Reduce §102 en 1 (de 15 directorios perdidos a 14 pendientes).
* Demuestra metodología para regenerar las 13 evidencias restantes
  antes del push a `origin/main`.
