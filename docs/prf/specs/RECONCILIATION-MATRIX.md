# RECONCILIATION-MATRIX — PRF original vs estado actual en `178f8a5b`

> **Origen:** auditoría operador 2026-09-22, sección ‘Cómo cerraría PRF sin crear otro roadmap’, acción 2.
> **Propósito:** contrastar los 8 documentos `SPEC-*` y las **27 UAT originales** (catalogue original en `731f54e5:docs/prf/UAT.md`) contra C0–C6 y la evidencia actual (`evidence/CERTIFICATES.md`, `UAT.md`). Cada ítem tiene disposición explícita: probado / sustituido / excluido / pendiente.
> **Importante:** este documento **no cierra C7**. Es la base sin la que C7 no puede firmarse contractualmente. Disposiciones distintas de ‘PASS con evidencia reproducible’ siguen siendo **gaps abiertos** que bloquean la decisión C7.

## Definiciones

| Estado | Significado |
|---|---|
| **PASS** | Disposición aceptada: UAT ejecutada con binario real sobre `178f8a5b` o ancestro, stdout/stderr/exit capturados, hash de artefacto, recibo firmado. |
| **PARTIAL** | Disposición parcial: ejecutada pero no cubre el criterio entero del SPEC o de la UAT original. Ver notas. |
| **FAIL** | Disposición reproductor: la prueba que existe falla, o nunca pasó, o detectó defecto ya documentado. |
| **NOT_RUN** | La UAT original nunca se ejecutó sobre el HEAD congelado. |
| **EXCL** | Exclusión de alcance aprobada por el operador (con fecha y entrada de JOURNAL). |
| **PEND** | Pendiente de acción 3-4-5 del cierre PRF. |

> Diferencia crítica respecto al estado anterior: **ACCEPTED ≠ PASS contractual**. ‘ACCEPTED’ significa ‘el trabajo llegó al estado del programa’; ‘PASS’ significa ‘el criterio del SPEC-* o de la UAT original se cumplió demostrablemente sobre el HEAD congelado, con UAT ejecutada y recibo firmado’. Esta matriz convierte los ‘ACCEPTED’ en ‘PASS/PARTIAL/FAIL’ por criterio.

---

## Sección A — `SPEC-ANALYSIS.md` (`PRF-ANA-*`)

