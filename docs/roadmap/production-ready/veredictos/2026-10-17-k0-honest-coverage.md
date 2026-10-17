# K0 — Veredicto de cobertura honesta (línea Kotlin)

**Fecha:** 2026-10-17
**SHA candidato:** `47185e77c39c1fe651cce4ee51fdcf77cf21d05a` (`k0/honest-coverage`)
**HEAD post-merge (esperado):** el SHA que el merge-gate apruebe + squash-merge de PR #377
**Estado del gate:** en curso al cierre de este veredicto (no se publica aquí).
**Unidad de trabajo:** K0 (línea Kotlin subordinada a CU-2, autoridad
[`docs/roadmap/consumer-contracts/ROADMAP.md`](../../consumer-contracts/ROADMAP.md) §Línea Kotlin K0–K3).

## Goal

La línea Kotlin documentada en
`docs/roadmap/consumer-contracts/kotlin/KOTLIN-LINE-K0-K3.md` define K0 como
**bugfix de fiabilidad observable**: el walk del grafo de CogniCode debe
registrar honestamente sus huecos (no simular cobertura verde cuando hay
extensiones anunciadas que ningún parser ejecuta).

Tres defectos latentes caracterizados en `k0/honest-coverage` antes del fix:

1. **WalkClassifier disperso.** `strategy.rs` declaraba el subconjunto
   `"rs" | "py" | "js" | "ts"` en tres sitios distintos (legacy walk,
   `build_full_graph`, `build_full_graph_report`) sin autoridad común.
2. **`Language::from_extension` ignorado.** El registro existe
   (`infrastructure::parser::language_config::LanguageConfig`, ~25 lenguajes)
   pero los walks preguntaban al `match` literal en línea. Una extensión
   registrada como lenguaje podía caer en `SkipReason::UnsupportedExtension`
   en una ruta y ser analizada en otra.
3. **`kotlin` en `ALL_TREE_SITTER_LANGS`.** 22 entradas en
   `interface/mcp/capabilities.rs`. Sin gramática cableada ni entrada en
   `Language`. CapabilityAdvertised ≠ CapabilityVerified.

## Cambio aplicado (commit `47185e77`)

- `per_file_graph::classify_walk_extension(Option<&str>) -> WalkClassification`
  único, autoridad única. Tres variantes:
  - `Analyze` → el lenguaje está registrado y se procesa.
  - `KnownUnsupportedGap` → el lenguaje es una brecha anunciada
    (`kt`, `kts` hoy; puente hasta K1).
  - `LegitExclusion` → no es código (extensión no-lenguaje o filtro del caller).
- `KNOWN_UNSUPPORTED_EXTENSIONS: &[&str] = &["kt", "kts"]` explícito. Se
  cuenta en `WalkClassification::KnownUnsupportedGap`, **no** en
  `SkipReason::UnsupportedExtension` (que era antes el sumidero silencioso).
- `interface/mcp/capabilities.rs`: `kotlin` retirado de `ALL_TREE_SITTER_LANGS`
  (22 → 21 entradas). Sincronización: la autoridad de qué extensiones son
  lenguaje vive en `Language::from_extension`; lo que la herramienta declara
  lo lee de ahí.
- `strategy.rs`: los tres sitios que preguntaban al `match` literal ahora
  invocan `classify_walk_extension`. `analysis_service.rs`: la lista
  hardcodeada `["rs","py","js","ts","go","java","c","cpp","h"]` reemplazada
  por `Language::from_extension(path.extension()).is_some()`.

## Tests RED→GREEN (TDD)

`crates/cognicode-core/src/infrastructure/graph/strategy.rs::k0_honest_coverage_tests`:

| Test | Modo | Pino |
|---|---|---|
| `registered_source_symbols_reach_the_graph` | RED→GREEN | Un `.rs` registrado llega al grafo (`BuildStatus::Complete`). |
| `complete_requires_no_unprocessed_registered_sources` | RED→GREEN | `Complete` exige cero fuentes registradas sin procesar. |
| `known_unsupported_source_is_an_honest_gap` | RED→GREEN | `kt`/`kts` se cuentan en `KnownUnsupportedGap`, no en `SkipReason::UnsupportedExtension`. |

