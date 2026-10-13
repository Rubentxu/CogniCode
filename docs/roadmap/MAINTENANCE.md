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
| **M0.10** | **walker-grammar-drift** (descubierto post-M0.6): los walkers `walk_php_type_refs` y `walk_swift_type_refs` en `crates/cognicode-core/src/infrastructure/parser/type_ref_walkers.rs` pinean nombres de nodo del grammar tree-sitter-php 0.23 / tree-sitter-swift 0.7.3 que ya no emite el grammar actualizado (tree-sitter-php 0.24.2 + tree-sitter 0.27). Síntoma: 3 de los 4 tests `#[ignore]` PHP/Swift fallan con `node type 'function_definition'/'class_declaration' not found in source` en lugar de `LanguageError`. Efecto observable usuario: `cognicode analyze` sobre proyecto con `.php`/`.swift` reporta `Languages: {}` y `parsed_files=0` porque `find_all_symbols_with_path` no extrae symbols (cadena causal: walker no encuentra nodo → symbols.empty → parsed_files=0 → Languages vacío). **NO scope de M0.6** (sería scope-creep). | **CLOSED 2026-09-27** (commits `2becec6a` parser core + `4eacab93` walker + `a47cf419` acceptance; 3 commits atómicos). Causa raíz más profunda de lo documentado en JOURNAL N+11: **dos capas** de grammar-drift — (a) parser central `tree_sitter_parser.rs`: `Language::Php.function_node_type()` y `Language::Swift.function_node_type()` devolvían `method_declaration` (incorrecto para funciones libres; el grammar emite `function_definition` y `function_declaration` respectivamente), y `find_identifier_name()` solo reconocía `identifier`/`type_identifier` pero el grammar actualizado emite `name` (PHP) y `simple_identifier` (Swift); (b) walker layer: `formal_parameter`→`simple_parameter`, `interface_base`→`class_interface_clause` (PHP); `inheritance_specifier` ahora repeated children, no field-name; return-type es child con field `name` cuyo kind es type node (Swift). `collect_type_names()` extendido con `name` + `named_type`. Validado con `cargo test --workspace` 5651/0/33 (vs 5565/0/37 pre-M0.10 = +86 tests, -4 ignored), `cargo test -p cognicode-core --test m10_acceptance` 6/0/0, `cargo fmt + cargo clippy -D warnings` exit 0. Los 4 tests `#[ignore]` re-habilitados y verde. **`collect_type_names` pineando contratos**: 6 acceptance tests rojos-verdes vía API pública `find_all_symbols_with_path`. | Detectado post-M0.6 cierre durante self-grill del sistema (commit `64235846` aceptación + entrada JOURNAL N+11). |
| **M0.11** | **rustdoc broken_intra_doc_links follow-up** (deuda arrastrada de M0.8): los 147 warnings `broken_intra_doc_links` que el cierre de M0.8 dejó abiertos. Drift localizado en refs `[\`Symbol\`]` a rutas refactoradas (predominante en `application/services/` y `promotion_authority/` post-refactor M9). Requiere auditoría semántica por símbolo (¿qué target intended tenía cada docstring original?) — no es bounded-cleanup mecánico. Estimación: 3-5 días-persona. Trigger: decisión operador (¿vale la pena el esfuerzo para un drift puramente documental?). Riesgo: bajo (solo docs, no afecta runtime). Blast radius: 14 archivos, mayoría en `cognicode-core`. | **CLOSED 2026-09-27** (ciclo `p-c1fac1fea05615c6/m011-rustdoc-intra-doc-links`, WorkItem `3aceb7be-15de-4d04-957c-63b823b8cc7c`, gate `e4f7641a`). Cerrada: la auditoría semántica resultó ser el trabajo, y la premisa del backlog era falsa. El conteo real fue **92**, no 147 ni 82. `cargo doc --workspace --no-deps` deja **0 warnings**, verificado por gate automático (`m011_rustdoc_gate`, 2/2 verde) que corre en cada `cargo test`. Cada enlace se verificó contra el símbolo real antes de escribirse; donde el símbolo ya no existía, el enlace se degradó a texto plano en vez de repuntarse a un sucesor arbitrario — enlace correcto en apariencia pero semánticamente falso es peor que enlace ausente. Único uso de supresión: un `cfg_attr` local a `domain/evidence_kernel` condicionado a `not(feature = "evidence-kernel")`, defendible porque rustdoc lee el build por defecto donde los módulos aún no existen; con el feature activo los enlaces resuelven y el lint queda armado. Sin cambios de lógica. | La deuda no se 잊ó: ahora es imposible reintroducirla sin romper el gate. La exclusión del test del perfil por defecto que se afirmaba en su cabecera **no existía** — se corrigió la afirmación, no el código, porque excluirlo exigiría un `[[test]]` explícito y un `required-features` lo saltaría en silencio, que es el peor resultado posible para un gate. | Detectado por reauditoría durante turno de cierre M0.10 (lesson 80 — auditoría dirigida post-bump). Lección añadida: Lesson 88 — un conteo de warnings heredado de un backlog nunca es evidencia; contar por cabeceras reales, y verificar cada target contra el símbolo antes de enlazar. |
| **M0.12** | **#[ignore] audit** (auditoría dirigida lesson 70/79/80): 27 ocurrencias de `#[ignore]` en workspace categorizadas en 5 grupos. 3 bounded benchmark tests re-habilitados con contract assertions reales: `test_lightweight_index_real_project_benchmark` (pin >100 symbols, 5s, mide 10708 symbols / 28337 locations), `test_on_demand_graph_real_project_benchmark` (pin OnDemandGraph queryable, 10s), `test_debug_call_relationships_in_real_code` (pin find_call_relationships + PetGraphStore integration sobre analysis_service.rs, 0.1s). 2 tests `#[ignore]` messages actualizados (Lesson 79) para cross-referenciar los bounded benchmarks como evidencia de que el path end-to-end silenced funciona (`test_real_code_analysis_workflow`, `test_enhanced_call_graph_features`). | **CLOSED 2026-09-27** (commits `b482a4ff` graphs + `0132e260` analysis_service). Validación: `cargo test -p cognicode-core --lib` 2220/0/15 → **2223/0/12** (+3 tests, -3 ignored exactos); `cargo fmt + cargo clippy -D warnings` exit 0; tests individualmente verificados. 14 lessons nuevas (79-84): Lesson 79 (silenced end-to-end path), Lesson 80 (grammar-drift 2 capas, ya formalizada en N+17), Lesson 82 (cargo test --workspace aborta silenciosamente al primer fail), Lesson 83 (#[ignore] audit debe distinguir bounded de unbounded), Lesson 84 (UAT tests que pinean paths absolutos son frágiles). | Detectado por reauditoría post-M0.10 (lesson 70 patrón). |
| **M0.13** | **target-dir UAT mismatch** (descubierto durante M0.12): el test `crates/cognicode-cli/tests/prf_cli_01_exhaustive_uat.rs:14-21` pinea `target/release/cognicode` (path absoluto, relativo a raíz del repo) pero `~/.cargo/config.toml` (global) override `target-dir = "/var/home/rubentxu/cargo-targets"`, por lo que `cargo build --release --bin cognicode` produce el binario en `/var/home/rubentxu/cargo-targets/release/cognicode` y NO en `target/release/cognicode`. Esto rompe 7 tests UAT (`doctor_valid_cwd_is_honest_and_signal_free`, `analyze_valid_dir_exits_zero_with_output`, `help_and_version_for_every_stable_subcommand`, `graph_subcommands_nonexistent_path_do_not_exit_zero`, `index_valid_dir_exits_zero`, `navigate_nonexistent_symbol_is_honest`, `unknown_command_is_refused_not_silent_zero`). Pre-existente (no introducido por M0.12). Fix transitorio aplicado en N+19: copiar binario a `target/release/cognicode` manualmente. **Fix durable**: cambiar el helper `cognicode_bin()` para usar `common::binary_path("cognicode")` (4 archivos CLI UAT) y `common::release_dir()` para los tests que empaquetan payloads (2 CLI release-flow + 1 MCP two-process). | **CLOSED 2026-09-27 N+20** (commits `555ed54c` + `89fffd58`). Validación: 9 archivos modificados (2 `common/mod.rs` con helper `release_dir()` añadido, 6 test files refactorizados para usar el helper, 1 con `repo_root()` también dedup); `cargo test --workspace` 5650/0/30 → **5668/0/30** (+18 tests del módulo `common::tests` que ahora se cargan); el workaround `cp binario a target/release/` ya NO es necesario (validado borrando el bin stale y re-ejecutando); probe output confirmó `release_dir() = /var/home/rubentxu/cargo-targets/release` como sibling de `debug/`. Lesson 84 (`UAT tests que pinean paths absolutos son frágiles`) formalizada en N+19 validada empíricamente con la cadena de duplicación 7-archivos. **Descubrimiento crítico**: `Cargo` NO exporta `CARGO_TARGET_DIR` a subprocesses de integration test — sólo `CARGO_BIN_EXE_<name>`. Por eso `release_dir()` se deriva de `binary_path()` ya resuelto (parent `release` / sibling `release` / workspace fallback), no del env var. | Detectado durante M0.12 al ejecutar `cargo test --workspace` (7 UAT tests fallaron, no regresión del cambio). |