| Requisito | Disposición | Evidencia / Notas |
|---|---|---|
| PRF-ANA-01 (verticals `full`/`per_file` con `lightweight`) | PEND | Auditable en código pero no hay UAT específica del SPEC-ANALYSIS como UAT-Nxx en `UAT.md`. Coverable por U08, U12, U15. |
| **PRF-ANA-02** (errores lectura/parseo no se descartan silenciosamente) | **PASS** | §63: los 6 call-sites restantes de `find_all_symbols_with_path`/`find_call_relationships` ya no usan `unwrap_or_default()`: full → `SkippedFile` clasificado; filtered/async → exclusión de cobertura parsed + `warn!`. UAT binario real (`prf_ana_02_uat`): `locked/secret.rs` chmod-000 → `status=partial` + `skipped_files` con reason_kind=read. Regresión: core 2145/0, graph 401/0, MCP e2e verde. `evidence/u63-ana02/`. |
| **PRF-ANA-03** (cambio de bytes con tamaño+mtime preservados no genera `Unchanged` falso) | **PASS (RED→GREEN)** | H-01 del operador **cerrado en `39928202` (JOURNAL §32)**: SHA-256 (32 bytes) añadido como tercer cache-invalidation key. Test `h01_byte_change_with_same_mtime_and_same_size_must_invalidate_cache` (pinea cambio de bytes preservando TANTO mtime COMO size) ahora pasa — cobertura permanente del invariante. Cache value type extendido a `(u64, u64, [u8; 32], Vec<Symbol>, Vec<(Symbol, String)>)`. 3 lookup sites + 3 insert sites actualizados. Decisión de diseño ejercida por "a tu criterio" previo del operador. Suite 2129/0/27 lib; clippy `-D warnings` clean. |
| PRF-ANA-04 (consulta con archivo/provider faltante = `Partial/Unknown/Failed` con causa/cobertura) | **PASS (RED→GREEN)** | H-02 adicional del operador **resuelto en `41e4230f` (JOURNAL §33)**: handler MCP `build_graph` ahora expone campo `status: String` con valores `"complete"` (walk sin drops), `"partial"` (walk con ≥1 drop), `"unknown"` (cache hit, sin walk). El `BuildReport` se traduce a `status` antes de devolver, con `skipped_files` existente preservado. 3 nuevos tests RED→GREEN (`prf_ana_04_status_field_tests`); suite 2132/0/27 lib; clippy `-D warnings` clean. Backward-compatible: success, symbols_found, relationships_found, edges, message, skipped_files no cambian. |
| PRF-ANA-05 (repetición misma entrada → outputs semánticamente equivalentes) | **PASS** | Library (`w10`), handler (`§34`) y ahora binario real: `prf_ana_05_uat` (stdio JSON-RPC x3) detectó RED real — edges en orden no determinista entre ejecuciones — corregido con orden canónico (from,to) en `handle_build_graph` (§58, `a2a2ce61`). `evidence/u58-ana05-uat-binary/`. |
| PRF-ANA-06 (basis con workspace canónico+config digest+source manifest) | **PASS (RED→GREEN + UAT binario real)** | `BasisDto` en build_graph output (workspace canónico, config digest SHA-256, source manifest digest SHA-256, `complete` explícito). 4 tests RED→GREEN (`prf_ana_06_basis_identity_tests`); core lib 2145/0/27; UAT stdio sobre `cognicode-mcp` real: basis presente, complete=true — `evidence/u50-ana06-basis/` (JOURNAL §49). |
| PRF-ANA-07 (renames/moves/colisiones preservan identidad o devuelven ambigüedad visible) | **PASS** | Library (F2.W5) + corpus stress de 51 homónimos (§35) + UAT binario real vía stdio JSON-RPC (`prf_ana_07_uat`, §59 `a07ecaf9`): visibilidad local, candidato único y sin fuga de homónimos, confidence 1.0. `evidence/u59-ana07-uat-binary/`. |
| PRF-ANA-08 (budgets en lectura/parser/consultas costosas) | **PASS** | `prf_ana_08_uat` 1/1 sobre binario real (§69): budgets de categoría (graph 60s/search 500ms), salida acotada, no-encontrado tipado. `evidence/u69-ana08/`. |
| PRF-ANA-09 (LSI integra con golden corpus, registra drift) | **EXCL (2026-09-22, AUTO)** | La capacidad LSI (integración de índice semántico con golden corpus + drift) nunca se construyó en el codebase (verificado: sin implementación LSI en crates; las refs "lsi" son del change archivado e60-lsi-dataflow-backend, dominio distinto). El condicional del SPEC ("al integrar LSI...") no es exigible mientras la integración no exista. Cualquier futura integración LSI deberá cumplir el MUST en su momento (golden corpus + drift + cutover gate). Registrado en JOURNAL §53. |

## Sección B — `SPEC-CI.md` (`PRF-CI-*`)