## Evidencia de invocación real (UAT-K0 / AAT-K0)

`bats` efímero (test harness real, no en repo), dos canales ejecutados con
binarios `0.101.21` (release):

| Canal | Comando | Resultado |
|---|---|---|
| Hot-paths | `cognicode --format json graph per-file <fuente>` | exit 0, símbolos presentes |
| Graph full | `cognicode --format json graph full <dir>` | exit 0, status `Complete` |

No quedan archivos temporales tras el cierre.

## Lo que K0 **no** toca (delimitación)

- **K1 tree-sitter-kotlin** (siguiente bloque subordinado) — sigue bloqueado
  hasta que se cablee la gramática y entre en `Language::from_extension`.
- **PRF f6.w1 `cogh generate` (defecto latente separado).** Los tests
  `prf_f6_w1_clean_round_trip_generates_and_verifies` y
  `prf_f6_w1_tampered_payload_fails_verify` en
  `crates/cognicode-cli/tests/prf_f6_w1_release_coherence.rs` asumen
  `cogh generate`. El binario actual no expone ese subcomando
  (`cogh --help` solo lista install/uninstall/list/current/latest/update/
  reshim/rollback/doctor/where/init/setup/plugin/skill/skills/ide/version/help).
  **No son regresiones de K0**: el K0 toca 4 archivos en graph layer
  (`strategy.rs`, `per_file_graph.rs`, `capabilities.rs`,
  `analysis_service.rs`) — ninguno está en la fábrica de release. Además,
  **no se ejecutan en el merge-gate**: `merge-gate.pipeline.kts` líneas
  604–656 enumeran los `--test` por nombre, y `prf_f6_w1_release_coherence`
  no está. La enumeración explícita es la garantía contractual de cobertura
  en `cli_gate_coverage_contract`. Defecto a registrar y resolver en WU
  separada.

## Batería

`scripts/test-full.sh` ejecutada tres veces por la sesión — los dos
primeros con error de infraestructura local (binarios release no
construidos; verificado: `~/.cargo/config.toml` no es nuestra, los targets
viven en `/var/home/rubentxu/cargo-targets/`). El tercero, **#3 (PID
3898710)**, se ejecutó con `COGNICODE_CLI_BIN`,
`COGNICODE_MCP_BIN`, `COGNICODE_RELEASE_BIN` exportados apuntando a los
tres binarios recién construidos.

Resultado:

- `fmt --check`: exit 0.
- `clippy -D warnings`: exit 0.
- `cargo test --workspace`: **9 passed; 2 failed** en
  `cognicode-cli::prf_f6_w1_release_coherence` (las dos referenciadas
  arriba).
- `cargo test --workspace --doc`: exit 0.
- `known_failures (core/lib)`: 0 entradas — coincide con baseline.

**Interpretación**: rc=1 del wrapper se debe **exclusivamente** al defecto
pre-existente PRF f6.w1, no al delta de K0. Pre-existencia confirmada por:

- Los 4 archivos del delta K0 no tocan superficie CLI/release.
- `merge-gate.pipeline.kts` no enumera ese test en la lane (los
  enumerados son 33; los snapshots no se solapan).
- `git log` muestra `prf_f6_w1_release_coherence.rs` introducido en
  `c28f2143` ("V32.5"), con su última edición muy anterior al K0 y sin
  relación con la cobertura honesta.

## Provenance (trazabilidad)

- **Tests RED→GREEN**: `git rev-parse HEAD~1` versus `HEAD` del branch; las
  tres pruebas de `k0_honest_coverage_tests` se añadieron en `47185e77`.
- **Symmetry check**: `test_stable_tool_names_are_real` +
  `test_capabilities_matrix_for_stable_tools` +
  `test_capabilities_declarations_match_pineo` (capabilities.rs líneas
  393, 359, 469) siguen pasando — el delta no rompió la simetría de
  capabilities.
