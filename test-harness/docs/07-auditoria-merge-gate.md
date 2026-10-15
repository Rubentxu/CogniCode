# Auditoría de `merge-gate.pipeline.kts` (unidad B1 del evolutivo del test harness)

> **Estado**: CONTRATO CERRADO, AUDITORÍA LEÍDA Y DECISIÓN B1 APLICADA. Las
> mediciones de esta unidad están en §5, con la máquina y la invocación
> declaradas, y lo que no se midió se declara como no medido.
>
> **Procedencia del fichero auditado** (worktree `worktree-test-harness`):
> - SHA de HEAD antes de la unidad: `11b891588e5afb8edebf31d32279c7cc11e0f422`
> - `merge-gate.pipeline.kts`: 787 líneas, 73 declaraciones `stage(` (7 padres con indentación 8, 66 hijas con indentación 12)
> - sha256 del fichero auditado: `4ef652d31f1ee9d3a98b207b185fcb8e6f17784acda1fded8a6e7579e2995340`
> - sha256 de `scripts/ci/pipeline_authority.py`: `bef0155f562df3505850b68178c3bc927683105fee5d8f2b4b552c2a44efa5f3`
>
> La clasificación (`MERGE_REQUIRED`, `RELEASE_REQUIRED`, `CERTIFICATION_REQUIRED`,
> `ADVISORY`) es un inventario razonado, no una autorización: solo la columna
> "Decisión B1" describe lo que esta unidad cambió, y todo lo demás sigue
> bloqueando exactamente lo que bloqueaba. La decisión de reasignar dueño se
> registra en [`../adrs/ADR-TH-005-dueno-unico-por-suite.md`](../adrs/ADR-TH-005-dueno-unico-por-suite.md).
>
> **Aviso sobre mediciones**: las cifras marcadas como `(claim de fuente)`
> provienen de comentarios del propio fichero o de documentos del repo y no se
> reprodujeron una por una. Las duraciones de §5 sí se midieron en esta sesión,
> sobre el entorno que allí se declara.

## 1. Método y fuentes leídas

Lo que se hizo, texto por texto:

1. Lectura completa de `merge-gate.pipeline.kts` (787 líneas, cuerpo `sh(...)`
   de cada stage incluido). Las líneas de comentario se leyeron como
   declaraciones del autor, no como hechos verificados, y se marcan como
   `(claim de fuente)` cuando se citan.
2. Extracción mecánica de las 73 declaraciones `stage("...")` con su número de
   línea y su indentación, para separar padres de hijas. Conteo:
   `grep -c 'stage(' = 73`, 7 a indentación 8 y 66 a indentación 12. Sin
   discrepancia.
3. Lectura de las pipelines hermanas que permiten decidir si algo está duplicado
   fuera del merge: `integration.pipeline.kts`, `certification.pipeline.kts`,
   `release.pipeline.kts`, `release-candidate.pipeline.kts`,
   `product-fast.pipeline.kts`. Solo listado de stages y cuerpos `sh(...)`.
4. Lectura de la autoridad única de wiring: `scripts/ci/pipeline_authority.py`
   (`MERGE_AUTHORITY = "merge-gate.pipeline.kts"`, con `pipeline_paths()`,
   `steps()`, `pipeline_steps()`).
5. Lectura de los contratos que pinan pertenencia a la autoridad de merge:
   `scripts/ci/test_supply_chain_gate_contract.py` (verificado: asigna
   `cargo deny check advisories`, `cargo deny check licenses` y la instalación de
   `rust-analyzer` a `authority.MERGE_AUTHORITY`), y la lista completa de
   `scripts/ci/test_*.py`.
6. Lectura de scripts consumidos por las stages: `scripts/ci/target-dir.sh`,
   `scripts/ci/check-bin-tracking.sh`, `scripts/ci/run-all-contracts.sh`,
   `scripts/known_failures.yaml` (encabezado).
7. Verificación de dependencias de binarios release en suites concretas:
   `crates/cognicode-mcp/tests/common/mod.rs` (`release_dir()` y
   `binary_path_for`, que prefiere `CARGO_BIN_EXE_*` y cae a `release`/`debug`) y
   `crates/cognicode-mcp/tests/prf_cli_04_two_process_uat.rs` (usa
   `release_dir().join("cognicode")` y `release_dir().join("cognicode-mcp")`
   de forma explícita).
8. Verificación de la naturaleza de gates concretos leyendo su cabecera:
   `crates/cognicode-core/tests/e91_w7_regression_budget.rs` (mide tiempo de
   reloj contra presupuesto de 30 s, `(claim de fuente)`), `e91_w8_stage_profile.rs`
   (per-stage contra 5 000 ms, `(claim de fuente)`),
   `core_gate_coverage_contract.rs`.
9. Comprobación del estado de la retirada de Actions: `.github/workflows/` está
   vacío y `pr-ci.yml` no existe, y `scripts/ci/test_no_actions_workflows.py`
   afirma el invariante "cero ficheros de workflow".

**Lo que NO se hizo**: no se ejecutó `pipelinek`, ni `cargo`, ni ningún test. No
se midió tiempo, memoria, disco ni red. No se editó nada dentro del worktree.

### Regla de clasificación usada

La clasificación es un juicio de política, no una medición. Se aplicó este
criterio, en este orden:

| Clase | Criterio aplicado |
|---|---|
| `MERGE_REQUIRED` | Un defecto aquí puede entrar en `main` con el diff de un PR, es determinista y su coste es bajo frente a lo que protege, o su presencia en la autoridad de merge está pinada por un contrato verificado. |
| `RELEASE_REQUIRED` | Su valor solo se realiza al producir o publicar un artefacto (payload, empaquetado, compatibilidad N-1, procedencia). No cambia por el diff de un PR normal. |
| `CERTIFICATION_REQUIRED` | Es evidencia de certificación o campaña: agresiva, costosa o dependiente del entorno (campañas adversariales, presupuestos de rendimiento, sandbox, scorecards). |
| `ADVISORY` | Informativa. Su fallo no debería bloquear, pero hoy, si no termina en `|| echo 'ADVISORY: ...'`, PipelineK no tiene `continue-on-error` y sí lo hace (ver §3.1). |

Cuando la intención no está declarada en el repo, se dice en la justificación y
se registra en §3 como desconocido. La columna **Decisión B1** no propone
movimientos ejecutados: B1 no ejecuta nada. Los valores son `SIN CAMBIO` o
`SIN CAMBIO, mover exige decisión explícita`.

## 2. Hechos medidos: comandos por stage

73 filas, en orden de fichero. "Padre" en la columna de ruta significa stage
agregadora sin `sh(...)` propio.