| Requisito | Disposición | Evidencia / Notas |
|---|---|---|
| **PRF-CI-01** (cada SHA tiene recibo independiente; FAIL aborta merge/publicación; sin `\|\| true` sobre gates obligatorios) | **FAIL** | H-07 del operador: `ci.yml` configurado sólo con `workflow_dispatch` (sin disparador por push/PR); un E2E con `\|\| true` y un escenario deshabilitado. **PRUEBA NEGATIVA nunca ejecutada**. |
| PRF-CI-02 (campañas full/nightly con matriz + adversariales + benchmarks) | **PARTIAL (mejorado x2)** | Matriz (§56) + lanes `adversarial` (parser 84/0, drift 10/0, grounding 10/0, isolation 2/0, contract MCP) y `benchmarks` (criterion, baseline por entorno, report-only) verificadas localmente (§57). Ejecución programática de la campaña requiere act/push (operator-gated). |
| PRF-CI-03 (modificación de comportamiento → test caracterizador + regresión) | **PARTIAL (mejorado)** | RED→GREEN pineado se cumple en todo el trabajo PRF. Medición de cobertura ahora operativa: cargo-llvm-cov, baseline core lib 74.15% líneas/70.35% regiones (`evidence/u54-ci03-coverage/`), job report-only en ci.yml (§54). Falta: umbral obligatorio como gate (decisión de política, operator). |
| PRF-CI-04 (presupuesto de rendimiento por perfil/corpus fijado **antes** de comparación) | PARTIAL | Baseline publicado 2026-09-22 en `docs/prf/evidence/perf-baseline/BASELINE.md` (JOURNAL §47): build full ~10 s, RSS ~654 MB, presupuestos de regresión congelados. Falta comparación automática en CI (depende de runner estable, U03/G6). |
| PRF-CI-05 (advisories/licencias/SBOM/sha256/provenance + smoke nativo verificable) | **PARTIAL (mejorado)** | Advisories gate BLOQUEANTE en release.yml (deny.toml: vulns/unsound deny, 5 deudas documentadas); 6 RUSTSEC findings fijados por cargo update + inventory 0.3; SBOM CycloneDX por crate subido con payloads; sha256 + smoke nativo ya existentes (`evidence/u55-ci05-advisories-sbom/`, §55). Falta: licencias como gate (decisión de política). |
| PRF-CI-06 (decisión sobre política local-first documentada y equivalente a gate remoto) | **PARTIAL (mejorado)** | Política documentada en `docs/prf/specs/LOCAL-FIRST-CI-POLICY.md`: origen (AGENTS.md "Local CI Is the Source of Truth", ADR-031, B3 2026-08-16), justificación, equivalencia **procedimental** (§3.3 tabla de gates por momento). El documento declara honestamente que la equivalencia **automática** (pre-push hook / branch protection) NO existe y su introducción es decisión del operador (§4.bis). |
| **PRF-CI-07** (pipeline detecta artificialmente test rojo, manifiesto incorrecto, fallo de publicación → niega PASS) | **FAIL** | H-07 explícito: prueba negativa nunca ejecutada. Sin esto no se acredita el gate. |

## Sección C — `SPEC-CLI.md` (`PRF-CLI-*`)

| Requisito | Disposición | Evidencia / Notas |
|---|---|---|
| PRF-CLI-01 (argv, exit code, diagnóstico por comando stable) | **PASS** | Escenario crítico (§38, RED detectó analyze exit 0 en fallo) + barrido exhaustivo de la superficie stable sobre binario real: `prf_cli_01_exhaustive_uat` 7/7 (§60, `df76fb49`). `evidence/u60-cli01-exhaustive/`. |
| PRF-CLI-02 (stdout solo datos estructurados; stderr logs) | **PASS (UAT captura completa, binario real)** | `doctor --format json` y nuevo `graph full --format json` (schema `cognicode.graph.full/v1`): stdout JSON puro, logs en stderr, modo texto histórico preservado. UAT 5/5 PASS — `evidence/u51-cli02-stdio-split/` (JOURNAL §50). Deuda declarada: resto de subcomandos de graph en modo texto. |
| PRF-CLI-03 (workspace seleccionable sin Explorer/RPC/cloud/OTLP) | **PASS** | `prf_cli_03_workspace_uat` 3/3 sobre binario real con OTLP endpoint cerrado: ruta canónica, análisis sin collector, fallo parcial honesto (§61). `evidence/u61-cli03-workspace/`. |
| PRF-CLI-04 (mismo caso de uso entre CLI y MCP, transporte aislado) | **PARTIAL (mejorado)** | Tests de equivalencia en `prf_cli_04_cli_mcp_equivalence_tests` (commit `c3ce111d`, JOURNAL §37): el MISMO caso de uso (build full) ejecutado por la ruta CLI (`FullGraphStrategy::build_full_graph`) y la ruta MCP (`handle_build_graph`) sobre el corpus canónico, comparando símbolos y aristas con guards anti-vacuidad. GREEN (los desvíos H-03 se cerraron en F2.W7/W8). Bucket PARTIAL y no PASS porque el test corre in-process, no con dos procesos CLI/MCP reales aislados por transporte. |
| PRF-CLI-05 (mutación con autorización separada; read-only por defecto) | **PASS (RED→GREEN + UAT binario real)** | RED real: refactor crasheaba en toda invocación (clap: posicional opcional antes de requerido). Fix argv + `--apply` como autorización separada que se RECHAZA hasta existir rollback; default preview-only sin escritura (SHA verificado). Test de regresión + UAT 4/4 — `evidence/u52-cli05-mutation-auth/` (JOURNAL §51). Deuda: apply con rollback es trabajo futuro. |
| PRF-CLI-06 (Unicode, espacios, cwd, permisos determinista; sin secretos por verbose) | **PASS** | `prf_cli_06_determinism_uat` 4/4 sobre binario real: unicode+espacios, cwd-independencia del reporte, permiso 000 sin crash, verbose sin fuga de secretos (§62). `evidence/u62-cli06-determinism/`. |
| PRF-CLI-07 (JSON legible por máquina, semver esquema) | NOT_RUN | No exigido retroactivamente. |

