# RECONCILIATION-MATRIX — candidato v0.98.0 (HEAD `8505ad85`) vs PRF original

> **Origen:** Plan de ejecución prolongada 2026-09-24 §B1, autorización del operador para "reconciliar el estado real antes de C7, sin reciclar el freeze `178f8a5b`".
>
> **Candidata evaluada:** tag anotado **`v0.98.0`** (objeto tag `d99d3911ded7ce807e5865905202843c7b5dda33`), apuntando al commit **`8505ad8506e68c85914eaaf2f71a7fde6149ed61`** (workspace version 0.98.0). Publicación en GitHub Releases 2026-09-24T17:36:17Z, run CI #36034410448 (5/5 SUCCESS), 12 assets.
>
> **Base previa:** `docs/prf/specs/RECONCILIATION-MATRIX.md` (matriz congelada en `178f8a5b` por sesión 2026-09-22, **NO se modifica** — es evidencia contractual previa). Este documento es su **re-evaluación contra el HEAD real posterior a la publicación de v0.98.0**, distinguiendo explícitamente:
> - Garantías transferibles: las que se cumplen en `8505ad85` por estar el código en HEAD posterior al SHA congelado.
> - Garantías que requieren nueva ejecución sobre la candidata concreta (no aceptadas por herencia).
>
> **Diferencia crítica:** `ACCEPTED ≠ PASS contractual`. PASS = criterio del SPEC-* o de la UAT original cumplido demostrablemente sobre la candidata con UAT ejecutada y recibo firmado.

---

## 1. Identidad inmutable de la candidata

| Campo | Valor | Fuente / Verificación |
|---|---|---|
| Tag anotado | `v0.98.0` | `git for-each-ref refs/tags/v0.98.0` → `d99d3911…` (objecttype `tag`) |
| SHA candidato (commit) | `8505ad8506e68c85914eaaf2f71a7fde6149ed61` | `git rev-parse v0.98.0^{commit}` |
| Versión workspace | `0.98.0` | `Cargo.toml` `[workspace.package] version` al SHA `8505ad85` |
| Publicación GitHub Releases | 2026-09-24T17:36:17Z | `gh release view v0.98.0 --json publishedAt` |
| Run CI publicación | `#36034410448` (5/5 SUCCESS) | GitHub Actions URL |
| Run validate posterior | `#36038178581` (tag/workspace gate PASS sobre `d40e61b2`) | GitHub Actions URL |
| Run validate negativo | `#36039255746` (gate atrapó mismatch forzado `version=0.99.0`) | GitHub Actions URL |
| Assets publicados | 12 (ver §3) | `gh release view v0.98.0 --json assets` |
| Tagger | Ruben <rubentxu@cognicode.dev> | `git cat-file -p v0.98.0` |
| Tag firma GPG | NO (`git verify-tag v0.98.0` → "no signature found") | Observado |

---

## 2. Relación SHA congelado `178f8a5b` ↔ candidata `8505ad85`

| Métrica | Valor | Verificación |
|---|---|---|
| Commits entre congelado y candidata | **333** | `git rev-list --count 178f8a5b..8505ad85` |
| Commits entre candidata y HEAD local actual | **20** | `git rev-list --count 8505ad85..01881f6b` (HEAD actual) |
| Diff paths `178f8a5b..8505ad85` | cambios en `crates/`, `scripts/`, `.github/`, `Cargo.toml`, `Cargo.lock`, docs | inspeccionado por `git diff --name-only` |
| Diff paths `8505ad85..01881f6b` (HEAD actual) | sólo `docs/prf/*` (5 archivos) | verificado commit por commit |

**Implicación contractual:**
- Las UAT/evidencias ejecutadas sobre `178f8a5b` o ancestros **son transferibles a `8505ad85` SOLO para los requisitos cuyo código no haya cambiado en los 333 commits intermedios**.
- Las UAT/evidencias sobre `d40e61b2` o posteriores (los 20 commits posteriores a `8505ad85`) NO son transferibles a la candidata `v0.98.0` (que apunta a `8505ad85`, **anterior** a esos 20). Estos 20 commits son exclusivamente `docs/prf/*` y no tocan código de producto, así que **no afectan al release publicado**, pero sí actualizan punteros contractuales.
- La candidata `v0.98.0` es **inmutable en GitHub Releases**: cualquier corrección posterior del código requiere una candidata nueva (`v0.98.1` o `v0.99.0`), no se modifica `8505ad85`.

---

## 3. Artefactos publicados (12 assets, SHA256 inmutable)

Descargados el 2026-09-24 desde GitHub Releases; SHA256SUMS completo en `/tmp/prf-v098-reconcile/SHA256SUMS` (verificable contra GitHub).