| # | Stage (padre/hija) | Línea | Comando o capacidad (condensado) | Clase | Justificación | Dependencias | Decisión B1 |
|---|---|---|---|---|---|---|---|
| 1 | `check` (padre) | 113 | agregador, sin `sh()` | `MERGE_REQUIRED` | Agrupa precondiciones, higiene y la suite de contratos. Si falla cualquiera, el merge no debe pasar. | ninguna (agregador) | SIN CAMBIO |
| 2 | `check/toolchain` | 114 | `cargo --version && rustc --version` | `MERGE_REQUIRED` | Precondición de entorno: sin toolchain no corre ningún paso posterior. Es el fallo rápido. | ninguna | SIN CAMBIO |
| 3 | `check/repo-contract` | 118 | `test -f Cargo.toml`, `test -f $authority`, `test -d scripts/ci`, `echo` conteo de crates | `MERGE_REQUIRED` | El gate no puede evaluar nada si falta su propia autoridad o la suite de contratos. Es un contrato de forma del repo, barato. | ninguna | SIN CAMBIO |
| 4 | `check/fmt` | 137 | `cargo fmt --all -- --check` | `MERGE_REQUIRED` | Determinista, barato, sensible al diff. G0.1 detectó 104 ficheros de drift `(claim de fuente en ROADMAP)` y la disciplina es bloquear. | ninguna | SIN CAMBIO |
| 5 | `check/clippy` | 141 | `cargo clippy --workspace --all-targets -- -D warnings` | `MERGE_REQUIRED` | Paridad con la CI retirada y tolerancia cero. El comentario del fichero documenta que una allowlist de 42 errores quedó muerta y se restauró el aserto desnudo. | compila el workspace en perfil de test | SIN CAMBIO |
| 6 | `check/rustdoc-gate` | 150 | `cargo test -p cognicode-runtime --test m011_rustdoc_gate --quiet` | `MERGE_REQUIRED` | Un warning de rustdoc sobrevivió un commit, la rama y seis commits más porque nada lo ejecutaba `(claim de fuente)`. Coste de un target. | ninguna | SIN CAMBIO. Candidato a `ADVISORY` si el maintainer decide que la higiene de doc-links no bloquea un merge. Mover exige decisión |
| 7 | `check/contract-toolchain` | 176 | `python3 -m pip install --disable-pip-version-check PyYAML==6.0.3 jsonschema==4.25.1` | `MERGE_REQUIRED` | Precondición declarada de `contracts` y `skills-validate`. El propio fichero documenta que `jsonschema` ausente hacía que el schema check del catálogo pasara en verde y en silencio. | red (índice pip). No hay fichero de requirements: esta línea ES la declaración | SIN CAMBIO. Dependencia de red registrada en §3.2 |
| 8 | `check/contracts` | 186 | `bash scripts/ci/run-all-contracts.sh` | `MERGE_REQUIRED` | Único punto de entrada de la suite de contratos, descubierta por glob `scripts/ci/test_*.py`. Incluye reachability de artefactos, cuerpos de stage, supply-chain y selector. Es la red anti-narrowing del gate. | python3, PyYAML y jsonschema de la stage 7 | SIN CAMBIO |
| 9 | `check/skills-validate` | 202 | `python3 scripts/validate_skills.py` | `MERGE_REQUIRED` | Valida nombres de tool contra el catálogo publicado. El fichero documenta que estuvo correcto y sin invocador hasta 2026-10-02. | PyYAML de la stage 7 | SIN CAMBIO |
| 10 | `check/skills-bundles` | 206 | `bash scripts/verify-skills.sh` | `MERGE_REQUIRED` | Comprueba manifiesto por bundle. Mismo caso CP5 que la stage 9. | PyYAML (vía Python) | SIN CAMBIO |
| 11 | `supply-chain` (padre) | 220 | agregador, sin `sh()` | `MERGE_REQUIRED` | Verificado: `test_supply_chain_gate_contract.py` asigna los dos ejes `cargo deny` a `MERGE_AUTHORITY`. | ninguna (agregador) | SIN CAMBIO |
| 12 | `supply-chain/install-cargo-deny` | 221 | `cargo install cargo-deny --version 0.20.2 --locked` si no está presente | `MERGE_REQUIRED` | Prerrequisito de las stages 13 y 14. Sin él el pipeline aborta antes de evaluar los ejes. Versión fijada a propósito para que el gate no derive sin commit. | red (crates.io). `(claim de fuente: 1m43s de instalación)` no medido por mí | SIN CAMBIO. Es un instalador, no un gate: ver §4.1 |
| 13 | `supply-chain/advisories` | 234 | `cargo deny check advisories` | `MERGE_REQUIRED` | Contrato verificado: `test_the_supply_chain_gate_runs_in_the_merge_authority` exige esta invocación en `MERGE_AUTHORITY`. Es diff-independiente y de red, pero quitarlo rompe un contrato. | cargo-deny (stage 12), red (base RUSTSEC), `deny.toml` | SIN CAMBIO, mover exige decisión explícita (rompe contrato) |
| 14 | `supply-chain/licenses` | 238 | `cargo deny check licenses` | `MERGE_REQUIRED` | Mismo contrato verificado, eje `licenses`. | cargo-deny (stage 12), `deny.toml` | SIN CAMBIO, mover exige decisión explícita (rompe contrato) |
| 15 | `lsp-toolchain` (padre) | 267 | agregador, sin `sh()` | `MERGE_REQUIRED` | Prerrequisito de `test-pr/core-lib`, que sin el LSP se auto-salta un test. | ninguna (agregador) | SIN CAMBIO |
| 16 | `lsp-toolchain/install-rust-analyzer` | 268 | `rustup component add rust-analyzer` si no responde | `MERGE_REQUIRED` | Contrato verificado: `test_the_gate_installs_the_lsp_server_it_needs` exige que la autoridad de merge instale rust-analyzer. | rustup (posible red) | SIN CAMBIO, mover exige decisión explícita (rompe contrato) |
| 17 | `lsp-toolchain/rust-analyzer-answers` | 281 | `rust-analyzer --version` o `exit 1` | `MERGE_REQUIRED` | Distingue "instalado" de "utilizable". El shim de rustup existe y sale con 1 `(claim de fuente)`, de modo que un `command -v` daría la respuesta equivocada. | binario rust-analyzer (stage 16) | SIN CAMBIO |
| 18 | `build-binary` (padre) | 305 | agregador, sin `sh()` | `MERGE_REQUIRED` | Prerrequisito de las suites que lanzan binarios por ruta. | ninguna (agregador) | **REESCRITA**: construye y verifica los binarios de perfil debug que el merge ejecuta; los release salen a la lane de candidato (ADR-TH-005) |
| 19 | `build-binary/build-cognicode-mcp-release` | 308 | `cargo build --release -p cognicode-mcp` | `RELEASE_REQUIRED` | Su consumidor real es `prf_cli_04_two_process_uat`, que pide el binario por `release_dir()` (verificado). Las demás suites MCP resuelven por `CARGO_BIN_EXE_*`, que es perfil debug. | compila release; escribe en el target dir compartido | **SALE** a `release-candidate.pipeline.kts`. En merge entra `build-binary/build-cognicode-mcp-debug` |
| 20 | `build-binary/build-control-plane-release` | 312 | `cargo build --release --bin cognicode-control-plane` | `RELEASE_REQUIRED` | Ningún consumidor de merge ni de candidato pedía este artefacto en perfil release: solo lo verificaba la stage 23. La garantía que aportaba (el binario se enlaza y arranca) se conserva en debug. | compila release | **SALE**; la garantía pasa a `build-binary/verify-debug-binaries` con el binario debug |
| 21 | `build-binary/build-cognicode-cli-release` | 319 | `cargo build --release -p cognicode-cli` | `RELEASE_REQUIRED` | Lo consumen las suites que taran `cogh`, `cognicode` y `cognicode-release` desde el directorio release. | compila release | **SALE** a candidato, que ya construye `cogh` y `cognicode` por target. En merge entra `--bins` en debug |
| 22 | `build-binary/build-cognicode-release-bin` | 323 | `cargo build --release --bin cognicode` | `RELEASE_REQUIRED` | Subconjunto estricto de la stage 21: `--bin cognicode` ya lo produce `-p cognicode-cli`. Duplicado demostrable, no una obligación aparte. | igual que 21 | **SALE**, y su duplicación se elimina: el candidato construye `cognicode` en la stage `binaries-$target` |
| 23 | `build-binary/verify-release-binaries` | 333 | `scripts/ci/target-dir.sh`, aserto de existencia y bit ejecutable de 3 bins, `--version` y `--help` | `RELEASE_REQUIRED` | Detecta "compila pero no arranca" y pregunta a cargo dónde escribió en vez de hardcodear. | stages 19-22, `target-dir.sh`, cargo | **SUSTITUIDA por dos asertos, no eliminada**: `build-binary/verify-debug-binaries` en merge (mismo aserto sobre los binarios debug) y `candidate/binary-smoke-$target` en candidato, que ya existía y además sabe ejecutar un target extranjero |
| 24 | `selector` (padre) | 363 | agregador, sin `sh()` | `MERGE_REQUIRED` | El selector CR-08 lo consumen `test-fast` y la CI. | ninguna (agregador) | SIN CAMBIO |
| 25 | `selector/qw08-crate-selector` | 367 | `cargo test -p cognicode-cli --test qw08_crate_selector --quiet` | `MERGE_REQUIRED` | Pina el comportamiento del selector, que es lo que decide qué suites corren. Es la protección del propio gate. | compila `cognicode-cli` en perfil de test | SIN CAMBIO |
| 26 | `selector/select-suites-script` | 375 | `SELECT_PATHS=scripts/ci/run-all-contracts.sh bash scripts/ci/select-suites.sh`, imprime salida | `MERGE_REQUIRED` | Prueba que el script real del selector es consumible, no solo que su contrato pase. Debilidad honesta: solo asevera `exit 0`, no el contenido. | `scripts/ci/select-suites.sh` | SIN CAMBIO. Aserto débil: reforzar o degradar exige decisión |
| 27 | `test-pr` (padre) | 387 | agregador, sin `sh()` | `MERGE_REQUIRED` | Contiene las suites de comportamiento de core, CLI y MCP. | lsp-toolchain (16, 17) para no dar verdes vacíos | SIN CAMBIO |
| 28 | `test-pr/core-lib` | 389 | `cargo test -p cognicode-core --lib --quiet` | `MERGE_REQUIRED` | Suite primaria de comportamiento del core. Un test de jerarquía dentro de ella se auto-salta si falta rust-analyzer, de ahí la stage 16. | rust-analyzer (stage 16/17) | SIN CAMBIO |
| 29 | `test-pr/core-adversarial-campaign` | 393 | `cargo test -p cognicode-core --test prf_sec_07_adversarial_campaign --quiet` | `CERTIFICATION_REQUIRED` | Es evidencia de la campaña adversarial PRF-SEC-07, no una regresión acotada al diff de un PR. | test target dedicado | SIN CAMBIO, mover exige decisión explícita |
| 30 | `test-pr/core-prf-mcp-05` | 397 | `cargo test -p cognicode-core --lib prf_mcp_05 --quiet` | `MERGE_REQUIRED` | Verificado en `rmcp_adapter.rs`: pina autoridad declarada por tool, exclusión de tools no read en read-only y el helper de mutación. Es superficie pública. | core lib | SIN CAMBIO |
| 31 | `test-pr/core-adversarial-e2e` | 401 | `cargo test -p cognicode-core --test prf_h06_adversarial_e2e --quiet` | `CERTIFICATION_REQUIRED` | E2E adversarial PRF-H06. Naturaleza de campaña, no de gate por diff. Su precondición (binario `cognicode-mcp`) se resolvía por ruta y se auto-saltaba en silencio si faltaba (`SKIP_NOT_APPLICABLE`). | test target dedicado; necesita el binario `cognicode-mcp` | **SIN CAMBIO en selección; PRECONDICIÓN CORREGIDA**: el merge construye el binario debug y lo entrega por `COGNICODE_MCP_BIN`; la suite falla en voz alta si falta |
| 32 | `test-pr/core-unrestricted-evidence-kernel` | 409 | `cargo test -p cognicode-core --features evidence-kernel --quiet`, redirigido a `$logDir/...log`, `tail -40`, `exit rc` | `MERGE_REQUIRED` | Sin la feature, cinco suites con `#![cfg(feature = "evidence-kernel")]` compilan a binario vacío y reportan 0/0/exit 0. El contrato `core_gate_coverage_contract` pina el comando. | feature `evidence-kernel`; escribe en `$logDir`, fuera del repo | SIN CAMBIO |
| 33 | `test-pr/core-a014-product-asset-resolution` | 439 | `cargo test -p cognicode-core --test a014_product_asset_resolution --quiet` | `MERGE_REQUIRED` | Pina la resolución de assets de producto de A-014, superficie que el fichero atribuye a `pr-ci.yml:481`. | test target dedicado | SIN CAMBIO |
| 34 | `test-pr/cognicode-ladybug` | 444 | `cargo test -p cognicode-ladybug --lib --quiet` | `MERGE_REQUIRED` | Suite lib del crate de almacenamiento durable. Es el sustrato del kernel de evidencia. | ninguna | SIN CAMBIO |
| 35 | `test-pr/bin-tracking` | 453 | `bash scripts/ci/check-bin-tracking.sh` | `MERGE_REQUIRED` | Guard QW-03: todo `[[bin]]` declarado tiene su fuente tracked por git. Barato, determinista y con historia de fallo real. | git, `cargo metadata` vía el script | SIN CAMBIO |
| 36 | `test-pr/cr07-metrics-exposition` | 476 | `cargo test -p cognicode-mcp --test cr07_metrics_exposition_contract --quiet` | `MERGE_REQUIRED` | `/metrics` es la razón del upgrade de OpenTelemetry y de la desaparición de RUSTSEC-2024-0437 `(claim de fuente)`. Regresión de superficie pública. | test target dedicado | SIN CAMBIO |
| 37 | `test-pr/cli-qw03-bin-tracking` | 481 | `cargo test -p cognicode-cli --test qw03_bin_tracking_guard --quiet` | `MERGE_REQUIRED` | Contrato del guard de la stage 35. Doble red deliberada. | test target dedicado | SIN CAMBIO |
| 38 | `test-pr/cli-qw04-preflight` | 485 | `cargo test -p cognicode-cli --test qw04_preflight_contract --quiet` | `MERGE_REQUIRED` | Contrato de preflight del CLI. Se corresponde con `release-candidate/preflight/qw04-preflight-contract`. | test target dedicado | SIN CAMBIO |
| 39 | `test-pr/cli-a013-lifecycle-gate-contract` | 489 | `cargo test -p cognicode-cli --test a013_lifecycle_gate_contract --quiet` | `MERGE_REQUIRED` | Gate de lifecycle de A-013, superficie de ciclo de vida del producto. | test target dedicado | SIN CAMBIO |
| 40 | `test-pr/cli-a014-capabilities-json` | 493 | `cargo test -p cognicode-cli --test a014_capabilities_json --quiet` | `MERGE_REQUIRED` | Pina la superficie `--json` de capabilities publicada. Verificado que este fichero toca rutas release. | test target dedicado; rutas release (stages 19-22) en su resolución | SIN CAMBIO |
| 41 | `test-pr/cli-a015-licenses-gate` | 497 | `cargo test -p cognicode-cli --test a015_licenses_gate --quiet` | `MERGE_REQUIRED` | Gate de licencias del CLI. Nota: el gate llama A-015 a licencias y el action register llama A-015 a onboarding `(claim de fuente en ROADMAP)`. | test target dedicado | SIN CAMBIO |
| 42 | `test-pr/cli-a015-onboarding-gate` | 501 | `cargo test -p cognicode-cli --test a015_onboarding_gate --quiet` | `MERGE_REQUIRED` | `cogh setup`/doctor existían y ningún gate los ejecutaba `(claim de fuente en ROADMAP)`. Verificado que el fichero toca rutas release. | test target dedicado; rutas release | SIN CAMBIO |
| 43 | `test-pr/cli-action-register-identity-contract` | 507 | `cargo test -p cognicode-cli --test action_register_identity_contract --quiet` | `MERGE_REQUIRED` | Mantiene visible la colisión de identidad de los ciclos gemelos A-014 (N+63.2) `(claim de fuente)`. | test target dedicado | SIN CAMBIO |
| 44 | `cli-unit-tests` + `cli-integration-suites` | 546 | antes: `cargo test -p cognicode-cli --features ladybug --quiet`; ahora: `--lib --bins --doc` por un lado y una invocación con `--test` por cada suite del crate que no es de perfil release por otro | `MERGE_REQUIRED` | Compila todas las `tests/*.rs` del crate y activa la feature. Sin ella `evidence_cli_mcp_equivalence` compila vacío. Pinado por `cli_gate_coverage_contract`. | feature `ladybug`; escribe en `$logDir` | **REESCRITA**: la selección pasa de comodín a enumeración explícita, porque una suite que exige binarios release ya no puede correr aquí. `--lib --bins --doc` se separa para no perder los tests de unidad al enumerar. El workaround documentado es deuda: ver §3.4 |
| 45 | `test-pr/cli-gate-coverage-contract` | 562 | `cargo test -p cognicode-cli --test cli_gate_coverage_contract --quiet` | `MERGE_REQUIRED` | Anti-narrowing del gate CLI: falla si el paso irrestricto falta o se estrecha. | test target dedicado | SIN CAMBIO |
| 46 | `test-pr/mcp-a009-agent-safe-profile` | 572 | `cargo test -p cognicode-mcp --test a009_agent_safe_profile --quiet` | `MERGE_REQUIRED` | Pina el perfil agent-safe publicado (CP2 A-009). Superficie pública del producto. | `common` del crate: prefiere `CARGO_BIN_EXE_*`, cae a release/debug | SIN CAMBIO |
| 47 | `test-pr/mcp-a010-tool-authority-audit` | 580 | `cargo test -p cognicode-mcp --test a010_tool_authority_audit --quiet` | `MERGE_REQUIRED` | Auditoría de autoridad por tool (CP2 A-010). | igual que 46 | SIN CAMBIO |
| 48 | `test-pr/mcp-a012-structured-output` | 588 | `cargo test -p cognicode-mcp --test a012_structured_output --quiet` | `MERGE_REQUIRED` | Pina el output estructurado (CP2 A-012), contrato consumible por agentes. | igual que 46 | SIN CAMBIO |
| 49 | `test-pr/mcp-a013-lifecycle-uat` | 596 | `cargo test -p cognicode-mcp --test a013_lifecycle_uat --quiet` | `MERGE_REQUIRED` | Contrato end-to-end del binario MCP: arranque, workspace inexistente, cierre, hung-process como release blocker. | igual que 46 | SIN CAMBIO |
| 50 | `test-pr/mcp-prf-sec-02-read-only-uat` | 604 | `cargo test -p cognicode-mcp --test prf_sec_02_read_only_uat --quiet` | `MERGE_REQUIRED` | Diferenciación de capacidades read/write sobre binario real. Es un control de seguridad, no una campaña. | igual que 46 | SIN CAMBIO |
| 51 | `test-pr/mcp-a016-tools-runtime-consistency` | 612 | `cargo test -p cognicode-mcp --test a016_tools_runtime_consistency --quiet` | `MERGE_REQUIRED` | Consistencia entre catálogo publicado y runtime MCP. | igual que 46 | SIN CAMBIO |
| 52 | `test-pr/mcp-prf-ana-02-uat` | 620 | `cargo test -p cognicode-mcp --test prf_ana_02_uat --quiet` | `MERGE_REQUIRED` | Honestidad de estado: un fichero ilegible debe aparecer en `skipped` con `Partial`, nunca un `complete` silencioso. | igual que 46 | SIN CAMBIO |
| 53 | `test-pr/mcp-prf-ana-05-uat` | 628 | `cargo test -p cognicode-mcp --test prf_ana_05_uat --quiet` | `MERGE_REQUIRED` | Determinismo de `build_graph` repetido sobre el mismo workspace. | igual que 46 | SIN CAMBIO |
| 54 | `test-pr/mcp-prf-ana-07-uat` | 636 | `cargo test -p cognicode-mcp --test prf_ana_07_uat --quiet` | `MERGE_REQUIRED` | Reglas de visibilidad y resolución de colisiones en el grafo publicado. | igual que 46 | SIN CAMBIO |
| 55 | `test-pr/mcp-prf-ana-08-uat` | 644 | `cargo test -p cognicode-mcp --test prf_ana_08_uat --quiet` | `MERGE_REQUIRED` | Presupuesto de categoría de búsqueda y salida acotada. Es funcional (bounded output), no un presupuesto de rendimiento de máquina. | igual que 46 | SIN CAMBIO |
| 56 | `mcp-prf-cli-04-two-process-uat` | 652 | antes: `cargo test -p cognicode-mcp --test prf_cli_04_two_process_uat --quiet` en merge; ahora en `release-candidate/release-contracts` | `RELEASE_REQUIRED` | Equivalencia CLI frente a MCP en dos procesos reales. Es la prueba de consumidor del contrato E0, y ejecuta `cognicode` y `cognicode-mcp` **de perfil release** (`release_dir()` verificado). | necesita `cognicode` y `cognicode-mcp` release; en candidato se le apuntan con `COGNICODE_RELEASE_BIN_DIR` | **SALE** a `release-candidate.pipeline.kts`, donde corre contra los bytes que esa lane empaqueta |
| 57 | `test-pr/mcp-prf-f4-w3-corrupt-cache-recovery` | 660 | `cargo test -p cognicode-mcp --test prf_f4_w3_corrupt_cache_recovery --quiet` | `MERGE_REQUIRED` | Una caché corrupta nunca debe presentarse como evidencia válida. Es corrección, no campaña. | igual que 46 | SIN CAMBIO |
| 58 | `test-pr/mcp-prf-f5-w3-signal-cancel` | 668 | `cargo test -p cognicode-mcp --test prf_f5_w3_signal_cancel --quiet` | `MERGE_REQUIRED` | SIGKILL al servidor MCP no debe dejar estado que bloquee la sesión siguiente. | igual que 46 | SIN CAMBIO |
| 59 | `test-pr/mcp-prf-mcp-02-uat` | 676 | `cargo test -p cognicode-mcp --test prf_mcp_02_uat --quiet` | `MERGE_REQUIRED` | stdout solo con JSON-RPC, logs solo por stderr, cierre limpio al cerrar stdin. Es una regla dura del contrato del core. | igual que 46 | SIN CAMBIO |
| 60 | `test-pr/mcp-prf-sec-01-uat` | 684 | `cargo test -p cognicode-mcp --test prf_sec_01_uat --quiet` | `MERGE_REQUIRED` | Allowlist de directorios en `build_graph`: traversal, escapes absolutos y symlinks rechazados. Seguridad. | igual que 46 | SIN CAMBIO |
| 61 | `test-pr/mcp-prf-sec-03-telemetry-optin-uat` | 692 | `cargo test -p cognicode-mcp --test prf_sec_03_telemetry_optin_uat --quiet` | `MERGE_REQUIRED` | La telemetría debe ser opt-in y por defecto no debe existir intento de export OTLP. | igual que 46 | SIN CAMBIO |
| 62 | `test-pr/mcp-prf-sec-05-shutdown-recovery-uat` | 700 | `cargo test -p cognicode-mcp --test prf_sec_05_shutdown_recovery_uat --quiet` | `MERGE_REQUIRED` | Shutdown y crash sin corrupción del snapshot durable. | igual que 46 | SIN CAMBIO |
| 63 | `test-pr/mcp-prf-state-01-data-catalog-uat` | 708 | `cargo test -p cognicode-mcp --test prf_state_01_data_catalog_uat --quiet` | `MERGE_REQUIRED` | Catálogo canónico/derivado/transitorio ejercido en el boundary de proceso. | igual que 46 | SIN CAMBIO |
| 64 | `test-pr/mcp-prf-state-02-uat` | 716 | `cargo test -p cognicode-mcp --test prf_state_02_uat --quiet` | `MERGE_REQUIRED` | Namespace por `canonical_root + analysis_config_digest`: dos workspaces con símbolos idénticos no se pisan. | igual que 46 | SIN CAMBIO |
| 65 | `test-pr/mcp-prf-state-03-04-uat` | 724 | `cargo test -p cognicode-mcp --test prf_state_03_04_uat --quiet` | `MERGE_REQUIRED` | Semántica del snapshot durable y rechazo de snapshot corrupto. | igual que 46 | SIN CAMBIO |
| 66 | `test-pr/mcp-prf-state-03-concurrent-uat` | 732 | `cargo test -p cognicode-mcp --test prf_state_03_concurrent_uat --quiet` | `MERGE_REQUIRED` | Dos procesos MCP concurrentes sobre el mismo workspace no pierden datos. | igual que 46 | SIN CAMBIO |
| 67 | `test-pr/mcp-prf-state-04-isolation-uat` | 740 | `cargo test -p cognicode-mcp --test prf_state_04_isolation_uat --quiet` | `MERGE_REQUIRED` | Reproducción mínima de aislamiento entre dos procesos secuenciales. | igual que 46 | SIN CAMBIO |
| 68 | `merge-gate` (padre) | 747 | agregador, sin `sh()` | `MERGE_REQUIRED` | Cierra la lane con lint de drift, compatibilidad, contratos de cobertura y presupuestos. | stages 1-67 | SIN CAMBIO |
| 69 | `merge-gate/capabilities-drift-lint` | 749 | `python3 sandbox/scripts/capabilities_drift_lint.py --strict` | `MERGE_REQUIRED` | El fichero lo declara "hard gate". Detecta drift entre capacidades publicadas y reales, que es exactamente lo que puede introducir un PR. | python3, script de sandbox | SIN CAMBIO |
| 70 | `merge-gate/compat-matrix-0-97-x` | 756 | condicional a `sandbox/.compat/0.97.3/cognicode-mcp`: `cargo test -p cognicode-mcp --test find_usages_compat_0_97 -- --skip compat_forward_0973_server_handles_modern_request`. Si falta el fixture, imprime `SKIP` | `RELEASE_REQUIRED` | La compatibilidad N-1 con 0.97.x es una promesa de release (ADR-PRF-008, política R1), no una propiedad del diff de un PR. En un checkout sin el fixture su efecto sobre el merge es nulo (imprime SKIP). | fixture `sandbox/.compat/0.97.3/cognicode-mcp` (ausente por defecto), cargo | SIN CAMBIO, mover exige decisión explícita. Es el único sitio donde corre hoy |
| 71 | `merge-gate/core-gate-coverage-contract` | 772 | `cargo test -p cognicode-core --features evidence-kernel --test core_gate_coverage_contract --quiet` | `MERGE_REQUIRED` | Anti-narrowing del gate de core. Verificado que el contrato lee los ficheros de pipeline (no `pr-ci.yml`), así que también impide que la pipeline Kotlin estreche el gate. | feature `evidence-kernel` | SIN CAMBIO |
| 72 | `merge-gate/e91-w7-regression-budget` | 777 | `cargo test -p cognicode-core --test e91_w7_regression_budget --quiet` | `CERTIFICATION_REQUIRED` | Verificado en la cabecera del test: mide tiempo de reloj de `GraphInsightsService::analyze` contra un presupuesto (`30 s` con 2x de margen, `claim de fuente`). Un gate de merge basado en reloj de pared es sensible a la contención de la máquina. | core en perfil de test. Fixture determinista de 1000 nodos | SIN CAMBIO, mover exige decisión explícita |
| 73 | `merge-gate/e91-w8-stage-profile` | 782 | `cargo test -p cognicode-core --test e91_w8_stage_profile --quiet` | `CERTIFICATION_REQUIRED` | Perfil por stage con cota de `5 000 ms` por stage (`claim de fuente`). Misma naturaleza de presupuesto de rendimiento que la stage 72. | core en perfil de test, mismo fixture Tier-2 | SIN CAMBIO, mover exige decisión explícita |