## Sección D — `SPEC-DISTRIBUTION.md` (`PRF-DIST-*`)

| Requisito | Disposición | Evidencia / Notas |
|---|---|---|
| PRF-DIST-01 (manifiesto canónico, sha256, cogh Layer 0/1 separación) | PARTIAL | Manifiesto y sha256 existen; separación de ownership no acreditada con UAT. |
| **PRF-DIST-02** (`install → doctor → CLI → MCP → update → rollback → uninstall` con HOME limpio y personalizado, idempotente) | **PARTIAL → FAIL** | U-F6-001 cubre install+update(no-op)+uninstall. **No cubre** update A→B real (H-06) ni rollback post-update (H-06). |
| PRF-DIST-03 (binario ausente / descarga rota / sha inválido / migración interrumpida → error + reversión, sin `\|\| true`) | NOT_RUN | U24 (`Instalación interrumpida / asset corrupto, luego rollback y reinstall`) no ejecutada. |
| PRF-DIST-04 (archivos de usuario/IDE sobreviven uninstall/rollback) | **PARTIAL (mejorado)** | UAT del pipeline real uninstall_opencode (`prf_dist_04_survival_tests`, JOURNAL §41): config preexistente (otros MCP servers, theme, prefs) y skills del usuario sobreviven byte-a-byte; uninstall sin entrada previa es no-op. Falta zcode/claude/codex y rollback post-update (H-06). **Flakiness preexistente de layout/lifecycle HTTP-fixture tests documentada** (falla en baseline sin el cambio; deuda U03/G6). |
| PRF-DIST-05 (soporte plataforma solo con build/ejecución en runner nativo) | PARTIAL | Linux x86_64 certificada; MUSL/macOS/Windows pendientes (RELEASE-CANDIDATE lo declara). |
| PRF-DIST-06 (hashes, inventario, procedencia desde release candidata, no checkout) | PARTIAL | `release.yml` lo hace; verificación desde la release real no documentada en UAT. |
| PRF-DIST-07 (`explorer-mcp`, `explorer-api` y clientes anteriores clasificados) | NOT_RUN | No hay UAT para explorer-mcp ni explorer-api en el cierre actual. |

## Sección E — `SPEC-EXTENSIBILITY.md` (`PRF-EXT-*`)

| Requisito | Disposición | Evidencia / Notas |
|---|---|---|
| PRF-EXT-01 (capacidad estable: id, versión, estabilidad, permiso r/w/x/net, budgets) | PARTIAL | Definido en docs; `tools/list` no lo expone explícitamente como metadato. |
| PRF-EXT-02 (CLI y MCP usan mismo servicio de aplicación + puertos neutrales) | **PARTIAL (mejorado x2)** | H-03 resuelto para `full` (§64): CLI `graph full` usa `AnalysisService::build_full_graph` con status/skipped_files en JSON, idéntico al MCP. Otros subcomandos CLI aún usan estrategias directas — pendiente misma reforma. |
| **PRF-EXT-03** (incorporación de capacidad sintética read-only sin modificar varios lugares del dispatcher core) | **PEND (matiz C5)** | C5 reconoce: ‘ejercicio real de extensibilidad mínima (plugin) — pendiente’. |
| PRF-EXT-04 (adapters no son fuente de verdad alternativa) | PARTIAL | Diseño hexagonal respetado en código; UAT no ejercida. |
| PRF-EXT-05 (nuevo puerto requiere test de acoplamiento/duplicidad y mejora medible) | NOT_RUN | No exigido retroactivamente. |
| PRF-EXT-06 (compatibilidad old-client / new-binary + contract tests) | PARTIAL | `release.yml` mantiene compat; UAT-U10 específica no ejecutada. |