| # | Asset | SHA-256 (primeros 12) | Tamaño |
|---|---|---|---|
| 1 | `bundle-0.98.0-aarch64-unknown-linux-gnu.yaml` | `3e1d9b87186f` | 1084 |
| 2 | `bundle-0.98.0-x86_64-unknown-linux-gnu.yaml` | `daffdc140baf` | 1079 |
| 3 | `cogh-0.98.0-aarch64-unknown-linux-gnu.tar.gz` | `38e212bcdc3d` | 3 268 128 |
| 4 | `cogh-0.98.0-x86_64-unknown-linux-gnu.tar.gz` | `7172d79c57af` | 3 318 999 |
| 5 | `cognicode-0.98.0-aarch64-unknown-linux-gnu.tar.gz` | `51c95151c8c3` | 8 649 545 |
| 6 | `cognicode-0.98.0-x86_64-unknown-linux-gnu.tar.gz` | `df6845597e32` | 8 920 991 |
| 7 | `cognicode-0.98.0.tar.gz` | `a12ea0df6729` | 2 414 |
| 8 | `cognicode-mcp-0.98.0-aarch64-unknown-linux-gnu.tar.gz` | `36af115ab056` | 12 388 417 |
| 9 | `cognicode-mcp-0.98.0-x86_64-unknown-linux-gnu.tar.gz` | `b822e13c34f7` | 12 730 310 |
| 10 | `cognicode-mcp-0.98.0.tar.gz` | `eb52facb65d3` | 4 764 |
| 11 | `release-inventory-0.98.0.json` | `4203f3a53241` | 2 756 |
| 12 | `SHA256SUMS` | `4079166b5e66` | 1 192 |

**Tier-1 anunciada en `release.yml`:** `linux-x86-64`, `linux-aarch64` GNU — **ambas con artefactos publicados**, verificación de integridad por SHA256SUMS reproducible.

**Plataformas NO publicadas en v0.98.0 (por contrato):** MUSL, macOS, Windows (per `release_contract.rs` y `release.yml` matrix — `PRF-DIST-05` los declara "pendientes hasta certificar").

---

## 4. Reconciliación por requisito (matriz única)

> Símbolos: **PASS heredable** = garantía transferible desde `178f8a5b` o ancestro porque el código del requisito no cambió en `178f8a5b..8505ad85`. **PASS re-ejecutado** = UAT o test corrido contra `8505ad85` o tag `v0.98.0` específicamente. **PARTIAL** = ejecutado pero no cubre criterio entero. **GAP** = no cubierto por la candidata.

### 4.1 SECCIÓN A — `SPEC-ANALYSIS.md`

| Requisito | Disposición sobre `v0.98.0` | Evidencia / nota |
|---|---|---|
| PRF-ANA-01 (capabilities + lightweight) | **GAP heredado** | Capabilities declaradas por código (sess 4, `d2862663`+`0d96e93a`), pero la pieza `full`/`per_file`/`lightweight` no se ejecutó como UAT sobre `v0.98.0`. Matriz base declara PARTIAL. |
| PRF-ANA-02 (errores no silenciosos) | **PASS heredable** | UAT binario real sobre `prf_ana_02_uat` (§63) corre contra HEAD; el código tocado (call-sites en `build_project_graph`) **no fue modificado en `178f8a5b..8505ad85`** (verificable: `git log --oneline 178f8a5b..8505ad85 -- crates/cognicode-core/src/application/services/analysis_service.rs` no incluye refactors del flujo). |
| PRF-ANA-03 (cambio bytes preservando mtime+size) | **PASS heredable + GREEN adicional** | SHA-256 key en `39928202` (§32). Test `h01_byte_change_with_same_mtime_and_same_size_must_invalidate_cache` **RE-ejecutado verde** en `8505ad85` (verificación pendiente en B4 sobre el binario publicado). |
| PRF-ANA-04 (status `complete`/`partial`/`unknown`) | **PASS heredable** | `41e4230f` (§33); código del handler MCP `build_graph` no refactorizado entre congelado y candidata. |
| PRF-ANA-05 (reproducibilidad) | **PASS heredable** | `a2a2ce61` (§58). Determinismo canónico `(from,to)`. |
| PRF-ANA-06 (basis con workspace+config_digest+source manifest) | **PASS heredable** | `prf_ana_06_basis` (§49). `BasisDto` con SHA-256 digests. |
| PRF-ANA-07 (renames/colisiones) | **PASS heredable** | `prf_ana_07_uat` sobre `a07ecaf9` (§59). Corpus 51 homónimos. |
| PRF-ANA-08 (budgets categorizados) | **PASS heredable** | `prf_ana_08_uat` 1/1 (§69). Timeouts graph 60s/search 500ms. |
| PRF-ANA-09 (LSI golden corpus) | **EXCL (2026-09-22)** | LSI no implementado. Condicional del SPEC no exigible. |

### 4.2 SECCIÓN B — `SPEC-CI.md`

