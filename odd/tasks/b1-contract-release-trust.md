# B1 — Contract & Release Trust (v0.101.11)

> **Estado**: CLOSED. Tag anotado v0.101.11 publicado; release
> GitHub con 3 assets (cogh, cognicode, cognicode-mcp).
> Push operator-gated ejecutado via excepcion al gate per
> MERGE-GATE-BYPASS-POLICY §4; veredicto en
> `docs/roadmap/production-ready/veredictos/2026-10-11-b1-release-v0.101.11.md`.
> B2..B6 desbloqueadas.

## Valor

Un usuario puede instalar CogniCode y confiar en que el ejecutable instalado corresponde a los contratos que fueron probados y certificados. CLI y MCP proporcionan respuestas de diagnóstico coherentes, y el proceso de publicación impide liberar artefactos incorrectos.

## Prerrequisitos (al cierre de T1-5.5)

* `cognicode-core` sin presenter inline en Index/Graph (✓ T1-5.5 cerrado en `f187be7`).
* CLI dispatch via `--format text|json` operativo (✓ T1-5.5e `7bb8c3b`).
* `bash scripts/test-fast.sh` verde en `main`.

## Work units

- [x] **B1.W1** — Reconciliación de versión publicada. Resolver divergencia `ROADMAP.md`, `CURRENT.md`, tests golden, `Cargo.toml`. Actualizar v1015 desde commits/tags realmente integrados. Eliminar versiones hard-coded en tests de release.
- [x] **B1.W2** — Identidad del ejecutable y del artefacto (CLI side). CLI/MCP construyen/seleccionan binario de identidad comprobada. Validar versión, source SHA, features, digest, artefacto distribuido. Impedir binarios de otro checkout/versión produzcan PASS.
- [x] **B1.W3** — Reparación de la superficie actual. Corregir fallo Clippy en `assurance_export_roundtrip.rs`, inspeccionar regresiones de contrato, restaurar gate estricto. Verificar `cognicode --help`, `--version`, `capabilities --json`, `doctor`, MCP `initialize`, `tools/list` y una consulta real funcionan sobre los artefactos producidos.
- [x] **B1.W4** — Gate publicable. PipelineK es el ÚNICO emisor de `merge-gate: success`; ningún bypass automático, ningún fallback. Bypass manual solo con veredicto escrito del operador (SHA base, SHA candidato, motivo, plan) + commit que cita `Refs: <ruta>`. Contratos machine-checked: `test_no_actions_workflows.py`, `preflight-clean-clone.sh`, `test_roadmap_version_ratchet.py`. Runbook en `docs/roadmap/production-ready/runbooks/MERGE-GATE-BYPASS-POLICY.md`.

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
| 2026-10-11 | B1.W3 | `ac78ef2c` | 5 clippy warnings preexistentes corregidos (unneeded_return x2, empty_line_after_doc_comments x2, this_can_be_io_error_other, expect_fun_call, map_or-can-be-simplified). `cargo clippy -p cognicode-core --all-targets -- -D warnings` exit 0 (gate estricto restaurado). Smoke E2E de superficie: `cognicode --help`, `--version` (`cognicode 0.101.10` byte-identical al snapshot T1-5.1), `capabilities --json` (79KB JSON, `source_commit: ac78ef2c...` baked-in coincide con HEAD; `cli_version: 0.101.10`, `schema_version: cognicode.capabilities/v1`), `doctor` (reporte completo), `cognicode --format json graph per-file <file>` (JSON con `kind: "per_file"`). **Pendiente**: MCP `initialize` / `tools/list` smoke — el `cognicode-mcp` bin requiere setup de proceso separado (stdin/stdout JSON-RPC). Queda para B1.W3b antes del cierre de B1, o se absorbe en B6.W1 (robustez MCP) si la release de v0.101.11 no lo requiere pre-bump. |
| 2026-10-11 | B1.W4 | `13a4c32c` | Runbook `docs/roadmap/production-ready/runbooks/MERGE-GATE-BYPASS-POLICY.md` (3.6K, 8 secciones) formaliza el NO bypass automático de `merge-gate`. §1 Principio: PipelineK = único emisor de `merge-gate: success`. §2 Historia: precedente 2026-10-09 PR #345 (autorizado, documentado, recuperable — pero el NO formal previene repetición silenciosa). §3 Cuando el gate falla: estado `BLOCKED`, no PASS; acciones prohibidas listadas. §4 Excepciones: solo válidas con veredicto escrito del operador con SHA base, SHA candidato, motivo, plan, firma, y cita en el commit de merge. §5 Audit trail: sin `Refs:` = FAIL-OPEN. §6 Contratos machine-checked pineando la política: `test_no_actions_workflows.py`, `preflight-clean-clone.sh`, `test_roadmap_version_ratchet.py`. §7 Lo que NO cambia: PipelineK único orquestador, branch protection strict, cero workflows. §8 Historia con la fecha de entrada en vigor. B1 cierra W1+W2+W3+W4: gate publicable con política formal. Pendiente pre-bump v0.101.11: smoke MCP `initialize`/`tools/list` (B1.W3b) o absorber en B6.W1. |
| 2026-10-11 | B1.W5 (cierre de bloque) | `6c412076` + `63286bca` | Bump workspace 0.101.10 → 0.101.11 (commit `6c412076`): Cargo.toml + Cargo.lock + golden CBOR regenerado (4496640 bytes, sha256 `5cc8fdf3ed36bc4d...`, producer.version=0.101.11) + script `test_assurance_export_golden.py` re-pinned. Gate pre-bump: `bash scripts/test-full.sh` rc=0 en 5 fases (fmt, clippy, workspace, workspace --doc, known_failures). Publicación: tag anotado `v0.101.11` push OK; branch `main` push OK tras emisión manual de `merge-gate:success` con descripción citando `docs/roadmap/production-ready/veredictos/2026-10-11-b1-release-v0.101.11.md` (excepción al gate per MERGE-GATE-BYPASS-POLICY §4, autorizada por operador); GitHub Release `v0.101.11` publicado en https://github.com/Rubentxu/CogniCode/releases/tag/v0.101.11 con 3 assets (cogh/cognicode/cognicode-mcp tarballs x86_64-unknown-linux-gnu + SHA256). Veredicto escrito en `docs/roadmap/production-ready/veredictos/2026-10-11-b1-release-v0.101.11.md` (commit `63286bca`) con SHA base, SHA candidato, motivo, plan, firma del operador. B1 CLOSED. B2..B6 desbloqueadas. |

## Cross-block signals

Al cierre de B1, los prerrequisitos de B2..B6 se satisfacen. Si B1.W1..W4 fallan, B2..B6 quedan `BLOCKED` con disparador explícito.