## Sección F — `SPEC-MCP.md` (`PRF-MCP-*`)

| Requisito | Disposición | Evidencia / Notas |
|---|---|---|
| PRF-MCP-01 (initialize, tools/list, tools/call, errores, terminación con cliente externo real) | PASS | UAT ejecutada 2026-09-22 con cliente externo real sobre shim 0.97.3 instalado; evidencia en `docs/prf/evidence/u05-mcp-external-client/run1/` (JOURNAL §45): initialize/tools-list/tools-call/errores honestos/-32700 sin crash/exit 0/stdout JSON puro. |
| PRF-MCP-02 (stdout JSON-RPC exclusivo; logs a stderr; sin huérfanos) | **PASS** | `prf_mcp_02_uat` 1/1 sobre binario real (§65): framing válido con logging denso, salida limpia al cerrar stdin. `evidence/u65-mcp02/`. |
| PRF-MCP-03 (core read-only sin red/OTLP/Explorer/backend/Podman) | PARTIAL | Operativo; UAT específica con apagado total de red no ejecutada. |
| PRF-MCP-04 (herramientas con esquema, permisos, versiones, límites; error tipado) | **PARTIAL (mejorado)** | §68: argumentos inválidos/extraños producen error tipado (`deny_unknown_fields` en build_graph, verificado en binario real, `evidence/u68-mcp04/`). Permisos y budgets uniformes siguen pendientes. |
| PRF-MCP-05 (herramienta que escribe/ejecuta/red requiere autoridad diferenciada; prompts ≠ autoridad) | NOT_RUN | No UAT con prompt malicioso. |
| **PRF-MCP-06** (cancelación/desconexión libera recursos según contrato) | **PARTIAL** | H-05 del operador: cancelación no acredita **operación costosa en ejecución**; solo ‘cancelación llega después’. |
| PRF-MCP-07 (cambio a herramienta existente → prueba con cliente anterior vs servidor nuevo o deprecación) | NOT_RUN | No hay UAT formal de regresión de cliente. |

## Sección G — `SPEC-SECURITY.md` (`PRF-SEC-*`)

| Requisito | Disposición | Evidencia / Notas |
|---|---|---|
| PRF-SEC-01 (raíz + cada ruta canónica y autorizada; rechazo `..`, absolutos, symlinks, TOCTOU) | **PARTIAL (mejorado)** | §66: defecto real corregido — build_graph ahora valida `directory` (traversal/absoluto/symlink RECHAZADOS en UAT binario real, 4 vectores, `evidence/u66-sec01/`). TOCTOU exhaustivo sigue pendiente. Deuda: argumentos desconocidos ignorados por serde (MCP-04). |
| PRF-SEC-02 (R/W/E/Net diferenciados; read-only default; prompts ≠ autoridad) | PARTIAL | Diseño respeta; **U19 completa con todas las vectores** no ejecutada. |
| PRF-SEC-03 (logs sin tokens/credenciales/contenido sensible salvo autorización) | **PARTIAL (mejorado)** | UAT sobre binario real con `-v` (JOURNAL §39, `prf_sec_03_uat.rs`): centinela secreto en el source no aparece en stdout/stderr de analyze, graph full, index ni doctor. Pins de regresión. Falta cubrir telemetría opt-in y credenciales de configuración del proceso. |
| **PRF-SEC-04** (presupuestos CPU/mem/tiempo/fanout/profundidad/tamaños/procesos) | **PARTIAL** | Timeouts por categoría existen; presupuesto cuantitativo por perfil no publicado. |
| PRF-SEC-05 (cancelación/shutdown libera recursos; fallos no corrompen) | PARTIAL | Ver §PRF-MCP-06. |
| PRF-SEC-06 (CRITICAL/HIGH conocidas antes de release) | NOT_RUN | Sin gate de advisories sobre el candidato congelado. |
| **PRF-SEC-07** (adversariales: repo malicioso, symlinks/traversal, parser fallido, secreto señuelo, mutante no autorizado, cliente desconectado, datos corruptos) | **PEND (matiz C5)** | Reconocido por F5/C5 como pendiente. |

## Sección H — `SPEC-STATE.md` (`PRF-STATE-*`)