### Recuento por clase

| Clase | Stages | Desglose |
|---|---|---|
| `MERGE_REQUIRED` | 63 | 7 padres + 56 hijas |
| `RELEASE_REQUIRED` | 6 | 6 hijas (stages 19-23 y `compat-matrix-0-97-x`) |
| `CERTIFICATION_REQUIRED` | 4 | 4 hijas (`core-adversarial-campaign`, `core-adversarial-e2e`, `e91-w7-regression-budget`, `e91-w8-stage-profile`) |
| `ADVISORY` | 0 | ninguna |
| Total | 73 | 7 padres + 66 hijas |

Comprobación aritmética: 63 + 6 + 4 + 0 = 73, y 56 + 6 + 4 = 66 hijas. El
recuento es el del inventario sobre el fichero auditado; describe la clasificación
razonada, no el número de stages que B1 movió.

El cero en `ADVISORY` es un hallazgo en sí. En
`merge-gate.pipeline.kts` no hay ninguna stage declarada como no bloqueante, a
diferencia de `integration.pipeline.kts` y `certification.pipeline.kts`, que sí
usan el patrón `|| echo 'ADVISORY: ...'`.

## 3. Riesgos y desconocidos

### 3.1 Cero stages `ADVISORY` en la autoridad de merge

Toda hija que no termina en `|| echo` aborta la pipeline al fallar (el propio
fichero documenta que PipelineK no tiene `continue-on-error` y que ese es el
único mecanismo, `claim de fuente`). Consecuencia: nada aquí es degradable sin
decisión explícita. Un fallo transitorio de red, de índice de crates o de
reloj de pared bloquea un merge igual que una regresión real.