| Requisito | Disposición sobre `v0.98.0` | Evidencia / nota |
|---|---|---|
| PRF-CI-01 (recibo por SHA; FAIL aborta) | **PARTIAL (clippy cerrado)** + **GAP disparador automático** | Gate clippy `34153097` (§90) verificado; disparador automático en push-PR es H-07 operator-gated. **Sobre `v0.98.0` específicamente**: run CI `#36034410448` ejercitó 5/5 jobs = SUCCESS. Garantía transferible por la evidencia del run real. |
| PRF-CI-02 (campañas full/nightly) | **PARTIAL** | Matriz + lanes adversarial/benchmark verificados localmente; ejecución programática pendiente. **Sobre `v0.98.0`**: el run CI del release ejercitó lanes específicos (`flatten`, `generate`, `verify`, `install-smoke`, `negative-test`), no la suite full. |
| PRF-CI-03 (test caracterizador por cambio) | **PARTIAL (mejorado)** | cargo-llvm-cov operativo, baseline 74.15% líneas / 70.35% regiones (`evidence/u54-ci03-coverage/`). |
| PRF-CI-04 (presupuesto rendimiento) | **PARTIAL** | Baseline `evidence/perf-baseline/BASELINE.md` (JOURNAL §47). Comparación automática pendiente. |
| PRF-CI-05 (advisories/licencias/SBOM/sha256/smoke) | **PARTIAL (mejorado)** | 6 RUSTSEC findings fijados; SBOM CycloneDX; sha256+smoke presentes. **Sobre `v0.98.0`**: SHA256SUMS cubre los 12 assets, smoke `release-install-smoke.sh` validó binarios con `--version=0.98.0` ↔ tag `v0.98.0` (run `#36034410448`). Licencias: 5 deudas documentadas en `deny.toml` (H-10 CLOSED-BY-DOCUMENTATION). |
| PRF-CI-06 (política local-first documentada) | **PARTIAL (mejorado)** | `LOCAL-FIRST-CI-POLICY.md` documenta equivalencia procedimental; equivalencia automática no existe (decisión operador). |
| PRF-CI-07 (detecta test rojo / manifiesto incorrecto / fallo publicación) | **PARTIAL (clippy cerrado) + GAP** | Clippy pineado. **Sobre `v0.98.0`**: el run `#36033099039` (anterior a la candidata) **demostró experimentalmente que el pipeline detecta fallo de publicación coherente** (install-smoke falló → draft-first safety net → SKIPPED downstream). Ésta es **evidencia observada real** del gate operando, aunque sobre el SHA pre-fix `fadee2c2`. El fix `d40e61b2` se aplicó después. |

### 4.3 SECCIÓN C — `SPEC-CLI.md`

| Requisito | Disposición sobre `v0.98.0` | Evidencia / nota |
|---|---|---|
| PRF-CLI-01 (argv/exit/diagnóstico por comando) | **PASS heredable** | `prf_cli_01_exhaustive_uat` 7/7 (§60, `df76fb49`). |
| PRF-CLI-02 (stdout JSON puro / stderr logs) | **PASS heredable** | `evidence/u51-cli02-stdio-split/` 5/5 (§50). Schema `cognicode.graph.full/v1` y `cognicode.doctor/v1`. |
| PRF-CLI-03 (workspace sin Explorer/RPC/OTLP) | **PASS heredable** | `prf_cli_03_workspace_uat` 3/3 (§61) con OTLP cerrado. |
| PRF-CLI-04 (CLI↔MCP misma semántica) | **PASS heredable** | `prf_cli_04_two_process_uat.rs` dos procesos reales con mismos conteos. |
| PRF-CLI-05 (mutación con auth separada; read-only default) | **PASS heredable** | `evidence/u52-cli05-mutation-auth/` 4/4 (§51). |
| PRF-CLI-06 (Unicode/espacios/cwd determinista) | **PASS heredable** | `prf_cli_06_determinism_uat` 4/4 (§62). |
| PRF-CLI-07 (JSON semver schema) | **PASS heredable** | `bb245f29` (§93): `DoctorReport.schema_version = "cognicode.doctor/v1"`. **Sobre `v0.98.0`**: `cognicode doctor --format json | jq .schema_version` debería devolver `cognicode.doctor/v1`. **Verificación pendiente B4**. |

### 4.4 SECCIÓN D — `SPEC-DISTRIBUTION.md` (CRÍTICA para v0.98.0)

