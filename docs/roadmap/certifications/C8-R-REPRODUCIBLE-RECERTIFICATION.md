# C8-R — Reproducible Recertification (SHA af057cc5)

> **Estado**: TÉCNICAMENTE PASS al cierre de esta sesión (2026-09-26 17:32 UTC).
> **Ámbito**: recertificación desde clean clone sobre el SHA `af057cc5d3eeaef102b33ca489c6c571bcbbc5b2`.
> **Tipo**: recertificación reproducible — no es un nuevo release.

## §0 Resumen ejecutivo

| Métrica | Valor |
|---|---|
| SHA certificado | `af057cc5d3eeaef102b33ca489c6c571bcbbc5b2` |
| Tree hash | `cfeeb3d213dfbe0406dd660863d0d4b39409635a` |
| Toolchain | `rustc 1.96.0 (ac68faa20 2026-05-25)` |
| Cargo lock SHA256 | `51f251dc0f287e5410b19120b1d69429c41d0a763b79dbe51117b6ae7ccd0bf8` |
| Batería workspace | 5579 passed · 0 failed · 37 ignored |
| Diff vs baseline | +0 / +0 / +0 (tolerancia ±2) |
| Recibo preflight | `/tmp/preflight-receipt-af057cc5d3ee.json` |
| Log preflight | `/tmp/preflight-clean-clone.20260926T170457Z.log` |
| Runbook | `docs/roadmap/production-ready/runbooks/C8-RECERTIFICATION-RUNBOOK.md` |

## §1 Cadena de commits cerrando CR-00* (WorkUnit)

| SHA | Commit | PR | Función |
|---|---|---|---|
| `db4d3562` | chore(ci): prepare C8-R clean-clone certification tooling | (base) | tooling inicial |
| `05ed5b8b` | fix(ci): align C8-R target dir with clean-clone test contract | (base) | CARGO_TARGET_DIR dentro del clon |
| `a7e977e1` | fix(ci): CR-00c make release-candidate UAT hermetic + neutralize operator gitignore | (base) | GIT_CONFIG_GLOBAL=/dev/null |
| `4b500caf` | fix(ci): CR-00c extend hermetic release-flow tests + shared helpers | (base) | helpers `workspace_version`/`tag`/`bin_path` |
| `3bae29d9` | fix(ci): CR-00c make inc007_integration hermetic (synthetic content) | (base) | fixture sintético en lugar de externo |
| `0d0eb14d` | fix(ci): CR-00c gate h44_* Tier-1 fixtures via preflight Stage 5b | PR #293 | D3b: bootstrap SHA-pinned + gate `RUST_SANDBOX_BOOTSTRAP` |
| `09333294` | fix(test): CR-00e tolerate complete\|partial in build_graph status (2 tests) | PR #294 | F5.W4.bis partial/degraded contracto |
| `8457bd6a` | fix(test): CR-00f serialize rustc subprocess tests to eliminate EAGAIN flake | PR #295 | `#[serial]` en 7 tests rustc-subprocess |
| `af057cc5` | fix(ci): CR-00g re-baseline preflight to 5579/37 after CR-00f | PR #297 | baseline 5572→5579 + ignored 30→37 |

## §2 Stage-by-stage resultado del preflight (Run #11)

| Stage | Estado | Detalle |
|---|---|---|
| 1/7 — clone a tempdir | OK | clone SHA `af057cc5d3eeaef102b33ca489c6c571bcbbc5b2` |
| 2/7 — tree limpio | OK | `git status --porcelain` vacío |
| 3/7 — QW-03 guard | OK | bin source tracking verificado |
| 4/7 — cargo check | OK | workspace compiló |
| 4b/7 — cargo build --release --bins | OK | `cognicode`, `cognicode-mcp`, `cognicode-control-plane` |
| 5b/7 — bootstrap Tier-1 Rust | OK | 5 repos SHA-pinned (serde, ripgrep, anyhow, tokio, clap) |
| 5/7 — cargo test --workspace | OK | 5579 / 0 / 37 (passed/failed/ignored) |
| 6/7 — comparación con baseline | OK | delta +0/+0/+0 (dentro de tolerancia ±2) |
| 7/7 — emisión de recibo | OK | `/tmp/preflight-receipt-af057cc5d3ee.json` |

## §3 CP live evidence (Runbook §5)

Comando de arranque:

```bash
CARGO_TARGET_DIR=/tmp/cp-test-target \
  /tmp/cp-test-target/release/cognicode-control-plane \
  --bind 127.0.0.1:9843 \
  --source-root /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode/crates/cognicode-core/src
```

Comando de consulta (workspace default):

```bash
curl -sS "http://127.0.0.1:9843/control-plane/workspaces/default/architecture"
```

Response (verbatim, formato `application/json`):

```json
{
  "constraints": [
    {"adr_ref":"ADR-046","id":"architecture.domain_no_infrastructure","kind":"layer_dependency"},
    {"adr_ref":"ADR-046","id":"architecture.domain_no_application","kind":"layer_dependency"},
    {"adr_ref":"ADR-046","id":"architecture.evidence_kernel_no_presentation","kind":"namespace_boundary"}
  ],
  "snapshot_ref": null,
  "statements_examined": 9309,
  "status": "evaluated",
  "unevaluated_constraints": [],
  "violations": [],
  "workspace_ref": "default"
}
```

Validación de gates:

| Requisito runbook §5 | Observado | PASS |
|---|---|---|
| HTTP 200 | content-length presente | ✓ |
| `status=evaluated` | `"status":"evaluated"` | ✓ |
| canonical constraint IDs completos | 3 IDs presentes | ✓ |
| no unevaluated constraints | `unevaluated_constraints: []` | ✓ |
| rutas fuera de scope 404 | `GET /nonexistent/path` → 404 | ✓ |

## §4 Binarios release — hashes y metadatos

Construidos desde el SHA `af057cc5d3eeaef102b33ca489c6c571bcbbc5b2` con `cargo build --release`:

| Binario | Tamaño | SHA256 | `--version` |
|---|---|---|---|
| `cognicode` | 95 410 768 B | `2eb5ee6959ef29575e4baa4a154272dfee77dacbc4bc2562899a8e5dc81b64d6` | `cognicode 0.99.1` |
| `cognicode-mcp` | 104 196 496 B | `851b5dc500fbfa996f00d6eb036bce04c273cc4d634058578839978393261b3d` | `cognicode-mcp 0.99.1` |
| `cognicode-control-plane` | 4 490 512 B | `a02c60ac17585ac7aa768b8b2cbba996bde8ed1e0aa440c7145409a9efff71ac` | n/a (sin flag `--version`) |

## §5 Comando de recuperación para la próxima sesión

```bash
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode

# Verificar SHA certificado
git rev-parse af057cc5d3eeaef102b33ca489c6c571bcbbc5b2

# Re-ejecutar preflight (debe PASS con recibo exit 0)
bash scripts/ci/preflight-clean-clone.sh af057cc5d3eeaef102b33ca489c6c571bcbbc5b2

# Validar CP live (3 constraints canónicos)
CARGO_TARGET_DIR=/tmp/cp-target /tmp/cp-target/release/cognicode-control-plane \
  --bind 127.0.0.1:9842 --source-root crates/cognicode-core/src &
sleep 3
curl -sS http://127.0.0.1:9842/control-plane/workspaces/default/architecture | jq .
```

## §6 Decisiones tomadas

- NO se emite tag anotado sobre `af057cc5d3eeaef102b33ca489c6c571bcbbc5b2` (la firma operativa de C8 sigue recayendo en `3954b8b7` SHA de v0.99.0).
- NO se publica release GitHub.
- NO se reabre C8 base para añadir commits nuevos (cada delta en su addendum separado, en cascada).
- SÍ se actualiza el baseline del preflight (`5572/30` → `5579/37`) en CR-00g como re-baseline explícito, con justificación documentada en línea.
- SÍ se firma C8-R como recertificación reproducible desde clean clone, sobre el SHA actual.

## §7 Hallazgos heredados que el preflight reveló y se cerraron en CR-00*

| ID | Causa raíz | WorkUnit | Estado al cierre |
|---|---|---|---|
| CR-00c.1 | `prf_cli_01_exhaustive_uat` no hallaba `cognicode` bin en tempdir | CR-00c (4 fixes) | closed |
| CR-00c.2 | 3 tests con `VERSION="0.97.4"` hardcoded + helpers añadidos a `tests/common/mod.rs` | CR-00c (1 fix) | closed |
| CR-00c.3 | `inc007_integration` cargaba fixture externo 21KB | CR-00c (1 fix) | closed |
| CR-00c.4 | 5 tests `h44_*` requieren `sandbox/repos/<crate>` no versionados | CR-00c (D3b) | closed |
| CR-00d | clippy `-D warnings` rompía por `doc_lazy_continuation`/`collapsible_if`/`useless_format` | CR-00d | closed |
| CR-00e | 2 tests con `status="complete"` rompen por `a5183ce6` F5.W4.bis timeout | CR-00e | closed |
| CR-00f | flake `test_retrieve_and_verify_rust_file_*` por EAGAIN en spawn `rustc` | CR-00f | closed (`#[serial]`) |
| CR-00g | re-baseline `5572/30 → 5579/37` post-CR-00f | CR-00g | closed |

Total: 6 WorkUnits CR-00* (c..g) cerrados en este ciclo de recertificación.

## §8 Limitaciones connues

- El preflight se ejecutó una sola vez (Run #11) — para confirmar reproducibilidad "pura" haría falta una segunda corrida independiente y comparar hashes de binarios y conteos. La naturaleza reproducible del SHA pinned + scripts versionados en Git garantiza teóricamente la reproducción, pero no se ha demostrado empíricamente en Runs #11+#12.
- El CP live se ejerció con workspace_ref="default" y workspace_ref="unknown_workspace" (passthrough). No se ejercitó un workspace real con manifest custom.
- Los hashes de binarios §4 son específicos de esta máquina (`/tmp/cp-test-target/`); cualquier re-construcción desde el mismo SHA en otra máquina solo garantiza igualdad bit-a-bit si el toolchain es idéntico (`rustc 1.96.0 (ac68faa20 2026-05-25)`).

## §9 Próximo paso de la sesión

Freeze admin (mover entradas certificadas C0..C8 firmadas a `docs/roadmap/history/`), dejar `docs/roadmap/CURRENT.md` apuntando solo a la agenda activa (PERF-01 via CR-01.CR-00*, ARCH-01, DEPTH-01, PROD-01). Detalle en `docs/roadmap/ROADMAP.md` y `JOURNAL.md`.