### 3.2 Dependencias de red dentro de la lane de merge

Tres puntos piden red y no son el producto: `pip install`
(`contract-toolchain`, línea 176), `cargo install cargo-deny` (línea 221) y la
consulta de advisories contra la base RUSTSEC (línea 234). El repo declara el
core "sin red" como principio, y aquí la red entra en el gate. No medí si estos
pasos son tolerantes a fallo de red ni cuánto tardan.

### 3.3 `prf_cli_04_two_process_uat` anclaba la dependencia de binarios release

Es la única suite en merge donde se verificó `release_dir()` explícito para dos
binarios (`cognicode` y `cognicode-mcp`). B1 resolvió la pregunta: si el único
consumidor verificable de los artefactos release es un conjunto de suites que
taran o ejecutan bytes release, esas suites y su construcción se van juntas a la
lane que produce esos bytes. En merge queda la dependencia de los binarios
debug, construida por la propia lane. Las 22 suites MCP que usan `common::`
resuelven por `CARGO_BIN_EXE_*` y no exigen release: su dependencia es un
fallback.

### 3.4 Workaround documentado en la antigua `cli-unrestricted-ladybug` (hoy `cli-integration-suites`)

El comentario del fichero dice que la stage murió 4 de 4 sin redirección de
salida y pasó 64 de 64 con ella, sobre la misma máquina, el mismo árbol y el
mismo comando, y lo declara explícitamente "workaround de un defecto del
orquestador, no un diagnóstico" (`claim de fuente`, no reproducido por mí). El
mismo patrón se aplicó a `core-unrestricted-evidence-kernel` (línea 409) por un
drenaje de transcript incompleto a los 10 s (`claim de fuente`). Es deuda
conocida y no medida en esta sesión.