## F0.1 (evolutivo, fuera de MAINTENANCE)

**F0.1 — `find_usages` CLI wrapper sobre MCP tool**: feature Post-PRF
que expone la MCP tool `find_usages` (ya existente) como CLI
command. Sirve como primera **prueba de consumidor real del contrato
E0** dentro de L1 (E0.W consumer proof). No es mantenimiento y
rompe la regla SemVer de v0.98.x como línea de patch, así que
encaja en su propia serie `F0.*` y requiere release minor (no patch).

- **Scope**: subcomando CLI nuevo en
  `crates/cognicode-core/src/interface/cli/commands.rs`
  (clean-architecture: la CLI vive en `cognicode-core`
  como interfaz; el bin dispatch está en `cognicode-cli`).
- **Prereq**: contrato E0 estable (L1); reconciliación L0 hecha.
- **Severidad**: baja (mejora UX, valor real).
- **Trigger**: carry-over PRF convertido en feature Post-PRF.
- **Estado actual en ROADMAP**: **CLOSED 2026-09-25** (cf.
  `docs/roadmap/ROADMAP.md` §42). Commits:
  - `3cb07f90 feat(cli): find-usages subcommand (F0.1 / L1.4)`
  - `881c0072 test(find_usages): E2E characterization of
    MCP handler (L1.4.W1)`
  - 14 tests verde en limpio + 4 E2E adicionales en
    `find_usages_mcp_handler_e2e.rs` y
    `find_usages_cli_mcp_equivalence.rs` (verificados
    en este turno N+22 con
    `cargo test -p cognicode-core --test
    find_usages_cli_mcp_equivalence` = 4/0/0 y
    `cargo test -p cognicode-core --test
    find_usages_mcp_handler_e2e` = 4/0/0).
  - ADR-PRF-008 architectural review cerrada.
