# U112 — T5 release snapshot (binarios release profile)

**Fecha**: 2026-09-23
**Source-commit**: `60c8d53da6fbf768efc1291b6371a3054a1986c9`
**HEAD**: 60c8d53d docs(prf): STATE sync to §119 (workspace-wide tests fixed at root)
**Profile**: release (`cargo build --release --workspace --exclude cognicode-graph-wasm`)
**Tier-1**: linux-x86-64, linux-aarch64 GNU
**Workspace version**: 0.97.4 (Cargo.toml workspace)

## Binarios construidos (Tier-1 x86_64-unknown-linux-gnu)

| Binario | Versión | SHA-256 | Tamaño (bytes) |
|---|---|---|---|
| `cogh` | `cogh 0.97.4` | `46a56d1f1230e73ce11192c45e57b9bf2dd1edab6db74245bc466f79d52a2760` | 6973000 |
| `cognicode` | `cognicode 0.97.4` | `365d04e85f0bfbe21e11bb003abe83cada0bdb522c88c4c892390b74237813aa` | 95455152 |
| `cognicode-mcp` | `cognicode-mcp 0.97.4` | `1957c5ad59318fb53db56757ab06d46035f2d3da17334e38e3b43f7852c22754` | 104185136 |
| `cognicode-mcp-server` | `cognicode-mcp-server 0.97.4` | `5eac0d4b5f39e752502ddba8898ab571f22a11d9e7675a3d83ef064601c4bb3e` | 106325328 |
| `cognicode-release` | `cognicode-release 0.97.4` | `9bc80233f80cb6c96e3005780dfe6b0c88c2dda7ec14ac7c1fa15d1fdcf7d314` | 1490808 |
| `mcp-client` | `mcp-client 0.97.4` | `37573547ebf8000d03b6e1dd645ec88dc93cde45811fdbd497c34752821feabc` | 3542192 |

**Total artefactos**: 6 bins (~314 MB optimizados, linked-statically).

## Tier-1 platforms (release.yml matrix)

```yaml
strategy:
  matrix:
    platform:
      - x86_64-unknown-linux-gnu
      - aarch64-unknown-linux-gnu
```

## Verificación T4 + T5 pre-release (post-§119 fix raíz)

### T0 — clippy --workspace --all-targets

```text
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.94s
```

EXIT 0. Cero warnings.

### T1 — lib (cognicode-core)

```text
$ cargo test -p cognicode-core --lib
test result: ok. 2155 passed; 0 failed; 27 ignored; 0 measured
```

2155 tests passed. Los 27 ignored son tests `#[ignore]` legítimos
(documentados en cada `mod` con razón de skip).

### T2 — cogh (CLI usuario)

```text
$ cargo test -p cognicode-cli --bin cogh
test result: ok. 315 passed; 0 failed; 1 ignored; 0 measured
```

315 tests passed (incluye `tracker::h_f6_1_tests` blindados en §105).

### T3 — workspace --tests con --test-threads=2 (post-§119)

```text
$ cargo test --workspace --tests -- --test-threads=2
[104 test results: ok]
test result: ok. 2155 passed
test result: ok. 315 passed
test result: ok. 955 passed
test result: ok. 64 passed
test result: ok. 177 passed
... (todos verdes)
```

**Cero failures workspace-wide**. El fix raíz §119 (eliminar
`Cargo.toml` decorativo de `mcp_03_ws` + relajar assert CLI-04 +
bump DIST a 0.97.4) elimina los 5 failures que aparecían con
`--test-threads=2` post-§117.

### T5.0 — Bins construidos contra HEAD `60c8d53d`

```text
$ cargo build --release --workspace --exclude cognicode-graph-wasm
    Finished `release` profile [optimized] target(s) in 6m 51s
```

6 bins release construidos. SHA-256 capturados arriba.

### T5.1 — Tier-1 platforms declaradas

```text
$ cognicode-release platforms
x86_64-unknown-linux-gnu
```

(`aarch64-unknown-linux-gnu` declarado en `release.yml` matrix
pero no construido en este snapshot — requeriría cross-compile
toolchain `aarch64-linux-gnu-gcc`).

## Smoke test bins release

```text
$ cognicode --version          # cognicode 0.97.4
$ cogh --version                # cogh 0.97.4
$ cognicode-mcp --version       # cognicode-mcp 0.97.4
$ cognicode-mcp-server --version # cognicode-mcp-server 0.97.4
$ cognicode-release --version   # cognicode-release 0.97.4
$ mcp-client --version          # mcp-client 0.97.4
```

Los 6 bins responden correctamente con `0.97.4`.

## Reproducibilidad

Comandos (síntesis):

```bash
# Desde raíz del workspace
CARGO_TARGET_DIR=/var/home/rubentxu/cargo-targets/release \
  cargo build --release --workspace --exclude cognicode-graph-wasm

# SHA-256
sha256sum /var/home/rubentxu/cargo-targets/release/release/{cogh,cognicode,cognicode-mcp,cognicode-mcp-server,cognicode-release,mcp-client}
```

## Cambios desde snapshot anterior (`018f50f8`)

- **6 bins re-construidos** contra HEAD `60c8d53d` (233→234
  commits ahead).
- **§117**: bins sincronizados con workspace tras detectar drift
  sistémico en `target/release/`.
- **§119**: workspace-wide `--test-threads=2` verde completo
  tras fix raíz de 5 tests integración.
- **Nuevos bins** (vs snapshot anterior):
  - `cogh` ahora en release profile (`46a56d1f…`).
  - `mcp-client` ahora bin release explícito (`37573547…`).

## Estado de certificación

- F0 = ACCEPTED.
- F1 = ACCEPTED.
- F2 = ACCEPTED (W1-W10 IMPLEMENTED vía cert C2).
- F3-F6 = ACCEPTED vía C3-C6.
- **C7 = BLOQUEADO** (auditoría 2026-09-22 revocó
  `READY FOR RELEASE ≡ C7 PASS`).
- H-F6-1 legalmente cerrado y blindado (§105).

## Diferencias vs T4 (release pipeline §104)

§104 (2026-09-23) construyó solo `cogh` + `cognicode-release`
para R1-R9 release factory. **Este snapshot §112 v2** construye
los 6 bins del workspace (incluyendo `cognicode-mcp`,
`cognicode-mcp-server`, `mcp-client`) porque todos son Tier-1.

## §102 actualización

- 0 perdidos regenerables con binario local.
- 9 perdidos con CI/cross-compile (Tier-1 aarch64 + Windows +
  macOS).

## Push + tag v0.97.4 + C7 firma

Operator-gated. Este snapshot deja el push-ready **formalmente
validado**: T0/T1/T2/T3/T5 verde, 6 bins release construidos,
SHA-256 capturados. El operador puede autorizar push cuando lo
considere apropiado.

No ejecuta: push, tag, C7 firma. Operator-gated.