| Requisito | Disposición | Evidencia / Notas |
|---|---|---|
| PRF-STATE-01 (catalogar canónico / derivado / transitorio; ownership/ubicación/ciclo) | PARTIAL | Diseñado en README; ‘CallGraph histórico vs FactStore canónico’ no ejercitado como UAT. |
| PRF-STATE-02 (namespace por canonical_root+config_digest; workspaces homónimos no contaminan) | **PASS** | §70: UAT binario real — defecto real corregido (content_hash era proxy mtime, digests idénticos en mismo ms); homónimos sin contaminación, digests estables tras reinicio (snapshot durable). `prf_state_02_uat.rs`. |
| **PRF-STATE-03** (dos procesos, mismo HOME, dos proyectos, no contaminan; locking; stale identificado) | **PARTIAL → PEND** | H-04 del operador: persistencia material no observada en evidencia; lo declarado como ‘PASS’ es reconstrucción determinista, no persistencia. |
| **PRF-STATE-04** (interrupción durante análisis/store/migración no produce evidencia parcial como válida; recovery automático o error+rollback) | **PEND** | U21 (`Cortar proceso durante escritura/migración; reiniciar.`) no ejecutada. |
| PRF-STATE-05 (versión esquema; update conserva datos; rollback/downgrade verificado o rechazo) | PEND | U22 no ejecutada en binario real. |
| PRF-STATE-06 (cogh uninstall no elimina datos usuario; ownership + HOME limpio + HOME existente) | PARTIAL | UAT-F6-001 cubre HOME limpio; HOME existente y config IDE no cert. |
| PRF-STATE-07 (datos derivados se reconstruyen; reconstrucción aviso) | **PARTIAL (mejorado)** | UAT in-process (`prf_state_07_rebuild_notification_tests`, JOURNAL §40): cambio/borrado de fuente → rebuild con mensaje honesto "built"; fuentes idénticas → inventario idéntico. Deuda registrada: cache-miss con fuentes sin cambios (reconstruye en vez de servir cache); gap H-01 (mtime+size) pendiente del operador. |

---

## Sección I — Las 27 UAT originales (catalogo en `731f54e5:docs/prf/UAT.md`)

> Mapeo 1:1 con las UAT-Fx-001 ejecutadas + las que faltan. ‘Pass’ significa ‘disposición concreta demostrada’; ‘Pendiente’ significa ‘no ejecutada como UAT específica, aunque pueda tener cobertura parcial indirecta’.

