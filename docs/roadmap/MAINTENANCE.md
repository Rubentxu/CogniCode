# Maintenance backlog v0.98.x

> **Estado**: backlog activo de mantenimiento sobre la rama estable v0.98.x.
> Estas unidades son **mantenimiento**, no features. Se cierran con bump SEMVER patch (`v0.98.2`, etc.) sin abrir nueva release mayor. Las features Post-PRF van a su propia serie (`F0.*`) y se liberan con SEMVER minor (`v0.99.0` o lo que la política E0 establezca al cierre).
> Vive en `docs/roadmap/` junto al roadmap ejecutivo.

## M0.* — Mantenimiento v0.98.x

| ID | Descripción | Estado | Evidencia | Trigger |
|---|---|---|---|---|
| **M0.1** | `cogh rollback --to <same>` falla por journal nuevo no listado en `active_install_is_coherent()`. Test RED: `current=0.98.1, install coherent, old journal exists` → debe devolver 0 sin cambios. | **CLOSED 2026-09-25** | **5 tests pineando el contrato no-op de `cmd_update` pasan verdes**: `t_e86_4_rollback_to_current_is_noop` (cmd_rollback) + `f3_t1_same_version_update_is_zero_mutation` + `f3_t2_rollback_after_noop_update_applies_original_transition` + `f3_t3_real_version_transition_still_transitions` + `f3_t4_broken_same_version_install_is_repaired_not_hidden` + `f3_t5_noop_reports_decision`. Todos verifican `before == after` (cero mutación del lifecycle) cuando tracker pin y resolved version coinciden y `active_install_is_coherent` retorna true. El test crítico es **f3_t4** (caso donde coherencia falla → cae a repair, no a hide). El comportamiento que el operador sospechaba bug NO se reproduce contra HEAD `ede4772d`. | Detectado por revisión operador 2026-09-25 |
| **M0.2** | `cargo fmt --all --check` falla con 104 archivos drift detectados por G0.1 al pasar por PR-CI. Trabajo mecánico: `cargo fmt --all` en bloque + commit atómico `chore(fmt): apply rustfmt over drifted files`. | CLOSED 2026-09-25 | PR #291 squash-merged como `26746a64`. 25 archivos formateados + 5 lints arreglados (M0.2.1+M0.2.2) + workflow chmod +x fix (M0.2.3) + 3 fixtures (M0.2.4) + assertion state13 (M0.2.5). CI run #36120627650 con 4/4 jobs PASS. | Detectado por G0.1 (run #36113397644) |
| **M0.3** | Auditoría clippy residual (`H-clippy-cli-residual D34-2`) + `moldql` panic test preexistente. | CLOSED 2026-09-25 | clippy strict: `cargo clippy --workspace --all-targets -- -D warnings` exit 0 (cubierto por M0.2.1+M0.2.2). moldql: `cargo test -p cognicode-explorer --lib` → 834 passed; 0 failed; 0 ignored; 0 measured. Tests de moldql_pattern_mcp/Rest/e28_3_runtime_wiring todos verdes (231+4+3+7+13 = 258 tests moldql pasan). Los `panic!` en `intent.rs:135,166,182,196` y `consolidated_handlers.rs:1177,1279,1359` son tests de contrato (pinean invariantes), NO regresiones. Sin panic test preexistente fallando. | Carry-over PRF (F7 §244, STATE §13, RELEASE-CANDIDATE §80–81) |
| **M0.4** | State pollution en `#[serial]` CLI tests (3 tests fallando con env vars `COGNICODE_ASSET_BASE_URL`/`COGNICODE_BUNDLE_MANIFEST` leak entre tests). Detectada tras el SemVer bump 0.98.1→0.99.0 (commit `d4a2e33e`). | CLOSED (commit `f76a4b03`; nuevo `AssetPoint` RAII guard reemplaza 6 llamadas `point_at(&release)` huérfanas de `unpoint()`; `prf_f6_w3_bis_rollback_reports_*` marcado `#[serial_test::serial]`; 4 nuevos tests pinean el contrato del guard; workspace tests verde). | 2026-09-25 |
| **M0.5** | 8 tests `#[ignore]` con flake documentado en `cognicode-core/src/application/services/file_operations.rs` y `cognicode-core/src/infrastructure/verification/rust_verifier.rs:205`. Causa raíz: `retrieve_and_verify` ejecutaba `Command::new("rustc").arg("--version").output()` upfront, lo cual dispara `fork()+exec` por cada llamada. En paralelo (default `cargo test`), 3 de los 8 tests fallan con `Err(InvalidParameter("rustc not found"))` por contención de procesos (`fork` retorna `EAGAIN`). Un segundo modo de fallo: múltiples `rustc --crate-type lib` paralelos escribiendo `libvalid.rlib` en el CWD compartido causaban `failed to open object file: No such file or directory`. Test RED confirmado: `cargo test -p cognicode-core --lib file_operations::tests:: -- --include-ignored` → 3 failures reproducibles. | **CLOSED 2026-09-26** (commits `5fad9b40` fix + `3d7ba021` docs+bump; fix: `which::which("rustc")` reemplaza fork upfront, `current_dir(temp_dir)` aísla `.rlib` outputs en temp_dir; 8 tests `#[ignore]` re-habilitados sin marker; pre-cambio: 5557/0/45; post-cambio: 5565/0/37 — +8 tests al count, -8 ignored, 0 regresiones; clippy exit 0). | Detectado en auditoría dirigida de tests `#[ignore]` flake — entrada 23 |
| **M0.6** | **PHP y Swift rotos en producción** por incompatibilidad tree-sitter parser vs runtime. `tree-sitter-php = "0.24.2"` y `tree-sitter-swift = "0.7.3"` están compilados con `tree-sitter = "0.25"` (parser version 15) pero el workspace usa `tree-sitter = "0.24"` (runtime `LANGUAGE_VERSION = 14`). Cualquier intento de parsear `.php` o `.swift` con `TreeSitterParser::new(Language::Php)` o `Language::Swift` falla con `LanguageError { version: 15 }` en producción. 4 tests `#[ignore]` en `cognicode-core/src/infrastructure/parser/type_ref_walkers.rs:1150,1169,1188,1207` pinean el bug. Test RED confirmado: `cargo test -p cognicode-core --lib infrastructure::parser::type_ref_walkers -- --include-ignored` → 4 failures (`LanguageError { version: 15 }`). Bug BLOQUEANTE para usuarios que trabajen con PHP o Swift. | **BLOCKED 2026-09-26 — pendiente decisión operador** | **Opciones de fix** (no aplicables unilateralmente por alcance mayor): **(A)** Bumpear `tree-sitter = "0.24"` → `"0.25"` o `"0.27"` en `Cargo.toml` workspace. Impacto: afecta 18 parsers. Riesgo: regresiones de API entre versiones tree-sitter. Beneficio: repara 2 lenguajes completos. **(B)** Downgrade `tree-sitter-php`/`-swift` a versiones compiladas con tree-sitter 0.24 — no existen versiones oficiales disponibles (las últimas son 0.24.2 y 0.7.3, ambas incompatibles). Única vía: fork comunitario (no oficial). **(C)** Marcar PHP y Swift como `Language::Unsupported` con mensaje claro al usuario. Fix quirúrgico sin reparación real. | Detectado en auditoría dirigida de tests `#[ignore]` post-M0.5 — entrada 24 |
| ~~**M0.3.b**~~ | ~~CLI equivalente a `find_usages` MCP tool.~~ | ~~REASIGNADO A F0.1~~ (ver JOURNAL §7, L0 del Post-PRF) | ~~No era mantenimiento: era feature Post-PRF. La asignación previa a E3 era inconsistente con la definición de E3 (RPC mínima condicionada a un segundo cliente real).~~ | ~~Carry-over PRF, ahora evolutivo~~ |

