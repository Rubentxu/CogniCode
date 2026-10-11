# B1 — Contract & Release Trust (v0.101.11)

> **Estado**: PENDING. Prerrequisito de B2..B6 (sin gate publicable
> confiable, ningún bloque puede cerrar con PR + merge-gate + tag).
> Detalle espeficado: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b1--contract--release-trust).

## Valor

Un usuario puede instalar CogniCode y confiar en que el ejecutable instalado corresponde a los contratos que fueron probados y certificados. CLI y MCP proporcionan respuestas de diagnóstico coherentes, y el proceso de publicación impide liberar artefactos incorrectos.

## Prerrequisitos (al cierre de T1-5.5)

* `cognicode-core` sin presenter inline en Index/Graph (✓ T1-5.5 cerrado en `f187be7`).
* CLI dispatch via `--format text|json` operativo (✓ T1-5.5e `7bb8c3b`).
* `bash scripts/test-fast.sh` verde en `main`.

## Work units

- [x] **B1.W1** — Reconciliación de versión publicada. Resolver divergencia `ROADMAP.md`, `CURRENT.md`, tests golden, `Cargo.toml`. Actualizar v1015 desde commits/tags realmente integrados. Eliminar versiones hard-coded en tests de release.
- [x] **B1.W2** — Identidad del ejecutable y del artefacto (CLI side). CLI/MCP construyen/seleccionan binario de identidad comprobada. Validar versión, source SHA, features, digest, artefacto distribuido. Impedir binarios de otro checkout/versión produzcan PASS.
- [ ] **B1.W2** — Identidad del ejecutable y del artefacto. CLI/MCP construyen/seleccionan binario de identidad comprobada. Validar versión, source SHA, features, digest, artefacto distribuido. Impedir binarios de otro checkout/versión produzcan PASS.
- [ ] **B1.W3** — Reparación de la superficie actual. Corregir fallo Clippy en `assurance_export_roundtrip.rs:408`. Inspeccionar regresiones de contrato. Restaurar gate estricto. Verificar `cognicode --help/--version/capabilities --json/doctor`, MCP `initialize/tools/list`, una consulta real.
- [ ] **B1.W4** — Gate publicable. Reforzar `merge-gate.pipeline.kts`, `integration.pipeline.kts`, `release-candidate.pipeline.kts`, `release.pipeline.kts`. Fallo PipelineK → `BLOCKED` con evidencia diferenciada. No bypass temporal de branch protection sin procedimiento de excepción.

## Acceptance criteria (resumen)

- `bash scripts/test-full.sh` exit 0.
- `python3 scripts/ci/test_assurance_export_golden.py` exit 0.
- `python3 scripts/ci/test_roadmap_version_ratchet.py` exit 0.
- `python3 scripts/ci/preflight-clean-clone.sh` exit 0 sobre clone nuevo.
- `git tag -l` muestra `v0.101.11` anotado.
- GitHub Release `v0.101.11` con assets publicables.
- ROADMAP §10 actualizado con SHA integrado + recibo.

## Out of scope

- Cambios funcionales al CLI/MCP (eso es B2..B6).
- Refactors de capabilities (eso es B2.W2).
- Repair profundo de `assurance_export_roundtrip.rs` (B1.W3 arregla el lint que rompe el gate; la reparación semántica del export es B2.W4).

## Riesgos

- **R-B1-W2**: la identidad del binario puede revelar divergencias ocultas entre `Cargo.toml` y los binarios instalados; el contrato debe leer del producto, no del toolchain.
- **R-B1-W4**: bypass de branch protection autorizado por PR #345 (`3203bc75`) sigue en memoria institucional; el contrato debe dejarlo explícito como NO REPETIBLE sin procedimiento formal.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| 2026-10-11 | B1.W1 | `f6b124c` | `scripts/ci/test_assurance_export_golden.py`: `PRODUCER_VERSION` derivado de `Cargo.toml [workspace.package].version` (vía helper `workspace_version()`) — el hardcoded `"0.101.9"` se eliminó. `GOLDEN_SHA256` actualizado de `45fa7821...` (v0.101.9) a `3ac84ea7...` (v0.101.10); el producer emite `producer.version` en el envelope CBOR, así que cada bump cambia el SHA. Pin anterior registrado en comentario del script para que `pipelinek-assurance` coordine su pin. `tests/fixtures/assurance_export_golden.cbor` regenerado (4494290 bytes, producer v0.101.10, HEAD `6af11bf83f07`). `python3 scripts/ci/test_roadmap_version_ratchet.py` exit 0 (workspace `0.101.10`, head `f6b124c`). CURRENT.md regenerado localmente (gitignored por `.gitignore:200`). |
| 2026-10-11 | B1.W2 | `336e743` | `crates/cognicode-cli/build.rs` (NEW) ejecuta `git rev-parse --short HEAD` al compilar y emite `cargo:rustc-env=GIT_SHA=<sha>`. `crates/cognicode-cli/src/identity.rs` (NEW) expone `version()`, `source_sha()`, `features()`, `print(w)`. `pub mod identity` en lib. `CliCommand::Identity` (commands.rs) + dispatcher en main.rs (detecta ANTES del executor para no clonar CliCommand, que no deriva Clone). 4 unit tests + 5 binary smoke tests verdes. `cognicode --version` byte stream preservado (T1-5.1 snapshot pin). Smoke E2E: `cognicode identity` emite `cognicode-cli 0.101.10 sha=99821b72 features=none` + `source: git@github.com:Rubentxu/CogniCode.git`. **Pendiente**: MCP server (`cognicode-mcp` bin) — sigue B1.W2b o B6.W1. |
| _pendiente_ | B1.W3 | _pendiente_ | _pendiente_ |
| _pendiente_ | B1.W4 | _pendiente_ | _pendiente_ |

## Cross-block signals

Al cierre de B1, los prerrequisitos de B2..B6 se satisfacen. Si B1.W1..W4 fallan, B2..B6 quedan `BLOCKED` con disparador explícito.