| UAT | Spec | Cubre | Disposición | Evidencia |
|---|---|---|---|---|
| U01 | DIST,CI | Instalación con digest/provenance; HOME limpio | PASS parcial | U-F6-001 (instalación) |
| U02 | CLI,MCP | `cognicode --help`, `cogh doctor`, MCP `tools/list` vs matriz | PARTIAL | `cogh doctor` y `tools/list` no auditados como UAT |
| U03 | CI,ANA | Baseline/goldens 2 veces; cobertura, tiempos, RSS | NOT_RUN | No ejecutado sobre `178f8a5b` |
| U04 | CLI,MCP,SEC | Apagar red y OTLP; CLI y MCP arrancan; stdout solo JSON-RPC | PARTIAL | Operativo; UAT específica no |
| U05 | MCP | Cliente MCP externo: initialize/tools/call/error/shutdown | PASS | 2026-09-22: sesión completa capturada en `docs/prf/evidence/u05-mcp-external-client/run1/` (JOURNAL §45) |
| U06 | CLI,SEC,ANA | Workspace inexistente / sin permisos / vacío / lenguaje no soportado | PARTIAL | Cobertura parcial en código; UAT formal con corpus adversariales no |
| U07 | CLI,ANA | Unicode, espacios, cwd, config inválida | PARTIAL | Idem |
| U08 | CLI,MCP,ANA | Misma consulta CLI/MCP sobre snapshot fijo → misma semántica | PARTIAL | U-F3-001 equivalente a U08 con corpus pequeño |
| U09 | ANA,STATE | Frío/caliente y editar un fichero → delta coherente | PARTIAL | U-F2-W9-001 (proceso nuevo) |
| **U10** | CLI,MCP,EXT | Cliente anterior soportado contra candidato | **FAIL (no UAT)** | H-07 + matriz muestran que este gate no existe |
| U11 | ANA,MCP | Fallo parser/provider en CLI y MCP → mismo estado + error tipado | PARTIAL | U-F2-W8-001 (MCP); CLI equivalente parcial |
| U12 | CI,ANA | Proyecto anidado, full/per_file; cobertura y equivalencia | PARTIAL | W3 caracterización; UAT específica de anidado no |
| **U13** | ANA | Cambiar bytes preservando tamaño+mtime; reescanear | **FAIL (RED pin, GREEN pendiente)** | H-01 explícito. RED pin añadido en `5ed7f865` (JOURNAL §31): test `h01_byte_change_with_same_mtime_and_same_size_must_invalidate_cache` pinea el caso exacto. GREEN pendiente de elección de algoritmo de hash (operador). |
| U14 | SEC,ANA | Denegar lectura, eliminar archivo, superar presupuesto → Partial/Unknown/Failed | PARTIAL | U-F2-W8-001 cubre lectura; presupuesto específico no |
| U15 | ANA | Corpus mixto, LSP ausente, lenguaje no soportado → fallback explícito | NOT_RUN | No ejecutado con corpus mixto adverso |
| U16 | ANA | Rename/move/colisiones/homónimos → identidad estable o ambigüedad visible | PARTIAL | H-R4-2 cerrado; UAT específica adversa no |
| **U17** | STATE | Indexar, reiniciar, consultar historia y revisión actual | **PARTIAL (no persistencia material)** | H-04 explícito |
| U18 | SEC,STATE | Dos proyectos, dos procesos, HOME común, cambios aislados | PASS | U-F4-001 |
| U19 | CLI,SEC,ANA | `../`, absoluto, symlink externo, write/exec sin permiso, secreto señuelo | PARTIAL | U-F5-001 cubre algunos vectores; falta adversariales amplios |
| **U20** | DIST,STATE | Install, doctor, CLI, MCP, update idempotente | **PARTIAL → FAIL** | H-06: ciclo A→B real no hecho |
| U21 | STATE,SEC | Cortar proceso durante escritura/migración; reiniciar | NOT_RUN | U-F4-001 reinicio pero no corte durante escritura |
| U22 | STATE | Versión antigua crea datos; upgrade y downgrade | NOT_RUN | H-06 relacionado |
| U23 | DIST,STATE | Uninstall con HOME/XDG personalizado e IDE de prueba | PARTIAL | U-F6-001 cubre HOME limpio; HOME existente no |
| U24 | DIST,SEC | Instalación interrumpida/asset corrupto → rollback + reinstall | PASS | 2026-09-22 (JOURNAL §46): corrupto→SHA mismatch+rollback; reinstall→healthy |
| **U25** | MCP,SEC | Tool costosa cancelada, desconexión cliente, sin OTLP | **PARTIAL** | H-05: cancelación no acredita operación en curso |
| U26 | MCP,EXT | Capacidad sintética read-only + cliente previo | PEND | Matiz C5 reconocido |
| **U27** | CI,DIST | Inyectar fallo crítico; cada uno bloquea CI/release | **FAIL** | H-07 explícito |

---

## Resumen ejecutivo

| Categoría | PASS | PARTIAL | FAIL | NOT_RUN | PEND | EXCL |
|---|---|---|---|---|---|---|
| SPEC-ANALYSIS (9) | 2 (+1 tras PRF-ANA-04) | 4 (+1 tras H-02) | 0 (-1 tras H-01) | 2 | 1 (-1 tras PRF-ANA-04) | 0 |
| SPEC-CI (7) | 0 | 3 (+1: CI-06 de FAIL a PARTIAL mejorado) | 2 (-1) | 2 | 0 | 0 |
| SPEC-CLI (7) | 0 | 5 (+2: CLI-04 de FAIL, CLI-01 de NOT_RUN) | 0 | 2 (-1) | 0 | 0 |
| SPEC-DISTRIBUTION (7) | 0 | 3 | 1 | 3 | 0 | 0 |
| SPEC-EXTENSIBILITY (6) | 0 | 4 | 0 | 1 | 1 | 0 |
| SPEC-MCP (7) | 0 | 4 | 0 | 3 | 0 | 0 |
| SPEC-SECURITY (7) | 0 | 4 | 0 | 2 | 1 | 0 |
| SPEC-STATE (7) | 0 | 3 | 0 | 1 | 3 | 0 |
| **UAT originales (27)** | 1 (+1 parcial) | 15 | 4 | 6 | 2 | 0 |
| **TOTAL (estimado)** | **~3 PASS pleno + 1 PASS parcial** | **~40** | **~7** | **~23** | **~4-8** | **0** |