## F0.1 (evolutivo, fuera de MAINTENANCE)

**F0.1 — `find_usages` CLI wrapper sobre MCP tool**: feature Post-PRF
que expone la MCP tool `find_usages` (ya existente) como CLI
command. Sirve como primera **prueba de consumidor real del contrato
E0** dentro de L1 (E0.W consumer proof). No es mantenimiento y
rompe la regla SemVer de v0.98.x como línea de patch, así que
encaja en su propia serie `F0.*` y requiere release minor (no patch).

- **Scope**: subcomando CLI nuevo en `crates/cognicode-cli/`.
- **Prereq**: contrato E0 estable (L1); reconciliación L0 hecha.
- **Severidad**: baja (mejora UX, valor real).
- **Trigger**: carry-over PRF convertido en feature Post-PRF.
- **Estado actual en ROADMAP**: F0.1 PENDING, L1.
- **SemVer esperado**: minor (`v0.99.0` o lo que la política E0 establezca al cierre) — NO patch.

## E3 (RPC mínima Post-PRF)

**E3 — RPC mínima Post-PRF**: NO_TRIGGERED. Definido por el Post-PRF
original como RPC de lectura condicionada a un segundo cliente real
que requiera proceso separado, concurrencia o reutilización que MCP
local no resuelva. Sin ese consumidor, no se abre.

- **Estado**: registrado como `NOT_TRIGGERED` en ROADMAP §2.
- **Si aparece trigger**: abrir con ADR + UAT + caso de negocio específico.

## Criterios de cierre (M0.*)

1. Test RED de regresión o caracterización.
2. Fix mínimo.
3. Test verde.
4. Si toca binario: bump SEMVER patch (`v0.98.2` por cada cierre o agrupación, según §release del roadmap).
5. Gate `merge-gate` verde en el PR.
6. Si toca docs de release o README: actualizar.

## Cómo NO se hace mantenimiento

- No se mezcla con features (no se mezcla M0 con E0..E2 ni con F0.*). Por eso `find_usages` (ahora F0.1) sale de aquí.
- No se reabre PRF ni se justifica con su roadmap.
- No se reabren certificaciones C# anteriores para "incluir" la corrección; las C# quedan como firma del estado en su fecha.

## Cómo se decide agrupar o separar releases

- Si dos M0.x tocan el mismo binario (cogh, cognicode-mcp) y no hay dependencias entre ellos → se pueden agrupar en un solo `v0.98.2`.
- Si hay dependencias (M0.2 fmt-fix toca todo el repo, M0.1 toca solo cogh) → se pueden hacer dos releases separados: `v0.98.2` con M0.2 y `v0.98.3` con M0.1, o ambos juntos.
- M0.3 NO requiere release por sí mismo (clippy ya estaba strict, moldql ya estaba testeado). M0.3 es "verificación de carry-over", no código.
- Las features F0.* (como F0.1 `find_usages` CLI) NO entran en la numeración v0.98.x. Se numeran aparte (`v0.99.0` minor o lo que la política E0 establezca).
- Decisión la toma el operador en cada cierre.