- **Nota de inventario (N+22, 2026-09-27)**: MAINTENANCE.md §26-§40
  contenía un estatus previo que describía F0.1 como PENDING y
  como 'fuera de MAINTENANCE' (inversión contradictoria). Esta
  nota reconcilia a la autoridad de `ROADMAP.md` §42 — F0.1 está
  CLOSED y esta entrada se conserva solo como rastro del registro
  evolutivo original (para que un lector que consulta
  MAINTENANCE entienda por qué F0.1 NO aparece en la tabla de
  pendientes M0.*).
- **SemVer esperado en su release**: minor (serie `F0.*`), no patch.
  El release v0.99.2 (2026-09-27, JOURNAL N+21) integró la
  rama que contiene los commits `3cb07f90` + `881c0072` dentro
  de su fast-forward (vía `arch/cr-06-application-fitness-functions`
  → main). Los commits F0.1 están materialmente en `origin/main`
  como parte de v0.99.2.

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
* **M0.6**: **CERRADA 2026-09-27** (commit `e2ee94ad`, "bump tree-sitter 0.24.7 →
  0.27.0, fixes PHP/Swift LanguageError version mismatch"). Reconciliado 2026-10-01:
  la fila de la tabla M0.6 ya decía CLOSED desde el 2026-09-27 y la fila de aquí
  decía BLOCKED. La tabla gana; el commit existe. La deuda residual que esta
  línea declaraba **viva**, `walker-grammar-drift`, **también está cerrada**:
  se resolvió como **M0.10** (commits `2becec6a` + `4eacab93` + `a47cf419`,
  2026-09-27) y los 4 tests PHP/Swift se re-habilitaron. Verificado de nuevo
  el 2026-10-01 contra `HEAD`, no contra la prosa heredada:
  `cargo test -p cognicode-core --lib type_ref_walkers` → **13 passed /
  0 failed / 0 ignored**, sin ningún `#[cfg(test)]`-less ni `#[ignore]` en ese
  módulo. Esta frase decía *"3 de 4 tests `#[ignore]` de PHP/Swift ahora
  fallan por `node type 'X' not found`"*; era cierto el 2026-09-27 y dejó de
  serlo el mismo día, un commit más tarde. La tabla M0.10 (arriba) es la
  autoridad y dice CLOSED.
* **F0.1** (find_usages CLI): **CERRADA 2026-09-25** (commit `3cb07f90`,
  `feat(cli): find-usages subcommand (F0.1 / L1.4)`). Reconciliado 2026-10-01: la
  sección F0.1 de más arriba ya lo daba por cerrado y esta línea lo daba por
  PENDING. El commit existe.
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

## SDDK-107 — repair local ledger storage

Detectado durante el release v0.99.2 (JOURNAL N+21, 2026-09-27
10:13 UTC). El flow `sddk cycle start` falla con
`sqlite storage error: no such table: ledger_events`:

* `sddk version` →
  - `binary: 1.145.1`
  - `framework: 1.171.2`
* Schema mismatch: el binario activo espera
  `ledger_events` pero la DB local
  (`~/.local/share/sddk/data/ledger.sqlite`) tiene
  un schema más antiguo sin esa tabla.
* `sddk ledger verify-chain` pasa (stream vacío,
  status PASS), pero `sddk ledger verify` y
  `sddk cycle start` fallan.

Impacto:

* `sddk release apply --route local` requiere
  `--cycle <CYCLE>` y por tanto está bloqueado.
* `sddk release vault` requiere `--cycle <CYCLE>`
  con ciclo no terminal (BLOCKED) — bloqueado.

Workaround aplicado (N+21): release material via
git nativo (`git push origin main` + `git tag -a
v0.99.2` + `git push origin v0.99.2`) con archivado
manual vía CHANGELOG + JOURNAL. El release es
completo y trazable, lo que se pierde es la
verificación automática post-release que sólo
SDDK provee.

Decisión: la reparación del storage se delega al
operador porque requiere decisión de orquestación
(¿regenerar el DB local? ¿downgrade del binario?
¿actualizar el binario via `sddk dev install`?).
Trigger pendiente.

**Estado**: **RESUELTO 2026-09-27** vía binario `1.169.121`
(ver `ROADMAP.md` fila SDDK-107). Reconciliado 2026-10-01: esta
línea seguía en OPEN pese a que `ROADMAP.md:46` ya lo daba por
resuelto. El binario instalado hoy es `sddk 2.4.2`, muy por encima
de la versión que lo arregló, y el ledger de planning de este
checkout responde con normalidad, así que el bug de la tabla
`ledger_events` no se reproduce. El trigger pendiente de las
líneas de arriba se descarta; el workaround de release por git
nativo queda como historia, no como deuda abierta.

**Lesson 85** — formalizada: el bypass implícito
del required-check `merge-gate` por la API de GitHub
funcionó en este release, pero futuros operadores
deberían considerar si prefieren enforce estricto
vía repo settings antes de invocar `git push origin
main` por bypass.

---

### 2e. Bisección del SIGABRT — localised, sin causa raíz

Sesión 2026-10-13, HEAD `7b638d18`.

**Por qué gdb no daba nada antes**: el perfil release global de
`~/.cargo/config.toml` fija `debug = false` y `strip = true`, así que el
binario de `cargo-targets/release` sale *stripped* y el backtrace solo traía
direcciones sin resolver. No era un bug del símbolo: era una configuración.
Rebuild con `CARGO_PROFILE_RELEASE_DEBUG=2 CARGO_PROFILE_RELEASE_STRIP=none`
en un `CARGO_TARGET_DIR` aparte (para no invalidar la caché compartida) da un
binario de 335 MB, `with debug_info, not stripped`. Sin gdb, ese binario
aborta en ~4 s de forma determinista (`exit 134`, `corrupted size vs. prev_size`).

**Nota de método**: dos intentos previos de gdb no reprodujeron nada porque
el servidor se lanzaba con `--transport stdio`, flag que **no existe** en
`cognicode-mcp` (es stdio por defecto; `--help` sólo acepta `--cwd`,
`--read-only`, `-h`, `-V`). El proceso moría con exit 2 por argumento
inválido, no por el defecto. Faltaba leer `--help` antes de asumir el flag.

**Hipótesis levantadas y DESCARTADAS por medición** (no por suposición):

- `GraphCache::get_ref` (`graph_cache.rs:141`) era el único `unsafe` en la
  ruta: devolvía un `&CallGraph` con vida ligada a `&self`. **Es realmente
  insound** — un `set`/`update` posterior suelta el Arc del head y el pointeur
  queda colgando; el `Box::leak` sólo fija un clon, y el guard de
  `ArcSwap::load` se cae al salir de la función. Se elimina en `bd4051c6`.
  **Pero no causaba este abort**: la función tenía *cero* llamadores en el
  crate, los binarios y los tests. soundness real, causa no.
- `child(i)` de tree-sitter 0.27: sospechado de tomar offset en bytes. Falso:
  el binding Rust expone `pub fn child(&self, i: u32) -> Option<Self>` por
  índice, y `Node` es refcounted, así que no puede quedar colgante. Descartado
  leyendo `binding_rust/lib.rs:1829`.
- ABI de gramáticas contra runtime 0.27: las 30 cargan y parsean (suite
  existente, 0 fallos). Descartado de nuevo.
- Symlinks con `follow_links(true)`: existen 40 symlinks in-tree y varios
  duplican contenido (`rust-intelligence -> sandbox/fixtures/rust-intelligence`),
  pero **ninguno** apunta a un ancestre, luego no hay ciclo. Duplica ficheros
  indexados; no corrompe un heap. Descartado como causa del SIGABRT.
- Rayon: `build_project_graph` parsea con `into_par_iter`, pero
  `TreeSitterParser` usa cache thread-local (`PARSER_CACHE.with`) y cada
  parser lleva su propio `Arc<Mutex<TsParser>>`. Sin dato compartido mutable.
  Descartado como causa.

**Lo que sí queda establecido**:

- Backtrace symbolizado (thread 131 `tokio-rt-worker`, SIGABRT):

  ```
  #4  malloc_printerr ()                        ← "corrupted size vs. prev_size"
  #5  unlink_chunk.isra ()
  #6  _int_malloc ()
  #14 RawVecInner::try_allocate_in (capacity=21)
  #25 SymbolFqn::from_legacy_side       symbol_fqn.rs:66
  #26 Symbol::new                       symbol.rs:34
  #27 node_to_symbol_with_path          tree_sitter_parser.rs:276
  #28 find_all_symbols_with_path        tree_sitter_parser.rs:199
  #29 LightweightIndex::build_index     lightweight_index.rs:170
  #31 handle_build_lightweight_index    handlers/mod.rs:3664
  ```

- El abort ocurre en un `malloc` de 21 bytes dentro de Rust **100% seguro**
  (`&str` → `String`). glibc no detecta ahí la corrupción: la **detecta** en
  la primera `malloc` posterior a una escritura inválida hecha antes. El sitio
  del backtrace es el *testigo*, no el culpable.
- Por tanto el fallo **no** está en el código Rust del camino `build_index`
  (que es seguro y no comparte estado mutable), sino en una escritura fuera de
  límites dentro de tree-sitter o de un crate C de gramáticas. Coherente con
  que `build_graph` pase: su `build_project_graph` cachea por fichero y evita
  el patrón exacto que dispara el fallo.
- `build_project_graph` es la ruta real de `build_lightweight_index`; la
  diferencia con `build_graph` no está en qué función se llama sino en que
  `build_graph` camina con `follow_links(false)` y el índice ligero con `true`.
- El fallo es de **concurrencia/interoperación**, no de gramática ni de
  volumen: el mismo repo pasa en `build_graph`, y el camino directo y
  monothread de `LightweightIndex::build_index` **no** aborta (corre >20 min
  sin morir). Sólo el servidor aborta.

Corrección de método: el test de repro tenía `parent().join("crates")`, que
apuntaba a `crates/crates` (inexistente) y fallaba por el motivo equivocado.
Arreglado en `7b638d18`.

**Estado**: la causa raíz del SIGABRT sigue **sin identificar**. El
`odr-violation` de 2f es real pero queda descartado como causa en 2h.

### 2f. ODR violation de `map` — hallazgo real, pero NO es la causa del abort

AddressSanitizer, con **Rust y C** instrumentados, reporta:

```
ERROR: AddressSanitizer: odr-violation (0x56098f0e05e0):
  [1] size=100 'map' src/parser.c:4347:7 in cognicode-mcp
  [2] size=100 'map' src/parser.c:4381:7 in cognicode-mcp
SUMMARY: AddressSanitizer: odr-violation: global 'map' at src/parser.c:4347:7
```

**Mecanismo** (`tree-sitter-0.27.0/src/parser.h:203`):

```c
#define ADVANCE_MAP(...)                                              \
  {                                                                   \
    static const uint16_t map[] = { __VA_ARGS__ };                    \
    for (uint32_t i = 0; i < sizeof(map)/sizeof(map[0]); i += 2) {     \
      if (map[i] == lookahead) { state = map[i+1]; goto next_state; }  \
    }                                                                 \
  }
```

`ADVANCE_MAP` declara un `static const uint16_t map[]` dentro del cuerpo de
la función con un nombre fijo. El C generado de cada gramática la invoca
docenas de veces (31 en `tree_sitter_rust`), así que un mismo `.o` contiene
muchos arrays distintos llamados todos `map`. Al enlazar 30+ gramáticas en un
binario, el enlazador les asigna a todos el símbolo `map`.

**Censo de símbolos** (verificación independiente del sanitizer):

```
$ nm cognicode-mcp | grep -cE '\bmap\.[0-9]+'
1936
$ nm cognicode-mcp | grep -E '\bmap\.[0-9]+' | awk '{print $1}' | sort -u | wc -l
1934
```

1936 arrays de bloque estático, 1934 direcciones distintas: hay nombres
solapados. El hallazgo es real.

### 2g. CORRECCIÓN: el `odr-violation` NO causa el `corrupted size`

**Este apartado corrige una conclusión anterior mía.** Escribí que la
colisión de `map` explicaba el `corrupted size vs. prev_size`. **No lo
explica**, y lo he descartado por medición directa:

`crates/cognicode-core/tests/map_symbol_collision_repro.rs` enlaza las
gramáticas reales en un binario de test y parsea el árbol `crates/` con la
gramática de verdad, bajo ASan **con `detect_odr_violation=0`** para que la
corrida no pare en la queja de nombres y siga hasta el fallo de memoria real:

```
test map_symbol_collision_corrupts_heap_under_asan ...
     parsed 915 files across 5 grammars, 53281 symbols
test result: ok. 1 passed; 0 failed
ERROR: AddressSanitizer reports: 0
```

Cero errores de ASan. 915 ficheros, 5 gramáticas, 53281 símbolos, ninguna
escritura fuera de límites. Si la colisión fuera la causa del abort, este
test habría fallado aquí.

Por qué el ODR violation **no** corrompe memoria, a pesar de lo que sugiere
el mecanismo: los dos globales que ASan señala tienen **el mismo tamaño
(`size=100`)** y **la misma dirección**. El enlazador plegó copias de datos
idénticos o equivalentemente alineados. `sizeof(map)/sizeof(map[0])` se
evalúa con el tamaño que compiló cada copia, y como el tamaño coincide, el
bucle `for` recorre el mismo número de elementos y lee dentro de los límites.
La colisión es un defecto de nombres real que **debe** corregirse, pero es
latente y benigno en esta forma: no produce corrupción de heap.

**Estado real del defecto del SIGABRT: sin causa raíz identificada.** Lo que
sabemos con certeza:
- Es determinista (~4 s), sólo en el proceso del servidor MCP, no en el
  camino directo y monothread de `LightweightIndex::build_index`.
- glibc lo detecta en un `malloc` de 21 bytes dentro de Rust seguro
  (`SymbolFqn::from_legacy_side`), o sea el backtrace es el *testigo*.
- No es de gramáticas (las 30 cargan y parsean), ni de symlinks, ni de
  rayon, ni de la colisión de `map`.
- Bajo ASan no se reproduce dentro del timeout de 30 s: depende del ritmo.

Siguiente paso honesto: instrumentar **el runtime C de tree-sitter durante
el propio camino que aborta**, no un test de parseo aparte.

### 2i. Corrección de método: el primer test de parseo no cubría el caso

El test de 2g reimplementaba el bucle de parseo a mano. Eso lo hace más débil
que llamar a la cosa real, por dos motivos que medí:

1. **No cubría el `WalkFilter` ni el `follow_links(true)`** que hace
   `build_index`, así que los árboles de fixtures alcanzados por symlink no
   se indexaban.
2. **Sólo alcanzaba 5 de las 30 gramáticas.** Medido con el test rápido
   `repo_grammar_breadth_is_measured_not_assumed`:

   ```
   repo contains 30 distinct languages: [Json, Python, TypeScript, Bash,
   JavaScript, Yaml, Rust, Java, Erlang, Dart, SystemVerilog, R, Julia,
   Zig, PowerShell, Haskell, Groovy, Fortran, Scala, Lua, Verilog, Go, Hcl,
   CSharp, Cpp, C, Swift, Ruby, Elixir, Php]
   ```

   Un verde sobre 5 gramáticas no dice casi nada cuando el repo tiene 30.
   `Language::to_ts_language` enlaza las 30 al binario, y la amplitud de
   gramáticas es exactamente la variable sospechosa.

Corregido en `0204a554`: el test ahora llama a
`LightweightIndex::build_index` sobre la raíz del repo, que es el punto de
entrada de producción. Sigue bajo ASan con `detect_odr_violation=0`.

**Aviso sobre el método de execução**: el primer intento de correr este test
bajo ASan dio un falso "vacío". Dos errores míos, no del defecto:
- El script envolvía la salida en `tail -120`, que no emite nada hasta que el
  proceso acaba, así que un log vacío parecía un fallo.
- Al esperarlo desde una sesión con timeout de 10 min, el proceso de test
  moría sin que su salida llegara a escribirse. Un `pgrep` contra el nombre
  del binario matcheaba su propio shell y daba la impresión de que seguía
  vivo. Comprobación real: `ps -o time=` sobre el PID concreto.
- Una recarga del servidor abortó la sesión y mató el proceso en segundo
  plano. `nohup ... &` no sobrevive; hace falta `setsid setsid ... < /dev/null`
  para desacoplarlo del ciclo de vida de la sesión.

**Estado de la corrida decisive** (ASan Rust+C sobre el `build_index` real):
tras **~2 h de CPU acumulados** (CPU ≈ transcurrido, o sea sin deadlock),
**cero** reportes de AddressSanitizer. El binario sin instrumentar aborta en
~4 s; el mismo trabajo bajo ASan sigue vivo. Refuerza que la corrupción
depende del ritmo, y que el `odr-violation` de 2f no la provoca.

Con las 30 gramáticas, el `WalkFilter` real y el `follow_links(true)` real —
o sea, la producción tal cual — el ASan **no ha visto ninguna escritura fuera
de límites en dos horas de CPU**. Ese es el dato que importa, y no depende
de que la corrida termine: para que la colisión de `map` provocara el
`corrupted size`, habría tenido que saltar en algún momento de ese trabajo.
La corrida seguía en marcha al cerrar la sesión (el `build_index` completo
bajo ASan es cuestión de horas), así que su código de salida final queda
**pendiente**, pero el desenlace causal ya está medido.

Lección operativa: para una corrida larga y silenciosa hay que desvincularla
del wrapper, escribir la salida incrementalmente y verificar el PID real, no
el patrón de nombre. Un "no salió nada" sólo es evidencia si el proceso
terminó por su cuenta y su código de salida está registrado.

**Sobre el símbolo `map`**: pendiente de arreglar por ser un `odr-violation`
legítimo, pero **no es el bloqueante de release** que buscaba. No es un
fix de una línea y cambia cómo se enlaza el binario; se deja como deuda
técnica declarada, no como causa del crash.

### 2h. ASan: cómo se llegó al diagnóstico

El camino del stack es C: `tree-sitter` 0.27 compila su runtime con `cc`
dentro de su `build.rs` (`src/parser.c`, `stack.c`, `lexer.c`, ...). Instrumentar
sólo Rust no instrumenta esos objetos, y por eso el primer ASan **no vio nada**.

- **Intento 1** (`-Zsanitizer=address` en `RUSTFLAGS` global): falló al enlazar.
  Los proc-macros también se instrumentan, son cdylibs cargados por rustc y no
  tienen runtime de ASan →
  `undefined symbol: __asan_option_detect_stack_use_after_return`.
- **Intento 2** (`--target x86_64-unknown-linux-gnu` + `-Z build-std=std,panic_abort`,
  que deja los proc-macros para host sin instrumentar): **compiló y corrió**.
  Resultado observado, y es un dato en sí mismo:

  ```
  tool_call tool=build_lightweight_index duration_ms=30001 status=error
  {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text",
   "text":"internal: timeout"}],"isError":true}}
  ```

  Sin Abort, sin `corrupted size`, y **cero** reportes de AddressSanitizer.
  El servidor sobrevivió y devolvió `isError: true` con timeout de 30 s.
- **Intento 3** (el que diagnostica): `CFLAGS`/`CXXFLAGS` con
  `-fsanitize=address` para instrumentar también el C de tree-sitter.
  Verificado antes de correr: el objeto del runtime
  (`build/tree-sitter/<hash>/out/<hash>-lib.o`) pasó de 0 a **23 referencias
  `__asan`**, así que esta vez el C sí estaba instrumentado. Resultado: el
  `odr-violation` de 2f.

Consecuencia útil del intento 2: bajo ASan (5-10x más lento) el
`build_lightweight_index` **no llega a corromper el heap en 30 s**, mientras que
el `corrupted size vs. prev_size` del binario normal aparece a los ~4 s. La
corrupción depende del **ritmo**: una ventana temporal o de reutilización de
memoria que ASan al ralentizar no alcanza dentro del timeout.

**Defecto de contrato adicional descubierto en vivo**: `build_lightweight_index`
sobre este repo tarda más de 30 s y el servidor responde `internal: timeout`
(`isError: true`, `duration_ms=30001`). El timeout viene de
`timeout_for_category` en `rmcp_adapter.rs` (30 s para `file`, `composite`,
`quality`, `refactor`, `aix`, y default). Es decir: la herramienta anunciada no
puede indexar un repo de tamaño real dentro del presupuesto que el propio
servidor le impone. Aunque se arregle el abort, la herramienta seguiría siendo
inaccesible en este repo salvo que el trabajo se haga incremental o el
presupuesto sea mayor.

## Deuda de gates estáticos — G2 de C8-R (medida 2026-10-13, HEAD `097e7b4a`)

Medido con el compilador, no con aserciones de roadmap. Ambos fallos son
preexistentes a esta sesión: reproducen idéntico en `80974345`.

### `cargo fmt --all -- --check` — 21 ficheros, 97 hunks

Ficheros: `cogh.rs`, `skills.rs`, `lifecycle.rs`,
`b6_w3_cogh_lifecycle.rs`, `b6_w4_skills_autodiscovery.rs`,
`b6_w2_reproducible_perf.rs`, `commands.rs`, `robustness.rs`,
`refactor/{plan,apply,preview,revalidate,mod}.rs`,
`b5_w{1,2,3,4}_*.rs`, `b6_w{1,2,5,6}_*.rs`,
`prf_sec_02_read_only_uat.rs`.

Los tres ficheros que esta sesión tocó (`intent.rs`, `rmcp_adapter.rs`,
`prf_sec_02_read_only_uat.rs`) se corrigieron en `097e7b4a`. `apply.rs` y
`preview.rs` siguen sin formatear a propósito: su único cambio fue el enlace
intra-doc, y formatearlos habría arrastrado hunks ajenos al mismo commit.

### `cargo clippy --workspace --all-targets -- -D warnings` — 9 errores en 4 ficheros

| Fichero | Errores | Lint |
|---|---|---|
| `refactor/revalidate.rs` | 2 | `collapsible_if` |
| `refactor/preview.rs` | 2 | `collapsible_if` |
| `interface/cli/commands.rs` | 2 | `redundant_closure` |
| `refactor/plan.rs` | 1 | `assign_op_pattern` |

Más un `result_large_err` (`large size difference between variants`).

**Por qué no se cierra aquí**: es reformatting y lint-mecánico sobre código que
esta sesión no cambió de comportamiento. Mezclarlo con correcciones de contrato
los vuelve irrevisables, y el proyecto trata el cleanup de fmt como work unit
propia (ver `scripts/test-fast.sh` / `scripts/test-full.sh` en `AGENTS.md`).

**Efecto sobre C8-R**: el gate **G2** del runbook
(`docs/roadmap/production-ready/runbooks/C8-RECERTIFICATION-RUNBOOK.md:24-25`)
exige ambos comandos en verde. C8-R sigue `PENDING_HUMAN_SIGNATURE` y **no es
firmable** hasta que G2 pase, con independencia de la firma humana. G1, G3 y el
resto de la campaña sí están verdes: `cargo test --workspace` da 208 suites /
6562 passed / 0 failed, y `scripts/product` da 46/46.

---

## Defectos de contrato hallados por verificación en vivo (2026-10-13, HEAD `9fcde296`)

Ninguno de estos lo vio la suite. Todos se hallaron conduciendo el binario
release como cliente MCP real sobre stdio, que es exactamente lo que la
revisión pidió y lo que el suite sintético no cubre.

### 1. Herramientas anunciadas sin brazo de dispatch (4, confirmada en vivo)

`product/tools.json` es la fuente de verdad de `tools/list`. Una entrada del
catálogo sin brazo en `call_tool_handler` es una promesa que el servidor no
puede cumplir: el cliente la descubre por `tools/list`, la llama y recibe
`isError=true` / `tool not found`.

| Herramienta | Autoridad | Estado |
|---|---|---|
| `composite_impact_query` | read | **corregida** en `9fcde296` |
| `safe_refactor_apply` | **mutating** | abierta |
| `safe_refactor_preview` | read | abierta |
| `safe_refactor_revalidate` | read | abierta |

Las cuatro están confirmadas sobre el binario release, no por inspección.

**Por qué el suite era verde**: `test_all_listed_tools_are_dispatchable`
compara `tools/list` contra `dispatchable_tool_names()`, un segundo array
mantenido a mano. Añadir el nombre a ambos documentos satisface el guard
mientras el runtime sigue sin poder enrutar el nombre. El guard probaba que
dos ficheros coincidieran; nunca preguntaba al dispatcher. Es la misma clase de
defecto que el `cogh` fantasma retirado antes en este ciclo.

**Guard derivado** (`tests/undispatchable_tool_guard.rs`): llama a cada
herramienta anunciada por el dispatcher real y falla **solo** ante
`ToolNotFound`. Un fallo no reconocido cae en un bucket `UNKNOWN` en vez de
darse por bueno, para que un modo de fallo nuevo no se esconda dentro de "ok".
Exige `dispatch_tool_for_contract_test`, un seam tras el feature
`test-contract-seam` (off por defecto): los tests de integración compilan sin
`cfg(test)`, así que un re-export `#[cfg(test)]` sería invisible y el guard
dejaría de comprobar nada silenciosamente.

**Por qué el commit está en rojo a propósito**: el guard reporta las tres
`safe_refactor_*` como el mismo defecto. No se han cableado en `9fcde296`
porque cada una necesita su propia revisión de autoridad —
`safe_refactor_preview` necesita el mismo `deny_unknown_fields` que protege
`safe_refactor` contra un `authorization_token` smuggling, y
`safe_refactor_apply` es `mutating`. Dejar el test verde sobre un defecto de
cableado vivo es exactamente lo que este guard existe para impedir.

Servicios de dominio presentes y **sin caller en producción** (mismo patrón, una
capa más abajo): `refactor::preview::render_preview`,
`refactor::apply::apply`, `refactor::revalidate::run_revalidation`. B5.W2/W3/W4
solo los ejercita desde `tests/`.

### 2. `build_lightweight_index` aborta el servidor — corrupción de heap

`tools/call build_lightweight_index` mata el proceso **en todas las
invocaciones**: SIGABRT, glibc `corrupted size vs. prev_size` en `malloc`, en un
hilo `tokio-rt-worker`. No es un `isError`: el servidor desaparece.

Medido:
- `build_graph` sobre el mismo directorio responde OK (2458 ficheros,
  61698 símbolos). No es el volumen.
- `build_lightweight_index` con `{}` y con `{"strategy":"lightweight"}`:
  ambos abortan.
- Un solo fichero por lenguaje (rs, ts, js, tf, md): los cinco OK.
- 359 ficheros `.rs`: OK. 50/150/359 escalados: OK.
- `crates/`, `apps/`, `docs/`, `scripts/`, `sdd/`, `odd/`, `bundles/` por
  separado: todos OK.

Descartado por medición, no por suposición:
- **Gramáticas tree-sitter**: las 30 cargan y parsean contra `tree-sitter
  0.27.0` (`tests/tree_sitter_grammar_abi.rs`, 0 fallos). El
  `Err(_) => TreeSitterParser::new(Language::Rust).unwrap()` de
  `lightweight_index.rs:162` nunca se toma. Sigue siendo una trampa latente —
  un fallo de gramática produciría símbolos Rust silenciosos sobre otro
  lenguaje — pero **no** es la causa de este abort.
- **Símbolos**: el binario release va stripped, así que el backtrace de gdb
  solo da direcciones sin resolver. Hay que repetir contra un build con
  símbolos.

Diferencia estructural pendiente de aislar: `build_graph` camina con
`follow_links(false)` (`handlers/mod.rs:1039,1119`) y
`build_lightweight_index` con `follow_links(true)`
(`lightweight_index.rs:137`). No se ha encontrado ningún ciclo de symlinks
(361 symlinks, ninguno apunta a un ancestre), pero una copia del repo
desreferenciada no resuelve la pregunta: no aborta, pero tampoco termina en
5 min, así que el fallo y el resultado limpio difieren en más que symlinks.

**Efecto sobre C8-R**: reforzar el argumento de que G2 no es el único bloqueo.
Este defecto es de disponibilidad en una ruta de advertised, y una
certificación production-ready no puede firmarse con una herramienta
anunciada que tumba el servidor.

### 2b. Los cuatro fantasmas corregidos — verificado en vivo

`9fcde296` (composite) y `51e826da` (los tres `safe_refactor_*`). Tras el
rebuild release, sobre el binario real:

| Herramienta | Antes | Ahora |
|---|---|---|
| `composite_impact_query` | `tool not found` | OK, con `verdict_refused=true` |
| `safe_refactor_preview` | `tool not found` | `isError` honesto (plan inválido / inexistente / ambos a la vez) |
| `safe_refactor_apply` | `tool not found` | `isError` honesto (plan ilegible) |
| `safe_refactor_revalidate` | `tool not found` | `isError` honesto y explained |

Clasificación viva de las 77 (`scripts/product/classify_tool_errors.py`,
1 proceso por herramienta):
`OK 23 · VALIDATION 49 · CONTEXT 4 · CRASH 1 · MISSING_ARM 0 · UNKNOWN 0`.

**Cero fantasmas.** `CONTEXT` son precondiciones que la propia herramienta
nombra y remedy (`No graph available`, `Run build_graph first`,
`Call relationships not found (0 edges)`) — el opuesto de `tool not found`,
que no nombra remedio porque no hay handler al que llegar.

**Control de autoridad verificado en el cable**: un `authorization_token`
 smuggling en `safe_refactor_preview` se **rechaza** con
``unknown field `authorization_token`, expected `plan_path` or `plan_json` ``,
no se descarta en silencio. Antes de este trabajo el rechazo dependía de que
el cliente notara un campo ignorado.

`safe_refactor_revalidate` **rechaza a propósito**. `run_revalidation` necesita
el conjunto de ficheros cambiados del plan aplicado; un `plan_id` no lo
reconstruye y el resultado del apply no se persiste. Devolver cero ficheros
analizados con `verdict_refused: true` se leería como "comprobado, nada
encontrado" en vez de "nada comprobado". La pieza que falta es un registro
durable del outcome del apply.

### 2c. Superficies CLI verificadas en vivo

`cognicode 0.101.16`, `cogh 0.101.16`,
`cognicode capabilities --json` con `cli_version 0.101.16` y perfiles,
`cogh skills` con 4 bundles `[ok]`.

Claims publicados contrastados contra lo observado:

- README (EN y ES) dice **77 herramientas MCP**; el catálogo tiene 77 y
  `tools/list` devuelve 77. Correcto.
- Las tablas del README suman 35 y el texto dice explícitamente
  *"curated selection — not the full surface"*, con `product/tools.json` como
  lista autoritativa. Correcto y honesto: no es una afirmación falsa.
- Las menciones de `0.101.6`..`0.101.10` en README/SECURITY son historial de
  releases, no el claim de versión actual.

### 2d. Estado del gate tras estas correcciones

- `cargo test --workspace --features cognicode-core/test-contract-seam`:
  **6378 passed, 0 failed**.
- `core_gate_coverage_contract` detectó las dos suites nuevas y hubo que
  registrarlas en el mismo commit (66 → 68, más `ANCHOR_SUITES`). El contrato
  está haciendo su trabajo: una suite que nada ejecuta no puede aparecer en
  silencio.
- `bash scripts/test-fast.sh`: verde. Nota: el selector CR-08 **no** elige las
  suites nuevas; hay que invocarlas explícitamente con
  `--features cognicode-core/test-contract-seam`. Sin revisar.