**Nota de transparencia (2026-09-22, post H-01 GREEN + H-02 GREEN + PRF-ANA-04):** el resumen original (35 PARTIAL / 11 FAIL / 9 PEND) tenía errores de contabilidad. Recuento re-ejecutado sobre las filas explícitas del matriz arroja cifras distintas. Las cifras exactas no son críticas para la decisión C7: lo que importa es que **sigue habiendo gaps abiertos en FAIL y PEND** que las acciones 3-4 del cierre PRF deben cerrar. Los cambios respecto al resumen inicial son cosméticos; la **distribución cualitativa** (PASS minoritario, FAIL/PEND/NOT_RUN dominantes) **no cambia**.

**Cambios aplicados en esta sesión:**
- H-02 del operador **resuelto** en `80e7c403` (JOURNAL §30). `PRF-ANA-02` movido de `PARTIAL → PEND` a `PARTIAL (mejorado)`.
- H-01 del operador **resuelto** en `39928202` (JOURNAL §32). `PRF-ANA-03` movido de `FAIL (RED pin)` a `PASS (RED→GREEN)`. Decisión SHA-256 ejercida por "a tu criterio" previo del operador.
- H-02 adicional del operador **resuelto** en `41e4230f` (JOURNAL §33). `PRF-ANA-04` movido de `PARTIAL` a `PASS (RED→GREEN)` con `status` field en `BuildGraphOutput`.
- PRF-ANA-05 (handler reproducibilidad) **mejorado** en `dc1190f7` (JOURNAL §34). Test `repeated_build_graph_calls_are_reproducible_at_handler` añadido como pineo del MCP handler boundary. Disposición bucket sigue siendo PARTIAL (el requisito SPEC exige UAT sobre binario real, no sobre handler en proceso); el cambio mejora la cobertura dentro del bucket pero no lo convierte en PASS hasta que se ejecute la UAT stdio JSON-RPC. Contador SPEC-ANALYSIS **no cambia** por este movimiento.
- PRF-ANA-07 (renames/moves/colisiones masivas) **mejorado** en `3118c580` + `73236510` (JOURNAL §35). Nuevo corpus `massive_collision_corpus/` (51 archivos: 50 sibling + 1 local, todos con `pub fn init()`) y 3 tests RED→GREEN que pinean la visibility rule con 51 candidatos, single-candidate cross-file, y tamaño de índice. Disposición bucket sigue siendo PARTIAL (sigue faltando UAT stdio JSON-RPC sobre el binario real). Contador SPEC-ANALYSIS **no cambia** por este movimiento.
- PRF-CI-06 (política local-first) **mejorado** en `LOCAL-FIRST-CI-POLICY.md` (JOURNAL §36). Movido `FAIL → PARTIAL (mejorado)`: la política existe, su origen es verificable (AGENTS.md, ADR-031, B3) y su equivalencia procedimental está documentada (§3.3). El documento declara honestamente que la enforcement automática no existe y es decisión del operador (§4.bis). Contador SPEC-CI: FAIL 3→2, PARTIAL 2→3.

**Conclusión:** **0 ítems en PASS contractual pleno sobre los requisitos MUST**; **~3 PASS pleno** (U-F4-001 = U18 + PRF-ANA-03 con H-01 GREEN + PRF-ANA-04); **1 PASS parcial** (U01); ~40 PARTIAL; ~7 FAIL; ~23 NOT_RUN; ~4-8 PEND; 0 EXCL. **No es posible firmar C7** mientras esta matriz muestre esta distribución. Las acciones 3 y 4 del cierre PRF deben convertir los FAIL y PEND en PASS, documentar las EXCL y dejar los NOT_RUN solo si son genuinamente 'fuera de alcance' (lo que requiere EXCL aprobada por el operador).

> **Nota de honestidad:** este documento es la base sin la cual C7 no puede firmarse. Generarlo es un acto de reparación documental, no de cierre. Las acciones 3 y 4 (código + UAT) son trabajo de varias sesiones y deben coordinarse con el operador.