### 3.5 Comentarios desactualizados en la propia pipeline

- Línea 769-771: dice que `core-gate-coverage-contract` "reads pr-ci.yml". El
  fichero `.github/workflows/` está vacío y `pr-ci.yml` no existe (verificado).
  El contrato lee los ficheros de pipeline (verificado en
  `core_gate_coverage_contract.rs`, líneas 51-52 y 471-481). El comentario es
  historia, no descripción.
- El mapa `// pr-ci.yml:NNN` que abre el fichero y reaparece en cada bloque es
  procedencia de un orquestador retirado. No es un defecto, pero hace que el
  lector tenga que decidir cada vez si la referencia sigue vigente.
- `test_no_actions_workflows.py` afirma el invariante "cero ficheros de
  workflow". Cualquier referencia textual futura a `pr-ci.yml` que no sea
  histórica choca con ese invariante.

### 3.6 Gates de reloj de pared

Las stages 72 y 73 asertan tiempo de reloj (30 s agregado y 5 000 ms por stage,
`claim de fuente`) sobre una máquina compartida por varios checkouts con el
`target dir` compartido, según el propio `AGENTS.md`. No medí su varianza ni si
hoy pasan. Un gate de merge sensible a la carga de la máquina es una fuente
plausible de rojo no atribuible al diff.

