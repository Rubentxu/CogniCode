# Action Register — PRODUCT-1.0

| ID | Acción | Prioridad | Dependencia | Resultado / Acceptance |
|---|---|---:|---|---|
| A-001 | Crear track `community-productization` y puntero en ROADMAP | P0 | — | una sola autoridad de agenda |
| A-002 | Reconciliar versión release/main/README/repo metadata | P0 | A-001 | cero claims contradictorias |
| A-003 | Product manifest generado | P0 | A-002 | **CLOSED 2026-09-27** — schema `cognicode.product/v1`, `source_commit`, generator determinista, tests 5/5, ciclo `p-c1fac1fea05615c6/cp0-product-manifest`, commit `96d06854`; siguientes A-004/A-005 |
| A-004 | Tool/catalog generator | P0 | A-003 | **CLOSED 2026-09-27** — `cognicode.tools/v1`, captura runtime `tools/list` de 73 tools, generator determinista, schema, tests 4/4, ciclo `p-c1fac1fea05615c6/cp0-tool-catalog`, commit `85222f67`; A-005 queda siguiente |
| A-005 | Language/platform support matrix | P0 | A-003 | **CLOSED 2026-09-27** — `product/languages.json` + `product/platforms.json`, schema `languages.v1`, 30 lenguajes (18 supported / 12 experimental) y 3 plataformas; clasificaciones derivadas de la cobertura de aceptación real (m06 + m10) y de los lanes de release/CI, no declaradas; commit `73235889` |
| A-006 | Definir profiles core/reviewer/developer/experimental | P0 | A-004 | **CLOSED 2026-09-27** — `product/profiles.json` (`cognicode.profiles/v1`) pasa a ser la única fuente pública, reemplazando la constante `PUBLIC_PROFILES` hard-coded del manifest; profiles derivados de `release_contract.rs` + 2 declarados a mano (developer, experimental, `install:false`); commit `f24609f0` |
| A-007 | Resolver licencia repo y añadir ficheros | P0 | decisión operador | **CLOSED 2026-09-27** — decisión de licencia ya tomada en M0.9 (`f0708d4b`); CP1 no la re-decidió, la publicó: `LICENSE-MIT` + `LICENSE-APACHE` completos, `LICENSE` raíz, license-of-record en `[workspace.package]`, ADR-053; commit `95595af9` |
| A-008 | SECURITY/CONTRIBUTING/SUPPORT/CoC/templates | P0 | A-007 parcial | **CLOSED 2026-09-27** — `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `SUPPORT.md`, 3 issue forms y PR template, escritos desde los contratos reales; READMEs EN/ES reconciliados a cifras canónicas (73 tools, 30 lenguajes) citando los artefactos de origen; suite verificada por mutación (3 contratos rotos deliberadamente, 3 tests fallaron); commit `95595af9`. **Gate OSS incompleto**: CP1.7 Discussions sigue `operator-gated` (CP1-DEBT-02) |
| A-009 | Audit authority + agent-safe profile | P0 | A-006 | **CLOSED 2026-09-27** — el enforcement read-only ya existía pero no estaba conectado a nada: `HandlerContextBuilder::with_read_only` no tenía caller, así que el perfil público `reviewer` podía escribir igual que `core` pese a declarar `mutating: false`. `PROFILE_POSTURES` en el core pasa a ser la fuente única de verdad y `generate_profiles.py` la **deriva** del fuente Rust en vez de duplicarla, de modo que el contrato publicado no puede contradecir el runtime. 8 tests nuevos; uno habla JSON-RPC con el binario real y prueba que `write_file` no se anuncia, se rechaza con `read_only_mode` y no deja nada en disco. Detección por mutación en 4 direcciones. PR #300, commit `b1d3e773`, `merge-gate` verde |
| A-010 | MCP conformance baseline | P0 | A-009 | **CLOSED 2026-09-27** — la autoridad que publica `tools.json` queda verificada contra la que impone el runtime, con detección probada en 5 mutaciones (tool mutante declarada `read`, tool mutante fuera del catálogo, `runtime_tool_count` mentiroso, tool añadida a `MUTATING_TOOLS`, autoridad cambiada en `cognicode_meta`). 2 tests black-box contra el binario real demuestran que bajo `--read-only` `tools/list` es exactamente el conjunto read publicado y que toda tool mutante se rechaza con `read_only_mode`: es lo que convierte en medido el supuesto de que `MUTATING_TOOLS` es exhaustivo, del que dependía toda la garantía read-only. 10 tests. PR #302, commit `57c21f87` |
| A-011 | Schema snapshots stable profile | P0 | A-006 | **CLOSED 2026-09-27** — el repo publica 5 schemas y 5 documentos, y **ningún test leía un schema**: eran comentarios con sintaxis JSON. La primera ejecución del gate falló con 5 violaciones reales en `product-manifest.json` que llevaban tiempo en `main`, introducidas por A-006 al mover `profiles` fuera de `artifacts` y ampliar cada profile con `install`/`stability`. Se corrigió el schema, no el documento: el documento es la verdad y A-009 acaba de endurecer precisamente el campo `mutating` que el schema obsoleto se negaba a describir. El gate cubre los 5 pares y exige que todo `product/*.json` esté registrado y que cada schema lleve `required` + `additionalProperties: false`. 41 checks de producto (antes 36), paso nuevo en el job `check` del que depende `merge-gate`. PR #303, commit `11eaa936` |
| A-012 | Structured output stable reviewer tools | P1 | A-011 | **EN CURSO** — medido: 0 de 73 tools publican `output_schema`, incluidas las 57 del perfil `reviewer` (todas estables). El runtime serializa a `String` y devuelve `Content::text`, así que el consumidor recibe JSON dentro de texto sin esquema. El trabajo no es inventar contratos: ya existen 18 tipos de salida Rust definidos y serializados (`GraphAnalyzeOutput`, `IacQueryOutput`, `GetTypeRefsOutput`, `ReviewPrOutput`, `BuildGraphOutput`, `ExportCallflowOutput`, …). Falta (a) anunciar `outputSchema` en `tools/list`, que la spec MCP soporta y el código no usa; (b) derivar el JSON Schema de esos 18 tipos, para lo que hace falta `schemars`, hoy ausente; y (c) un gate que verifique que el `outputSchema` anunciado casa con la salida real, porque un schema publicado que el runtime no cumple es peor que ninguno. WorkItem `0a6ace31` |
| A-013 | Black-box lifecycle UAT | P0 | A-009 | artifacts: startup→shutdown PASS |
| A-014 | `cognicode capabilities --json` | P1 | A-003 | capability discovery machine-readable |
| A-015 | Simplificar onboarding `cogh setup` | P1 | A-014 | install→doctor→MCP en happy path |
| A-016 | Crear `Rubentxu/cognicode-site` | P0 | — | repo + baseline Astro/Starlight |
| A-017 | Landing hero/IA + 5-minute quickstart | P0 | A-002,A-016 | usuario llega a first useful call |
| A-018 | Generated reference ingestion | P0 | A-003,A-016 | site falla si manifest/schema inválido |
| A-019 | Cloudflare Pages project | P0 | A-016 | preview deployment funcional |
| A-020 | Asociar `cognicode.rubentxu.dev` | P0 | A-019 | HTTPS healthy |
| A-021 | CNAME Squarespace manual | P0 | A-020 | resolución estable documentada |
| A-022 | PipelineK site checks/deploy | P1 | A-019 | production deploy + receipt + rollback |
| A-023 | mise channel | P0 | release assets coherentes | fresh install PASS |
| A-024 | MCPB | P0 | A-009,A-013 | bundle instalado sin toolchain Rust |
| A-025 | MCP Registry | P0 | A-024 | discoverable entry publicada |
| A-026 | OCI/GHCR | P1 | A-013 | non-root/read-only default smoke |
| A-027 | macOS arm64 build/certification | P0 GA | release matrix | clean-host PASS |
| A-028 | Windows x64 build/certification | P0 GA | release matrix | clean-host PASS o explicitly deferred |
| A-029 | Aqua | P2 | stable release contract | install PASS |
| A-030 | Homebrew tap | P2 | A-027 | formula install/upgrade PASS |
| A-031 | Nix flake | P2 | stable release | nix profile smoke |
| A-032 | asdf plugin | P3 | adoption signal | compatibility channel |
| A-033 | skill `cognicode` | P0 | A-014 | eval suite PASS |
| A-034 | skill `cognicode-agent-hardness` | P0 | A-009,A-014 | eval suite PASS |
| A-035 | skill `cognicode-pr-review` | P1 | A-014 | eval suite PASS |
| A-036 | align `cognicode-quality-investigator` | P0 | A-014 | no stale tool assumptions |
| A-037 | skills.sh pack | P1 | A-033..036 | install pack smoke |
| A-038 | reproducible with/without benchmark | P1 | A-013 | methodology+raw evidence publicable |
| A-039 | 60–90 s demo | P1 | A-017,A-038 | landing-ready asset |
| A-040 | launch content drafts | P1 | Product Truth gate | READY_FOR_PUBLICATION |
| A-041 | Public Beta 0.99.x | P0 | PT+OSS+HARD+DOC+DIST core | external beta open |
| A-042 | External beta feedback round | P0 | A-041 | ≥10 diverse testers or evidence equivalent |
| A-043 | 1.0 admission | P0 | A-042 | GA checklist 100% |
| A-044 | Coordinated 1.0 launch | P0 | A-043 + operator | site/registry/skills/blog/social synced |
| A-045 | Optional DNS authority migration to Cloudflare | P3 | separate approval | IaC DNS, no launch dependency |

## Large-block execution

Para evitar micromanagement, agrupar en bloques:

1. **BLOCK-A Truth & OSS**: A-001..011.
2. **BLOCK-B Product DX**: A-012..018.
3. **BLOCK-C Site & Domain**: A-019..022.
4. **BLOCK-D Distribution**: A-023..032.
5. **BLOCK-E Agent Adoption**: A-033..040.
6. **BLOCK-F Beta → GA**: A-041..044.
7. **BLOCK-G DNS IaC**: A-045 opcional.

## Deuda de CP1 — estado real tras la sesión 2026-09-27

| ID | Estado | Resolución |
|---|---|---|
| CP1-DEBT-01 | **CLOSED** | `generate_profiles.py --check` y `generate_support_matrix.py --check` fallaban por diseño, no por deriva real: los generadores estampan `source_commit` desde `git rev-parse HEAD`, así que comparar el documento entero lo invalidaba en cada commit. `--check` ahora compara contenido estrictamente y valida la procedencia por forma, con la semántica compartida en `scripts/product/check_semantics.py`. Verificado por mutación en cuatro casos. Commit `ff60df36`. |
| CP1-DEBT-02 | **PARTIAL** | Habilitar Discussions **era automatizable**: `UpdateRepositoryInput.hasDiscussionsEnabled` existe en el schema GraphQL y se aplicó, verificado por `GET /repos/{owner}/{repo}` (`has_discussions: false` → `true`). Crear las **categorías** sí es manual: `createDiscussionCategory` no existe en `Mutation` (`undefinedField`) y el endpoint REST devuelve 404. Quedan cuatro categorías pendientes de alta manual; conjunto y propósito ya especificados en ADR-053. Commit `905d8b6b`. |
| CP1-DEBT-03 | **RECLASSIFIED** | No es deuda técnica sino una decisión explícita del maintainer: `.gitignore` declara `docs/` local-only con protocolo documentado de `git add -f`. No se modifica. El riesgo real —un documento público citando una ruta que solo existe en su disco— se cierra en el lado correcto: `test_community_documents_cite_real_repository_paths` pasó a exigir que **todas** las rutas citadas existan, no solo una. Commit `00068420`. |

## Deuda de CP2 — estado real tras la sesión 2026-09-27

| ID | Estado | Resolución |
|---|---|---|
| CP2-DEBT-04 | **CLOSED** | `generate_product_manifest.py --check` comparaba el documento entero contra el render, incluida la marca `source_commit`, que por defecto es `git rev-parse HEAD`. Eso hace el check insatisfacible en cualquier commit más nuevo que el que produjo el fichero: la única diferencia es la procedencia. Es el mismo defecto que CP1-DEBT-01 resolvió para `generate_profiles.py` y `generate_support_matrix.py` mediante `check_semantics.py`, pero el generador del manifest nunca adoptó el módulo, así que la regla tenía dos definiciones y la que quedaba viva era la incorrecta. Adoptada la semántica compartida y fijados tres tests. Descubierto al re-verificar la suite completa de producto tras el merge de A-009, no por el gate: los tests existentes pasaban `--source-commit` explícito, y la auto-invalidación solo aparece cuando el generador cae a `git rev-parse HEAD`. Commit `acc02068`, PR #301. |
| CP2-DEBT-05 | **CLOSED** | Dos premisas registradas en el WorkItem de A-010 eran falsas y se corrigieron midiendo en vez de anotando. (a) `requires_persistence` **no** era una constante: 4 de 73 tools la declaran `true` (`detect_drift`, `generate_contract`, `iac_query`, `reparse_on_edit`) y 44 de 73 declaran `graph`. (b) `requirements_status: not_declared` en las 73 tools **no** era un hueco: significa que `cache` y `network` son `null` porque el runtime no los declara, y un status que dijera `declared` con dos de cuatro requisitos en `null` sería la mentira. Ambas quedan fijadas por test. Lección: la Lesson 88 de M0.11 (un conteo heredado nunca es evidencia) se aplicó a un hallazgo propio, y el turno siguiente lo detectó antes de construir sobre él. |
| CP2-DEBT-06 | **CLOSED** | El rechazo de una tool mutante bajo `--read-only` **no es un error JSON-RPC**, es un resultado de tool con `isError: true` y texto `read_only_mode`. El primer assert de A-010 exigía un error de transporte y falló contra un servidor que hacía bien su trabajo. El test ahora pina el contrato real. Registrado porque la suposición contraria es fácil de heredar y produce un test que pasa por el motivo equivocado si la forma de la respuesta cambia. |
 |