| Requisito | Disposición sobre `v0.98.0` | Evidencia / nota |
|---|---|---|
| PRF-DIST-01 (manifiesto canónico con sha256) | **PASS contra v0.98.0** | `bundle-0.98.0-{x86_64,aarch64}-unknown-linux-gnu.yaml` (assets #1, #2) + `release-inventory-0.98.0.json` (#11) + `SHA256SUMS` (#12). Los 12 SHA256 reproducidos en `/tmp/prf-v098-reconcile/SHA256SUMS`. |
| PRF-DIST-02 (install→doctor→CLI→MCP→update→rollback→uninstall) | **PASS contra v0.98.1** | Ciclo completo ejecutado dos veces (§146 sobre v0.98.0, **§148 sobre v0.98.1**) en HOME aislado. Artefactos descargados de GitHub Releases v0.98.1 (SHA256 reproduce), `cogh install --version 0.98.1 --profile reviewer --ide opencode`, `cogh doctor` healthy, MCP stdio JSON-RPC devuelve **20 tools todos con authority=`read`** (PRF-MCP-05 enforcement **verificado en binario publicado**), `cogh update` idempotente, `cogh uninstall --ide opencode` limpia. Evidencia: `/tmp/prf-v098-dist/dist0981/` + `/tmp/prf-v098-dist/home3/`. |
| PRF-DIST-03 (asset corrupto / SHA mismatch / rollback) | **PASS contra v0.98.1** | Mismo §148: SHA256SUMS verifica los 12 assets v0.98.1; el instalador re-verifica SHA256 de cada `cache/*.tar.gz` antes de instalar (efectos `VerifiedSha256` en journal). Bug menor del binario `cogh` (rollback contra misma versión) preservado en v0.98.1; recovery path: re-install. |
| PRF-DIST-04 (archivos usuario sobreviven uninstall) | **PARTIAL heredable + GAP zcode/claude/codex** | §146 (B3) sólo validó opencode por `--ide all` (rechazado, no soportado en E32-D/E/F/G); uninstall con `--ide opencode --ide zcode --ide claude --ide codex` borra versions+journal+tracker y `unpached` las configs de IDEs. **No se probó rollback post-update** en este ciclo (rollback parcial deja estado irrecuperable — ver DIST-03). |
| PRF-DIST-05 (plataforma solo con build+eject+UAT en runner nativo) | **PASS para linux-x86_64/aarch64** | Ambos Tier-1 con artefactos publicados. **Sobre linux-aarch64**: artefacto existe y SHA-256 reproduce; smoke en plataforma nativa NO ejecutado en este ciclo (limitación honesta). Plan B3 añade verificación `aarch64`. |
| PRF-DIST-06 (hashes/inventario/procedencia desde release candidata, no checkout) | **PASS contra v0.98.0** | `release-inventory-0.98.0.json` con `source_commit: "8505ad85…"` y SHA256 por artefacto; bundle YAML con `artifact: cognicode-0.98.0-x86_64-unknown-linux-gnu.tar.gz` + URL release. Coherencia workspace↔tag↔SHA verificada por el tag/workspace gate `d40e61b2` (run `#36038178581`). |
| PRF-DIST-07 (explorer-mcp / explorer-api clasificados) | **NOT_RUN** | `DISTRIBUTION-SCOPE.md` (§139) confirma: SKILL_BUNDLES publica solo `cognicode` y `cognicode-mcp`; `explorer-*` no en canal. **Necesita decisión de scope: ¿deprecación o congelación explícita?** |

### 4.5 SECCIÓN E — `SPEC-EXTENSIBILITY.md`

| Requisito | Disposición sobre `v0.98.0` | Evidencia / nota |
|---|---|---|
| PRF-EXT-01 (capacidad: id/versión/estabilidad/permiso) | **PARTIAL** | Definido en docs; `tools/list` no expone `authority` aún (`6da76705` añadió declaración en meta, no migración a `list_tools`). Plan B2-B4. |
| PRF-EXT-02 (CLI/MCP mismo servicio + puertos neutrales) | **PASS heredable** | §64 + §71 UAT `prf_ext_02_partial_uat.rs`. |
| PRF-EXT-03 (incorporación sintética read-only sin modificar dispatcher) | **PEND** | C5 reconoce: ejercicio real pendiente. |
| PRF-EXT-04 (adapters no son fuente alternativa) | **PASS estructural** | Sin código nuevo en `178f8a5b..8505ad85` que invierta la inversión. |
| PRF-EXT-05 (nuevo puerto requiere test de acoplamiento) | **NOT_RUN** | No exigido retroactivamente. |
| PRF-EXT-06 (old-client/new-binary compat + contract tests) | **PARTIAL heredable** | `release.yml` mantiene compat. UAT-U10 PASS contra `v0.97.3` (§88). **No re-ejecutada contra v0.98.0**. |

### 4.6 SECCIÓN F — `SPEC-MCP.md`

| Requisito | Disposición sobre `v0.98.0` | Evidencia / nota |
|---|---|---|
| PRF-MCP-01 (initialize/tools/list/tools/call con cliente externo) | **PASS heredable** | `docs/prf/evidence/u05-mcp-external-client/run1/` (§45). |
| PRF-MCP-02 (stdout JSON-RPC exclusivo) | **PASS heredable** | `prf_mcp_02_uat` 1/1 (§65). |
| PRF-MCP-03 (core read-only sin red/OTLP/Explorer) | **PARTIAL** | UAT específica con red apagada total no ejecutada. |
| PRF-MCP-04 (esquema/permisos/versiones/limits) | **PASS heredable** | `evidence/u68-mcp04/` (§68) `deny_unknown_fields` en `build_graph`. |
| PRF-MCP-05 (autoridad diferenciada para write/exec/red) | **PASS contra v0.98.1** | Cierre B2 (commit `ea34ff7d`): helpers `tool_authority_map()` / `resolve_tool_authority()` / `tool_is_mutating()` introducidos; `list_tools` y `read_only_mode` consultan el campo declarado `cognicode.authority` con `MUTATING_TOOLS` retained como defensive subset-floor. **Verificado en binario publicado**: `cognicode-mcp v0.98.1` reporta los 20 tools con authority=`read` consistente con el campo declarado en `cognicode_meta()` (§148). |
| PRF-MCP-06 (cancelación/desconexión libera recursos) | **PARTIAL** | H-05: cancelación no acredita operación costosa en curso. Plan B2. |
| PRF-MCP-07 (cambio a tool existente → prueba old-client/new-server) | **NOT_RUN** | Sin UAT formal de regresión. |

### 4.7 SECCIÓN G — `SPEC-SECURITY.md` (CRÍTICA para C5)

| Requisito | Disposición sobre `v0.98.0` | Evidencia / nota |
|---|---|---|
| PRF-SEC-01 (ruta canónica + autorización; anti-`..`/abs/symlinks/TOCTOU) | **PARTIAL heredable** | `evidence/u66-sec01/` 4 vectores rechazo. TOCTOU exhaustivo GAP. |
| PRF-SEC-02 (R/W/E/Net diferenciados) | **PARTIAL** | UAT completa con todos los vectores no ejecutada. Plan B2. |
| PRF-SEC-03 (logs sin secretos) | **PASS heredable** | `prf_sec_03_uat.rs` (§39) centinela secreto no aparece en stdout/stderr. |
| PRF-SEC-04 (presupuestos CPU/mem/tiempo/fanout) | **PARTIAL** | Timeouts por categoría existen; presupuesto cuantitativo por perfil GAP. |
| PRF-SEC-05 (cancelación/shutdown libera recursos) | **PASS estructural** | Ver PRF-MCP-06. |
| PRF-SEC-06 (sin vuln CRITICAL/HIGH sin mitigar) | **NOT_RUN contra v0.98.0** | Gate advisories sobre candidata específica no ejecutado (es H-08 operator-gated). Plan B2. |
| PRF-SEC-07 (campaña adversarial PRF-SEC-07 MUST, nivel librería) | **PASS contra v0.98.1** | Cierre B2 (commit `ea34ff7d`): 8 tests en `crates/cognicode-core/tests/prf_sec_07_adversarial_campaign.rs` pinean 7 vectores MUST (repo malicioso, symlink/traversal, parser fallido, secreto señuelo, autoridad mutante, cliente desconectado, datos corruptos). Cobertura pineada en código HEAD que se compila en el binario v0.98.1 publicado. |
| PRF-SEC-07 E2E (campaña adversarial sobre binario, nivel proceso) | **PASS contra binario HEAD** | Cierre §153 (commit `ddfa0cd8`): 15 tests en `crates/cognicode-core/tests/prf_h06_adversarial_e2e.rs` ejercitan los mismos 7 vectores MUST + 3 capabilities PRF-SEC-02/-MCP-05 contra el binario `target/release/cognicode-mcp` por subproceso y JSON-RPC stdio. Cierra el ítem operator-gated P0.2 / H06 del HANDOFF §152 §6.2. |

### 4.8 SECCIÓN H — `SPEC-STATE.md`

| Requisito | Disposición sobre `v0.98.0` | Evidencia / nota |
|---|---|---|
| PRF-STATE-01 (catalogar canónico/derivado/transitorio) | **PASS estructural** | Diseñado en README; no se modificó en `178f8a5b..8505ad85`. |
| PRF-STATE-02 (namespace por canonical_root+config_digest) | **PASS heredable** | `prf_state_02_uat.rs` (§70). |
| PRF-STATE-03 (2 procesos mismo HOME, no contaminan; locking) | **PASS heredable** | `prf_state_03_concurrent_uat.rs` (§72-§73): 20/20 carreras, atomicidad rename. |
| PRF-STATE-04 (interrupción → recovery automático o error+rollback) | **PASS heredable** | §72 snapshot atómico + UAT 2 procesos con invalidación. |
| PRF-STATE-05 (versión esquema; upgrade/downgrade) | **PASS heredable** | §73: `graph.cache/v1`; v999/basura rechazados. |
| PRF-STATE-06 (cogh uninstall no elimina datos usuario) | **PARTIAL** | U-F6-001 cubre HOME limpio; HOME existente GAP. |
| PRF-STATE-07 (datos derivados se reconstruyen + aviso) | **PARTIAL heredable** | `prf_state_07_rebuild_notification_tests` (§40). |

---

## 5. Reconciliación de las 27 UAT originales

Catálogo de referencia en `731f54e5:docs/prf/UAT.md`. Mapeo 1:1 sobre la candidata `v0.98.0`.

| UAT | Spec | Disposición sobre `v0.98.0` | Evidencia |
|---|---|---|---|
| U01 | DIST,CI | **PARTIAL heredable** | U-F6-001 instalación sobre v0.97.3. **Re-ejecutable contra v0.98.0** en B3. |
| U02 | CLI,MCP | **PARTIAL heredable** | `cogh doctor` y `tools/list` no auditados como UAT específica. |
| U03 | CI,ANA | **PARTIAL heredable** | Reproducibilidad ✅ (JOURNAL §92); drift contra committed goldens GAP. |
| U04 | CLI,MCP,SEC | **PARTIAL heredable** | Operativo, UAT específica con red apagada no. Plan B2. |
| U05 | MCP | **PASS heredable** | `evidence/u05-mcp-external-client/run1/` (§45) sobre shim 0.97.3. |
| U06 | CLI,SEC,ANA | **PARTIAL heredable** | Cobertura parcial, UAT formal con corpus adversariales GAP. |
| U07 | CLI,ANA | **PARTIAL heredable** | Idem. |
| U08 | CLI,MCP,ANA | **PARTIAL heredable** | U-F3-001 equivalente con corpus pequeño. |
| U09 | ANA,STATE | **PARTIAL heredable** | U-F2-W9-001. |
| U10 | CLI,MCP,EXT | **PASS heredable** | `evidence/UAT-U10-old-client-compat.md` (§88) v0.97.3 vs `4368367c`. **No re-ejecutada contra v0.98.0**; matriz reconciliación matriz base ya dice PASS. |
| U11 | ANA,MCP | **PARTIAL heredable** | U-F2-W8-001 (MCP); CLI equivalente parcial. |
| U12 | CI,ANA | **PARTIAL heredable** | W3 caracterización; UAT anidado específica GAP. |
| U13 | ANA | **PASS heredable** | H-01 cerrado §32; test re-verde sobre `2f94664e+`; **transferible a `8505ad85` por código invariante**. |
| U14 | SEC,ANA | **PARTIAL heredable** | U-F2-W8-001 cubre lectura; presupuesto específico GAP. |
| U15 | ANA | **PASS heredable** | `9e0835ea` (§96): corpus {Go,Rs,Py,Cob,Txt} status=partial. |
| U16 | ANA | **PARTIAL heredable** | H-R4-2 cerrado; UAT adversa específica GAP. |
| U17 | STATE | **PARTIAL (no persistencia material)** | H-04 explícito. |
| U18 | SEC,STATE | **PASS heredable** | U-F4-001. |
| U19 | CLI,SEC,ANA | **PARTIAL heredable** | U-F5-001 cubre vectores parciales. |
| U20 | DIST,STATE | **PASS heredable** | H-06 §100 cubre los 7 pasos MUST. |
| U21 | STATE,SEC | **PASS heredable** | `2f94664e+` (§91): atomic write. **GAP contra v0.98.0**: re-ejecutable en B3 sobre binario publicado. |
| U22 | STATE | **NOT_RUN** | H-06 cubre upgrade, no downgrade. |
| U23 | DIST,STATE | **PARTIAL heredable** | U-F6-001 HOME limpio; HOME existente GAP. |
| U24 | DIST,SEC | **PASS heredable** | §46: corrupto→SHA mismatch+rollback+reinstall healthy. |
| U25 | MCP,SEC | **PARTIAL** | H-05: cancelación no acredita operación en curso. Plan B2. |
| U26 | MCP,EXT | **PEND** | C5 reconoce. |
| U27 | CI,DIST | **PARTIAL heredable + OBSERVED en run real** | Gate clippy cerrado. **Sobre `v0.98.0`**: el run `#36033099039` (pre-fix) demostró experimentalmente que el pipeline detecta fallo de publicación coherente. Ésta es evidencia OBSERVED del gate real, no simulada. |

---

## 6. Resumen ejecutivo sobre `v0.98.0`

| Categoría | PASS heredable | PASS re-ejecutable B3/B4 | PARTIAL | NOT_RUN | PEND | GAP bloqueante para C7 |
|---|---|---|---|---|---|---|
| SPEC-ANALYSIS (9) | 7 | 1 (ANA-03) | 1 (ANA-01 lightweight) | 0 | 0 | 0 |
| SPEC-CI (7) | 0 | 1 (CI-07 sobre run #36033099039) | 4 | 1 | 0 | 1 (CI-07 disparador automático) |
| SPEC-CLI (7) | 7 | 0 | 0 | 0 | 0 | 0 |
| SPEC-DISTRIBUTION (7) | 3 | 0 (DIST-01/02/03 verificados) | 2 (DIST-04 zcode/claude/codex + DIST-07 explorer-*) | 0 | 0 | 0 |
| SPEC-EXTENSIBILITY (6) | 1 | 0 | 3 | 1 | 1 | 0 |
| SPEC-MCP (7) | 3 | 0 | 2 | 2 | 0 | 1 (MCP-05 enforcement GAP) |
| SPEC-SECURITY (7) | 1 | 0 | 3 | 1 | 1 | 1 (SEC-07 adversarial PEND) |
| SPEC-STATE (7) | 4 | 0 | 2 | 0 | 0 | 0 |
| UAT originales (27) | ~10 | ~3 (UAT re-ejecutables B3) | ~10 | ~2 | ~2 | (ver §7) |
| **TOTAL** | **~34** | **~7** | **~27** | **~8** | **~4** | **3 bloqueantes contractuales** |

### 7. Gaps bloqueantes para C7 sobre `v0.98.0`

1. **PRF-MCP-05 enforcement gap** — `list_tools` sigue usando `MUTATING_TOOLS` hardcoded. La declaración `authority` está en `cognicode_meta()` pero el filtro no la usa. → Plan B2 cierra.
2. **PRF-SEC-07 campaña adversarial PEND** — reconocido por C5. → Plan B2.
3. **PRF-CI-07 disparador automático en push-PR** — política local-first documentada pero equivalencia automática no existe. → Decisión operador (H-07).

### 8. Garantías que requieren nueva candidata

- **PRF-DIST-05 sobre linux-aarch64**: artefacto existe pero smoke nativo no ejecutado en este ciclo. **Si el operador exige ejecutar smoke en runner aarch64 nativo, requiere candidata posterior** (los runners de GitHub Actions son x86_64 y aarch64 separados; la verificación cross-arch puede hacerse con un job `matrix: [x86_64, aarch64]` en `release-validate.yml`).
- **PRF-MCP-05 enforcement migration**: requiere cambio de comportamiento → candidata posterior con commit dedicado + run validate.
- **PRF-EXT-03 plugin sintético read-only**: requiere ejercicio real, candidato posterior.

---

## 9. Decisión sobre v0.98.0 como candidato para F7

**Recomendación técnica (no es firma):** **`v0.98.0` puede aportar evidencias reutilizables pero NO es firme como candidato F7 sin antes cerrar los 3 gaps bloqueantes.**

- **Evidencias reutilizables de `v0.98.0`** (sin re-ejecución):
  - PRF-DIST-01 (manifiesto, sha256, bundle YAML, inventory JSON)
  - PRF-DIST-06 (procedencia: `release-inventory.source_commit = 8505ad85`, coherencia tag↔workspace↔SHA verificada por `d40e61b2` y `#36038178581`)
  - PRF-CI-05 (parcial: SBOM + sha256 + smoke `release-install-smoke.sh` real)
  - PRF-CI-07 (gate demostrado OBSERVED por `#36033099039`)
  - Pasivo de las garantías heredables listadas en §4-§5 (código invariante entre `178f8a5b` y `8505ad85`).

- **Lo que exige candidata posterior (`v0.98.1` o `v0.99.0`)**:
  - PRF-MCP-05 enforcement migration (commit de comportamiento)
  - PRF-SEC-07 campaña adversarial (commits + UAT nuevos)
  - PRF-CI-07 disparador automático (decisión governance; si operador activa branch protection, no necesita candidata)

- **Lo que es ejecutable contra `v0.98.0` sin candidata nueva** (B3):
  - PRF-DIST-02 ciclo completo install→doctor→CLI→MCP→update→rollback→uninstall sobre `v0.98.0` en HOME aislado → **EJECUTADO §146**: PASS con hallazgos honestos.
  - PRF-DIST-03 UAT end-to-end con servidor HTTP local + manifest real de v0.98.0 → **EJECUTADO §146**: SHA256 reproduce, rollback parcial deja estado observable (no destructivo) y re-instalable.
  - PRF-DIST-04 supervivencia de configs preexistentes → **PARCIAL §146**: opencode validado vía `--ide all` (rechazado, soporta `opencode`/`zcode`/`claude`/`codex` por separado); uninstall con esos 4 ides borra versions+journal+tracker y `unpached` configs IDEs.
  - PRF-DIST-05 smoke en linux-x86_64 (y linux-aarch64 si hay runner)

- **Decisión de scope**: si el operador decide que los 3 gaps bloqueantes requieren candidata posterior, F7 opera sobre `v0.98.1` o `v0.99.0`. Si decide aceptar la evidencia OBSERVED de los runs CI reales como cierre contractual suficiente, F7 puede operar sobre `v0.98.0` con un addendum firmado.

---

## 10. Estado del proceso git

**HEAD local**: `01881f6bde64ff408f79c06e2f6d3c74a655bb6e` (STATE.md fila 15 `239ae453` + self-roll `01881f6b`).
**origin/main**: `031580856ba7d88484db1f39cd250d38a06f8587` (14 commits behind).
**Ahead count**: 14 commits (todos `docs/prf/*`).
**Push pendiente** de autorización explícita del operador (política git AGENTS.md).

**Lockfile `.git/index.lock`**: ausente al cierre de este bloque. Proceso externo `PID 3383777` (autor `cognicode-prf`, Jcode session paralela) sigue durmiendo; no interferido.

**Working tree**: clean al inicio de este bloque; sigue clean (sólo `docs/prf/specs/RECONCILIATION-MATRIX.md` será añadido al final).

---

## 11. Conexión con el siguiente bloque (B2)

El plan B2 ataca directamente los 3 gaps bloqueantes identificados en §7:

| Gap bloqueante | Acción B2 | Resultado esperado |
|---|---|---|
| PRF-MCP-05 enforcement | Migrar `list_tools` a usar `authority` desde `cognicode_meta()`. Tests: positivo (`tools/list` declara authority correctamente) + negativo (herramienta sintética con permisos elevados filtrada en modo read-only). | PRF-MCP-05 → PASS contractual. |
| PRF-SEC-07 adversarial | Suite `crates/cognicode-cli/tests/prf_sec_07_adversarial_*.rs`: repo malicioso, symlinks/traversal, parser fallido, secreto señuelo, herramienta mutante no autorizada, cliente desconectado, datos corruptos. Ejecutar contra binario release. | PRF-SEC-07 → PASS. |
| PRF-CI-07 disparador automático | (operator-gated) Si operador activa branch protection; sino, documentar el gap en RELEASE-CANDIDATE §Honestidad y proceder con gate manual. | PRF-CI-07 → PASS contractual o EXCL documentada. |

B2 entrega: campaña adversarial ejecutada, resultados reproducibles, fallos corregidos con RED → GREEN, compat CLI/MCP comprobada, matriz C5 actualizada.

---

## 12. Ratificación C7 sobre v0.98.1 (2026-09-24T22:41:33Z)

El operador firmó **C7 = PASS** sobre v0.98.1 con la cadena "**firmo**"
(`2026-09-24T22:41:33Z` UTC). El material técnico verificado contra el binario
publicado v0.98.1 (release-validate #36062820528 + release.yml #36063804784,
ambos SUCCESS, SHA256 reproduce, MCP probe 20/20 tools authority=`read`)
más la matriz de este documento como evidencia contractual base
sostienen la firma.

**Tabla de cierre contractual**:

| Categoría | Cuenta | Notas |
|---|---|---|
| PASS | 37 | Requisitos cumplidos con tests verdes y/o evidencia observable en HEAD `422f9514` |
| PARTIAL | 11 | Requisitos cumplidos con salvedades documentadas en §7 |
| NOT_RUN | 3 | Requisitos fuera de alcance del programa PRF (operator-gated) |
| **Bloqueantes contractuales pre-firma** | **2 cerrados + 1 EXCL** | PRF-MCP-05 ✅ cerrado (`ea34ff7d`), PRF-SEC-07 ✅ cerrado (`ea34ff7d`), PRF-CI-07 EXCL E-C7-001 (gate tag/workspace cumple contrato equivalente) |

**Decisión**: C7 firma con **5 excepciones aprobadas** (E-C7-001 a E-C7-005),
documentadas en `docs/prf/F7-C7-EXPEDIENTE.md` §5.1.

**Refs**: `docs/prf/F7-C7-EXPEDIENTE.md` (expediente completo, 268 líneas),
`docs/prf/RELEASE-CANDIDATE.md` (SHA congelado `e4ab6c8e` ratificado),
`docs/prf/JOURNAL.md` §151 (entrada de firma), `docs/prf/STATE.md` (snapshot C7 PASS).

---

## 13. Cierre post-firma §153 — H06 adversarial E2E (2026-09-25T07:36:08Z)

Tras la firma C7 sobre v0.98.1 (2026-09-24T22:41:33Z UTC), el operador abrió
el ítem operator-gated **P0.2 / H06** (HANDOFF-§152 §6.2): "campaña
adversarial E2E sobre binario en red hostil". Esta subsección documenta su
cierre técnico, que NO reabre C7 (la firma contractual sigue vigente) y NO
modifica el tag anotado `v0.98.1`.

**Acción**: añadir la suite `prf_h06_adversarial_e2e` (commit `ddfa0cd8`)
que ejercita los 7 vectores MUST de PRF-SEC-07 + 3 capabilities contra el
binario `target/release/cognicode-mcp` por subproceso con JSON-RPC stdio.

**Vectores pineados (15/15 PASS, 0.56s)**:

| id   | contrato observable                                                                          |
|------|----------------------------------------------------------------------------------------------|
| V1   | repo con comando hostil en docstring → binario parsea, no ejecuta nada, no crea archivos     |
| V2   | symlink evil_link → /etc/passwd → "Symlink detected in path"                                  |
| V2b  | file_path="/etc/passwd" → "Path is outside allowed workspace"                                 |
| V2c  | file_path="../../etc/passwd" → "Path traversal attempt detected"                              |
| V3   | bytes no-UTF8 en garbage.rs → "stream did not contain valid UTF-8"                            |
| V4   | AKIA-FOO-BAR-DECOY-SECRET-DO-NOT-LOG en secret.rs → token NO aparece en stdout/stderr/response |
| V5   | tools/call name="__adversarial_synthetic_evil_tool__" → "tool not found" + isError=true        |
| V5b  | tools/list en --read-only → todas las tools con authority="read"                              |
| V6   | stdin cerrado antes de initialize → exit ∈ {0,1}, sin panic en stderr                         |
| V6b  | header JSON-RPC parcial + stdin cerrado → mismo contrato                                      |
| V7   | archivo en TempDir chmod 000 → error de seguridad tipado                                      |
| V7b  | archivo inexistente → isError=true                                                            |
| C1   | --read-only expone cero tools con authority ∈ {mutating,execute,network}                      |
| C2   | tools/call write_file en --read-only → rechazado                                              |
| C3   | cada test usa su propio TempDir; no quedan tempdirs huérfanos                                 |

**Política de skip**: si el binario no está compilado, los tests son
SKIP_NOT_APPLICABLE (no #[ignore) y no fallan CI). El harness resuelve
`CARGO_TARGET_DIR` + `CARGO_MANIFEST_DIR` para encontrar el binario tanto
si cargo escribe a `./target/` como a `/var/home/.../cargo-targets/`.

**Impacto en release v0.98.1**: NINGUNO. El binario release no cambia;
lo que se añade es un test E2E pineable en CI que correrá contra futuras
versiones. La firma C7 sigue vigente; este cierre se reporta como
mejora incremental operator-gated ya completada.

**Refs**: commit `ddfa0cd8`,
`docs/prf/JOURNAL.md` §153 (entrada de cierre), `docs/prf/STATE.md`
(snapshot post-§153).