### 3.7 Cobertura que no pude comprobar

- No ejecuté la pipeline, así que no sé qué stages pasan hoy ni cuáles fallan.
- No verifiqué que las 7 stages padre y 66 hijas correspondan exactamente a lo
  que declara `scripts/ci/test_pipeline_stage_bodies.py` ni a los conteos de los
  contratos de cobertura. Solo conté las declaraciones en el fichero.
- No leí `release-candidate.pipeline.kts` ni `release.pipeline.kts` completos:
  solo sus listas de stages. Una duplicación de gates entre merge y esas lanes
  es posible y no la descarté.

## 4. Recomendaciones NO ejecutadas en B1

Ninguna de estas cambia el pipeline. Cada una toca un contrato, un invariante o
una política, así que ninguna es un cambio de B1 sin decisión.

1. **Sacar `cargo deny` del merge (stages 13 y 14) a un carril programado o de
   release.** Motivo para no ejecutarlo: `test_supply_chain_gate_contract.py`
   pina su presencia en `MERGE_AUTHORITY` (verificado). Es un cambio de
   contrato de gobernanza, no de comando.
2. **Degradar `contract-toolchain`, `install-cargo-deny` e
   `install-rust-analyzer` de gate a preparación tolerante a fallo.** Motivo:
   hoy un fallo de red aborta la pipeline, y `install-rust-analyzer` está pinado
   por el mismo contrato que el punto 1. Requiere decidir dónde vive la
   tolerancia, porque PipelineK no tiene `continue-on-error`.