- **UAT efímero**: borrado (`/tmp/uat-k0.sh` eliminado); evidencia
  conservada en este veredicto.

## Estado al cierre

- Branch `k0/honest-coverage` publicada (4 commits pre-K0 + 1 K0).
- PR #377 abierto contra `main`.
- Merge-gate en curso al cierre de este veredicto.

Si el merge-gate aprueba: squash-merge → bump PATCH a `0.101.22` (K0 es
bugfix de fiabilidad, no añade superficie) → tag anotado →
release-candidate → release. Pendiente del operador como `merge-gate=success`
llegue a GitHub.

## Paquete `docs/cognicode-kotlin-roadmap/` (resolución honesta)

Material upstream de la línea Kotlin, depositado en el árbol de trabajo
(NO tracked: bajo `docs/*` en `.gitignore:153` + bajo `docs/roadmap/*` en
`.gitignore:200`, ambos decididos por el operador 2026-06-24). Contiene
5 SPECs (Kotlin API, cobertura, relaciones, símbolos, providers), 4 ADRs
(ADR-K001..K004, todos `PROPOSED`), MILESTONES, AGENT-HANDOFF,
AGENT-TEAMS, GLOSSARY, RELEASE-RUNBOOK, TEST-STRATEGY, TRACEABILITY, UAT,
`tools/apply.py` (idempotente, dry-run por defecto).

**Decisión aplicada en este veredicto**:

- **NO** se ejecuta `python3 tools/apply.py --apply`: crearía 17 archivos
  huérfanos bajo `docs/roadmap/kotlin-evolution/` (gitignored) y un único
  puntero en `docs/ROADMAP.md` (tracked) apuntando a esos huérfanos.
  Resultado: el puntero sobrevive, el contenido no. Eso es un veredicto
  verde falso — readers de GitHub ven el puntero y luego el 404.
- La **línea K0–K3 canónica y tracked** vive ya en
  [`docs/roadmap/consumer-contracts/kotlin/KOTLIN-LINE-K0-K3.md`](../../consumer-contracts/kotlin/KOTLIN-LINE-K0-K3.md)
  (`git ls-files` muestra el path). Ese documento, 22 KB, integra el
  upstream del paquete en el lenguaje del roadmap activo.
- Si en el futuro el operador decide relajar la política `.gitignore` para
  abrir un subdirectorio `kotlin-evolution/` bajo `docs/roadmap/`,
  `tools/apply.py` queda idempotente y preparado para correr `--apply` sin
  sobrescribir nada distinto.

**Hechos a reconciliar en una futura pasada del operador**:

- El paquete declara `baseline_main_sha = 88f6d02a` y
  `last_release_verified = v0.101.17`. La release vigente al cierre de
  este veredicto es `v0.101.21` (squash `755e97a6`). El delta `0.101.18`
  → `0.101.21` (CU-1 + CU-2) NO está capturado en el `MILESTONES.md` del
  paquete upstream. La línea K0–K3 tracked en `consumer-contracts/kotlin/`
  sí lo refleja en su `REFERENCE-BASELINE` (v0.101.21 sobre `755e97a6`).
- El paquete menciona una ruta `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`
  ausente en el árbol. **No** se crea silenciosamente: el documento
  vigente al respecto es `docs/roadmap/ROADMAP.md` §10–11 (CLI/MCP y
  Consumer-First).

## Próximo trabajo ejecutable (post-K0)

1. **K0.merge**: merge-gate, squash-merge, bump `0.101.22`, tag,
   release-candidate, release.
2. **Latente PRF f6.w1**: o bien restaurar `cogh generate` y volver a
   pinear el release factory como unitario, o bien retirar el test del
   árbol (no del gate) con su periodo de gracia vía `#[ignore]` y un
   ticket. Decisión de alcance: tooling o test-harness.
3. **K1 cableado real**: añadir `tree-sitter-kotlin`, mapear a
   `Language::Kotlin`, y reemplazar `KnownUnsupportedGap` por
   `Analyze` en `.kt`/`.kts`. Es la única vía de cierre honesto del
   puente de K0.
