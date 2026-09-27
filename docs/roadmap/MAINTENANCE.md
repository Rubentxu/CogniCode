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
| **M0.6** | **PHP y Swift rotos en producción** por incompatibilidad tree-sitter parser vs runtime. `tree-sitter-php = "0.24.2"` y `tree-sitter-swift = "0.7.3"` estaban compilados con `tree-sitter = "0.25"` (parser version 15) pero el workspace usaba `tree-sitter = "0.24"` (runtime `LANGUAGE_VERSION = 14`). Cualquier intento de parsear `.php` o `.swift` con `TreeSitterParser::new(Language::Php)` o `Language::Swift` fallaba con `LanguageError { version: 15 }` en producción. 4 tests `#[ignore]` en `cognicode-core/src/infrastructure/parser/type_ref_walkers.rs:1150,1169,1188,1207` pineaban el bug. Test RED confirmado: `cargo test -p cognicode-core --lib infrastructure::parser::type_ref_walkers -- --include-ignored` → 4 failures (`LanguageError { version: 15 }`). Bug BLOQUEANTE para usuarios que trabajen con PHP o Swift. | **CLOSED 2026-09-27** (commit `e2ee94ad`; bump `tree-sitter = "0.24" → "0.27"` en workspace + alinear `cognicode-core-mock` con `workspace = true`; `LanguageError { version: 15 }` **REPARADO** — producción ahora parsea PHP/Swift sin error runtime; validado con `cargo check --workspace --all-targets` exit 0, `cargo test -p cognicode-core --lib` 2216/0/19 (zero regresión), clippy strict exit 0, fmt exit 0, deny licenses ok; cross-validation en worktree `/tmp/cognicode-m06-bump`). **Deuda residual descubierta** (NO scope de M0.6): 3 de los 4 tests `#[ignore]` PHP/Swift ahora fallan por `node type 'X' not found` en vez de `LanguageError` — el grammar tree-sitter-php 0.24.2 + tree-sitter 0.27 emite nombres de nodo distintos a los pineados por `walk_php_type_refs`/`walk_swift_type_refs`. Registrada como nuevo work item **walker-grammar-drift** para futura iteración. Mensajes `#[ignore]` actualizados para reflejar la nueva realidad. SemVer: bump patch `v0.99.1 → v0.99.2` pendiente decisión operador (binario linked contra nuevo tree-sitter runtime). Opción A elegida sobre B (fork, deuda permanente) y C (Unsupported, cosmético). | Detectado en auditoría dirigida de tests `#[ignore]` post-M0.5 — entrada 24 |
| ~~**M0.3.b**~~ | ~~CLI equivalente a `find_usages` MCP tool.~~ | ~~REASIGNADO A F0.1~~ (ver JOURNAL §7, L0 del Post-PRF) | ~~No era mantenimiento: era feature Post-PRF. La asignación previa a E3 era inconsistente con la definición de E3 (RPC mínima condicionada a un segundo cliente real).~~ | ~~Carry-over PRF, ahora evolutivo~~ |
| **M0.7** | Drift de rustfmt en 4 archivos tras el trabajo de e91.W7/W8 (commits `f340b624`, `690c44a6`) y el flatten de closure_chain en `graph_analytics.rs`. `cargo fmt --all --check` retornaba exit 1 con 6 sitios de drift distribuidos en 4 archivos: `control_query.rs:304` (use-list reorder en test module), `graph_analytics.rs:286` (closure_chain a `iter().filter_map(\|(s, t)\| ...).collect()`), `e91_w7_regression_budget.rs:175,196` y `e91_w8_stage_profile.rs:110,180` (eprintln arg-list rewrap en asserts). Drift invisible porque el workflow `pr-ci.yml` solo dispara en `pull_request` a `main` y `arch/cr-06-application-fitness-functions` solo recibe push. Detectado por auditoría manual con `cargo fmt --all --check` en el turno 9. | **CLOSED 2026-09-26** (commit `b0fe4730`; `cargo fmt --all` aplicado; diffstat 4 archivos, +17/-13 net — solo whitespace; tests 4235/0/19, clippy exit 0; sin cambios de runtime, sin cambios de signature; la próxima PR desde esta branch verá merge-gate fmt verde). Recomendación follow-up (no implementada): añadir pre-commit hook en `.git/hooks/pre-commit` que corra `cargo fmt --all --check` o pinear rustfmt-component version en toolchain para alinear con el runner. | Detectado en auditoría dirigida turno 9 (post-licenses) por el agente. |
| **M0.9** | **cargo-deny licenses gap closure**: añadir `license = "MIT OR Apache-2.0"` a todos los workspace crates que no lo declaraban, cerrando el gap que el allow-list `[licenses]` de deny.toml (commit `f551311c`) dejaba en descubierto. Auditoría previa incompleta: `f551311c` documentó "2 crates flagged" pero el output real de `cargo deny check licenses` listaba **10 workspace crates** sin license (verificado vía `git worktree` sobre el SHA). El nuevo commit `f0708d4b` cierra los 10 con disclosure explícita del miscount ("bumps reales, markers honestos"). License expression sigue el precedent de `cognicode-graph-algos` y `cognicode-graph-wasm` (únicos 2 que ya lo declaraban). | **CLOSED 2026-09-26** (commit `f0708d4b`; 10 archivos modificados, +10/-0; `cargo deny check licenses` exit 0 con "licenses ok"; `cargo check --workspace --quiet` exit 0; `cargo fmt --all --check` exit 0). NO bumped to CI-blocking (eso queda como follow-up operator-gated #3 del JOURNAL N+8). Sin cambios de código, sin cambios de tests. Disclaimers en commit body: (1) git history de `f551311c` no se reescribe; la corrección va forward en `f0708d4b`; (2) `[workspace.package]` no se tocó — el operador puede formalizar license-of-record ahí si lo desea. | Detectado por reauditoría de licencias durante turno 13 (post-M0.7, M0.8, just recipes). |
| **M0.8** | Drift de rustdoc en 14 archivos: 168 warnings retornados por `cargo doc --workspace --no-deps`, ninguna ejecutada por `pr-ci.yml` ni `ci.yml`. Distribución: `cognicode-core` 113 (predominante en `application/services/` y `promotion_authority/` tras el refactor M9), `cognicode-explorer` 29, `cognicode-cli` 4, `cognicode-ladybug` 4, `cognicode-mcp` 1, `cognicode-sandbox` 1, `cognicode-core-mock` 1, `cognicode-runtime` 0. Categorías detectadas: (1) `invalid_html_tags` 4 (URL, CapturedCall, RwLock, JSON — son angle-bracketed tokens que rustdoc 1.96 trata como HTML); (2) `redundant_explicit_links` 17 (form `[\`X\`](path::to::X)` donde `[\`X\`]` ya resuelve); (3) `broken_intra_doc_links` 147 (refs `[\`Symbol\`]` a rutas refactoradas — requieren análisis semántico por símbolo); (4) `private_intra_doc_links` 1. Drift invisible por el mismo motivo que M0.7: pr-ci.yml gate no corre en `arch/cr-06` push. Detectado por auditoría manual con `cargo doc --no-deps` en el turno 10. | **CLOSED 2026-09-26** (commit `d6afaac1`; bounded-cleanup de las categorías mecánicamente corregibles (1) y (2); `invalid_html_tags` 4 → 0, `redundant_explicit_links` 17 → 0; total warnings 168 → 147; 14 archivos, +23/-23 perfect-symmetria whitespace-only). Recomendación follow-up (no implementada): los 147 `broken_intra_doc_links` requieren un ciclo de auditoría dedicado (operator policy: ¿qué target intended tenía cada docstring original?), y considerar promover `cargo doc --no-deps -- -D warnings` como CI gate. | Detectado en auditoría dirigida turno 10 (post-rustfmt) por el agente. |
| **M0.10** | **walker-grammar-drift** (descubierto post-M0.6): los walkers `walk_php_type_refs` y `walk_swift_type_refs` en `crates/cognicode-core/src/infrastructure/parser/type_ref_walkers.rs` pinean nombres de nodo del grammar tree-sitter-php 0.23 / tree-sitter-swift 0.7.3 que ya no emite el grammar actualizado (tree-sitter-php 0.24.2 + tree-sitter 0.27). Síntoma: 3 de los 4 tests `#[ignore]` PHP/Swift fallan con `node type 'function_definition'/'class_declaration' not found in source` en lugar de `LanguageError`. Efecto observable usuario: `cognicode analyze` sobre proyecto con `.php`/`.swift` reporta `Languages: {}` y `parsed_files=0` porque `find_all_symbols_with_path` no extrae symbols (cadena causal: walker no encuentra nodo → symbols.empty → parsed_files=0 → Languages vacío). **NO scope de M0.6** (sería scope-creep). | **OPEN 2026-09-27** | Pendiente: (1) inspeccionar AST real que emite grammar actualizado sobre snippets simples (`tree_sitter_php::LANGUAGE_PHP` sobre `function save(User $user, Repository $repo): void { }`); (2) adaptar `walk_php_type_refs`/`walk_swift_type_refs` a nombres de nodo correctos; (3) re-habilitar 4 tests `#[ignore]`; (4) verificar `cognicode analyze` extrae symbols. Estimación: 1-2 días-persona. Sin tests automatizados pineando el contrato del walker-grammar-drift — requiere inspección manual del AST con `tree-sitter parse` o equivalente. Sin dependencias upstream. Riesgo: medio (cambio de contrato del walker podría afectar otros consumidores del AST, blast radius acotado a PHP/Swift que son walkers nuevos). | Detectado post-M0.6 cierre durante self-grill del sistema (commit `64235846` aceptación + entrada JOURNAL N+11). |

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

## PIVOT 2026-09-26 — fin del backlog M0.*

El pivot al programa **production-ready stabilization** (ver
`docs/roadmap/production-ready/EXECUTIVE-SUMMARY.md`) implica que el
backlog M0.* queda **técnicamente cerrado** como fase de mantenimiento
v0.98.x/v0.99.x:

* **M0.1 a M0.5**: CLOSED (entries 11, 12, 13, 14, 23 del JOURNAL).
* **M0.6**: **BLOCKED con herencia**. No se cierra sin decisión humana,
  pero queda fuera del scope del nuevo programa (CR-* y ST-* no dependen
  de M0.6; el fix de tree-sitter puede ejecutarse en cualquier momento
  del programa o como hotfix independiente si el operador lo decide).
* **F0.1** (find_usages CLI): sigue PENDING, L1 del Post-PRF. No se ha
  priorizado dentro del nuevo programa (queda como carry-over si
  reaparece como bloqueador E0.W consumer proof).
* **E3** (RPC mínima): sigue `NOT_TRIGGERED`. Sin consumidor real que
  lo justifique, no se ejecuta.

### C8 firma operativa 2026-09-26T10:14:47Z

El 2026-09-26T10:14:47Z el operador firma C8 al **nivel operativo**
sobre SHA `3954b8b7` (cuerpo principal del dosier
`docs/roadmap/certifications/C8-POST-PRF-GA.md`). Categoría:
**OPERATIVO** (no contractual). Sin tag anotado ni release formal.

* **Recertificación C8-R** queda abierta como **CR-01** dentro del
  programa production-ready (outcome PR-G2).
* **M0.6** sigue **BLOCKED** pero ya no es bloqueante para CR-01.
* **Expediente**: `docs/prf/ADMISSION-EXPEDIENTE-F8-C8-OPERATIVO-v0.99.0.md`.
* **Bloqueos activos restantes**:
  * M0.6 PHP/Swift tree-sitter bump (decisión del operador entre
    3 opciones: bump `tree-sitter` 0.24→0.25 afecta 18 parsers,
    fork comunitario no oficial, o marcar como `Language::Unsupported`).
  * CR-01 (recertificación C8-R desde clean clone).

El nuevo programa (21 acciones en 3 fases) opera sobre los **outcomes
de estabilización** (PR-G1..PR-DEPTH) y NO reabre el backlog M0.*.
Las features Post-PRF que vivían en el roadmap viejo (E0, E1, E2, F0.1)
se re-evalúan dentro del programa bajo el paraguas PR-DEVEX / PR-PERF.

Ver `docs/roadmap/JOURNAL.md` entry 25 y `docs/roadmap/CURRENT.md` §
"Programa production-ready (Post-PIVOT, no iniciado)" para el
contexto completo del pivot.