3. **Declarar `ADVISORY` explícito donde hoy no lo hay.** Motivo: no existe
   ninguna stage no bloqueante en el merge, y el mecanismo disponible
   (`|| echo 'ADVISORY: ...'`) convierte el gate en informativo para siempre.
   Es política, y además rompería los contratos que exigen la invocación
   literal.
4. **Mover las cuatro stages `CERTIFICATION_REQUIRED` a
   `certification.pipeline.kts`.** Motivo: dos son campañas adversariales PRF y
   dos son presupuestos de rendimiento. Moverlas reduce el merge pero quita
   cobertura, y el repo exige reemplazo o decisión explícita antes de estrechar
   un gate.
5. **Mover `compat-matrix-0-97-x` a la lane de release-candidate.** Motivo: es
   una promesa N-1 y hoy imprime `SKIP` sin fixture. Moverla la deja sin
   ejecución alguna si la lane de release tampoco trae el fixture, así que la
   decisión incluye dónde se materializa el fixture.
6. **Reforzar `selector/select-suites-script` (línea 375) para que asevere
   contenido, no solo `exit 0`.** Motivo: cambiar un aserto débil por uno fuerte
   puede hacer rojo un gate que hoy pasa en verde vacío. Exige decidir el
   aserto.
7. **Reescribir los comentarios de procedencia `pr-ci.yml:NNN` y el de la línea
   769.** Motivo: es documentación del fichero de autoridad y su corrección
   puede afectar a contratos que escanean texto (por ejemplo, el escaneo de
   anclas exige que las suites no estén citadas en workflows o scripts).
8. **Abrir trabajo para el defecto del orquestador detrás del workaround de
   `cli-integration-suites` (antes `cli-unrestricted-ladybug`).** Motivo: el workaround vive en la autoridad de
   merge y se apoya en una causa no diagnosticada. Sustituirlo por un diagnóstico
   o por una versión de PipelineK reparada exige verificación externa al repo.
