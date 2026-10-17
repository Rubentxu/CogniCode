# CogniCode — TODO Kotlin integrado sobre CU (revisión de código remoto)

**Fecha de revisión:** 2026-10-17. **Repositorio:** `Rubentxu/CogniCode`.
**Baseline observada:** `main@c67cc05ec0b8fd7eb18708cab3cba09786188f0d`; release remota `v0.101.20` (8 assets listados; instalación y hashes **no ejecutados** en esta revisión). **No congelar este SHA:** volver a verificarlo al iniciar un worktree.

> **AUTORIDAD ÚNICA:** `docs/roadmap/ROADMAP.md`, sección 11, que delega el detalle del programa CU en `docs/roadmap/consumer-contracts/ROADMAP.md`. **No crear otro roadmap paralelo.** Incorporar estos TODO en el programa CU como «línea Kotlin» mediante una edición mínima. Conservar el orden y la autoridad de CU-2.5, CU-3 y CU-4; no reabrir B1–B6 ni PRF histórico. **No crear especificaciones, ADR ni referencias nuevas:** aplicar las referencias *existentes* relacionadas abajo. No marcar un ADR propuesto como aceptado sin la decisión prevista por el proyecto.

## Estado verificado, no extrapolado

- `Cargo.toml`: `version = "0.101.20"`. Publicada `v0.101.20` con 8 assets, pero CU-2.5 figura **[ ]** y CU-3/CU-4 siguen abiertos en el roadmap CU; publicar una versión no prueba que todo CU-2 esté cerrado.
- En `domain/value_objects/language.rs` falta la variante Kotlin y el reconocimiento de `.kt` y `.kts`. En `infrastructure/parser/language_config.rs` y `tree_sitter_parser.rs` no figura la gramática Kotlin; `Cargo.toml` tampoco contiene su dependencia. `product/languages.json` no anuncia Kotlin.
- `PerFileStrategy::{build_full_graph,build_full_graph_report}` filtra explícitamente solo `rs|py|js|ts`; la otra estrategia y el camino de análisis de CU-1 usan rutas diferentes. **Es un defecto transversal de cobertura**, no exclusivo de Kotlin.
- `TreeSitterParser::find_all_symbols_with_path` / `find_call_relationships` comparan un único `function_node_type`/`class_node_type`/`call_node_type` por categoría, mientras `LanguageConfig` ya define conjuntos: no añadir casos Kotlin especiales sin decidir quién es la autoridad de los nodos. El índice local desambigua por nombre y cercanía, no por firma/tipo de receptor; debe permanecer conservador.
- Existen `AnalysisOutcome` con `Complete|Partial|Unknown|Unsupported|Failed`, `ConsumerAnalysisFacade` (CU-1), el resolvedor escalonado `S0/S1/S2` y `S3/S4` **reservados**, `CallGraphProjectionPort`, catálogo MCP y pipeline real de distribución. **Reutilizarlos**.
- Drift documental medido: `product/languages.json` publica todavía `version: 0.101.17` frente a `product/product-manifest.json` `0.101.20`; `docs/roadmap/CURRENT.md` contiene párrafos heredados que describen `v0.101.16` como última publicación. El enlace `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md` citado por §10 no existe en `main`; **no inventar el fichero**. Reconciliar lo que sea necesario para las entregas, sin convertirlo en un programa de documentación.

**Riesgo principal:** un grafo sin símbolos Kotlin puede aparentar estar completo; el segundo riesgo es añadir aristas Kotlin incorrectas por coincidencia de nombres. El primer release debe solucionar la honestidad antes de aumentar la cantidad de resultados.

## TODO por bloques largos de valor — todos acaban en PR, merge remoto y release

### [ ] K0 — «No me mientas sobre qué código se ha analizado» · release compatible

**Valor observable:** un consumidor de CLI/SDK/MCP sabe qué código se ha inspeccionado y cuáles son los límites. Un proyecto con Kotlin que aún no puede parsearse responde `Unsupported/Partial` con razón, nunca «grafo completo sin Kotlin».

- [ ] Quitar listas rígidas de extensiones de `PerFileStrategy`; hacer una única clasificación de archivos basada en el registro existente (analizable / excluido deliberadamente / lenguaje sin soporte / fallido), compartida por las estrategias. No atribuir incompletitud al `README.md` excluido legítimamente; **sí** a una fuente `.kt` relevante que no se pudo analizar.
- [ ] Propagar alcance, exclusiones relevantes, razones de omisión y bases del análisis al `AnalysisOutcome` existente; preservar compatibilidad pública CLI/MCP y `N−1`. Distinguir «ningún símbolo hallado con alcance completo» de «no se inspeccionaron todas las fuentes relevantes».
- [ ] Hacer que `product/languages.json` y el manifiesto estén alineados con capacidades **realmente comprobadas**; la mera detección de extensión no equivale a soporte semántico certificado. Corregir snapshots stale que afecten el gate/release, sin reescribir historia.
- [ ] **UAT-K0 efímera:** proyecto temporal Rust + `src/Util.kt` + archivo documental; una consulta real por el canal que consume `ConsumerAnalysisFacade` devuelve fuente Kotlin omitida, pero no declara `Complete`. **AAT-K0:** archivo Kotlin ilegible o fallo de walk → parcial/fallo tipado. Usar un caso real CLI/MCP como UAT; solo añadir E2E independiente si el transporte no queda observado.
- [ ] Al cierre: conservar una prueba pequeña que impida volver al falso `Complete`; retirar el script/corpus efímero tras registrar resultado. Full, merge, RC, publicación, descarga y smoke según el procedimiento común.

**Código a revisar:** `infrastructure/graph/strategy.rs`, `graph/per_file_graph.rs`, `application/analysis_basis.rs`, `application/services/analyze.rs`, `product/languages.json`.
**Referencias reales:** `docs/roadmap/consumer-contracts/specs/SPEC-CU-001-SDK-ANALYSIS.md`, `SPEC-CU-002-CLI-MCP-PARITY.md`, `SPEC-CU-005-SNAPSHOT-INCREMENTAL.md` (mismo directorio); `docs/adr/ADR-029-callgraph-projection-port-seam.md`, `docs/adr/ADR-016-renderer-neutral-semantic-projections.md`. `docs/prf/specs/SPEC-ANALYSIS.md` es **histórico**, útil para comprobar regresiones existentes, no agenda nueva.
**Agent teams:** A (recorrido/clasificación) y B (UAT de consumidor y prueba negativa) en worktrees disjuntos; integrador posee modificaciones compartidas del catálogo y publica.

### [ ] K1 — «Encuentro símbolos Kotlin sin instalar una JVM» · release de navegación

**Valor observable:** `get_file_symbols`, búsqueda y código por símbolo reconocen funciones, clases, interfaces, propiedades, `object`, `companion`, funciones de extensión y declaraciones de un proyecto Kotlin real. `.kts` y `.gradle.kts` se clasifican con sus limitaciones, sin simular compilación de la DSL.

- [ ] Comparar gramáticas Tree-sitter Kotlin que funcionen con la versión ya fijada de Tree-sitter; elegir **una sola** dependencia Rust, auditar licencia y compatibilidad, sin incorporar Kotlin Compiler/Gradle como requisito del parser. Añadir `Language::Kotlin`, extensiones y configuración en los catálogos canónicos.
- [ ] Resolver la discrepancia entre los conjuntos `LanguageConfig::{function_types,class_types,variable_types,call_types}` y el parser que solo consume un tipo por categoría. Reutilizar el extractor genérico si ya satisface la semántica; no añadir un extractor Kotlin paralelo ni hardcodear `match Kotlin` en cada consumidor.
- [ ] Conservar identidad canónica de símbolo, package/rango/localización y extracción legible de errores. Clasificar explícitamente funciones top-level, constructores, clases anidadas, companion y propiedad; la estructura sintáctica no autoriza a afirmar destino de llamada.
- [ ] **UAT-K1 efímera:** proyecto `.kt` sin JVM donde los símbolos aparecen por un endpoint real CLI/MCP y su código/rango coincide con las fuentes. **AAT-K1:** archivo Kotlin sintácticamente inválido → no `Complete` sin advertencias. E2E adicional únicamente si el primer escenario no observa el binario distribuido.
- [ ] Al cierre: mantener un contrato/parser test corto que cubra diversidad de declaraciones y el caso negativo; retirar UAT/AAT/E2E de sesión; release y smoke del binario descargado, sin tool-name guessing (consultar `--help`, `tools/list`).

**Código:** `domain/value_objects/language.rs`, `infrastructure/parser/{language_config,tree_sitter_parser}.rs`, `Cargo.toml`, catalogación y herramientas existentes.
**Referencias reales:** `SPEC-CU-001`, `SPEC-CU-002`, `SPEC-CU-004-CONTEXT-EXPLAINABILITY.md` (rutas `docs/roadmap/consumer-contracts/specs/`); `docs/roadmap/adr/ADR-012-st04-cli-mcp-separation.md`; producto `product/languages.json`, `product/tools.json`.
**Agent teams:** A (gramática y extractor), B (fixtures/contratos de símbolos), C (probe CLI/MCP existente); integrador decide gramática/Cargo y revisa contratos, evitando editar el mismo archivo en paralelo.

### [ ] K2 — «Sé quién llama a quién sin confundir funciones» · release de relaciones

**Valor observable:** impacto y grafo reflejan llamadas Kotlin/Kotlin y Kotlin/Java solo si hay evidencia suficiente. Métodos sobrecargados, funciones de extensión, `companion object`, clases del mismo nombre en paquetes distintos y lambdas no fabrican relaciones por coincidencia textual.

- [ ] Separar observación de invocación (`caller`, expresión, localización, receptor/argumentos si son conocidos) de **resolución del destino** (`resolved`, `ambiguous`, `unresolved`). Reutilizar ADTs, `SymbolId` y `CallGraphProjectionPort`; no crear un segundo grafo ni un identificador Kotlin incompatible.
- [ ] Resolver de forma conservadora con contexto verificable de paquete, import, contenedor, firma y receptor cuando exista. Si no se puede probar el destino: retener la invocación observada como incertidumbre, **no** crear arista hacia un homónimo arbitrario. No atribuir orden temporal a coroutines/`Flow` solo por topología.
- [ ] Comprobar que el grafo `full` y `per_file` tienen semántica compatible; los huecos o límites de resolución no desaparecen en la salida de CLI/MCP ni en las proyecciones de diagramas.
- [ ] **UAT-K2 efímera:** fixture Kotlin + Java con dos `parse()` homónimos y una invocación inequívoca; debe dibujarse solo la arista respaldada. **AAT-K2:** llamada ambigua y extensión sin tipo conocido → `Ambiguous/Unknown` sin arista falsa. E2E de impacto solo si las rutas públicas no están ya probadas por la UAT.
- [ ] Al cierre: conservar la menor ley de «no aristas falsas» y una comprobación de identidad interarchivo; retirar los scripts temporales; PR, integración y release de relación observable.

**Código:** `parser/tree_sitter_parser.rs`, `graph/per_file_graph.rs`, `graph/strategy.rs`, `domain/aggregates/call_graph.rs`, `interface/symbol_identity.rs`.
**Referencias reales:** `SPEC-CU-001`, `SPEC-CU-004`, `SPEC-CU-005` (en `docs/roadmap/consumer-contracts/specs/`); `docs/adr/ADR-029-callgraph-projection-port-seam.md`, `docs/adr/ADR-016-renderer-neutral-semantic-projections.md`.
**Agent teams:** A (observaciones de llamadas y extracción), B (resolución pura/grafo), C (corpus Kotlin↔Java y consumidor de impacto); integrador congela ADTs antes de merges.

### [ ] K3 — «Kotlin semántico cuando haya proveedor; útil cuando no lo haya» · release agent-first

**Valor observable:** con un LSP Kotlin disponible, CogniCode responde referencias/definiciones más precisas; sin LSP sigue navegando con S0 y describe la pérdida de precisión. El agente descubre capacidades **por lenguaje y operación**, sabe qué preguntar después y puede distinguir un análisis parcial de uno completo.

- [ ] Conectar Kotlin al `CompositeProvider` ya existente y probar una implementación **real** del servidor LSP elegida por compatibilidad. No lanzar Gradle ni un proceso JVM como requisito para la ruta Tree-sitter; límites y readiness visibles, timeout acotado, sin falsos verdes con servidor ausente.
- [ ] Verificar conversiones de posiciones Tree-sitter (bytes) ↔ LSP (líneas/UTF-16), jerarquías y referencias; resultados S0/S1/S2 llevan la procedencia existente y no ascienden de precisión por fallback.
- [ ] Extender el catálogo por lenguaje/operación **solo lo necesario**, usando `tools/list`, `product/tools.json` y los servicios de CU-3. No introducir un segundo sistema de affordances, paginación, SDK ni backend. Reconocer `Unsupported/Degraded` y acciones realmente habilitadas.
- [ ] **UAT-K3 efímera:** mismo fixture real Kotlin/Java consultado con LSP disponible y ausente; simbolización básica en ambos, precisión declarada solo donde se verificó. **AAT-K3:** servidor colgado o capacidad no anunciada → timeout/degradación y no falso `Complete`. **E2E-K3 opcional:** agente consulta `tools/list` y navega hasta una referencia sin leer archivos brutos, si CU-3 no proporciona ya el mismo test.
- [ ] **SCIP (`S3`) es condicional, no requisito de K3:** proponerlo solo si el diferencial LSP/Tree-sitter demuestra falsos negativos relevantes y se identifica un consumidor real. `S4` compilador igualmente diferido. No generar otro hito «solo infraestructura» ni demorar esta release por ello.
- [ ] Al cierre: una prueba permanente de fallback+procedencia y una prueba de conversión UTF-16 si aportan nuevas garantías; retirar UAT/AAT/E2E de sesión; publicar e instalar artefactos exactos, verificar compatibilidad con el consumidor.

**Código:** `infrastructure/lsp/{process_manager,providers/composite,providers/lsp}.rs`, `domain/traits/code_intelligence.rs`, `application/fact_bridge/lsp_facts.rs`, interfaces MCP/CLI ya existentes.
**Referencias reales:** `SPEC-CU-002`, `SPEC-CU-004`, `SPEC-CU-006-CONSUMER-RELEASE.md` (en `docs/roadmap/consumer-contracts/specs/`); `docs/roadmap/consumer-contracts/acceptance/TEST-CHARTER.md`, `docs/adr/ADR-016-renderer-neutral-semantic-projections.md`.
**Agent teams:** A (LSP/adaptador), B (contrato de catálogo y experiencia CU-3), C (UAT reales/probe y compatibilidad); integrador gestiona `Cargo.toml`, catálogo, RC y release.

## Integración con CU y regla de prioridad

1. **No pisar CU-2.5:** terminar su release/recibo vigente y verificarlo contra la GitHub Release. K0 puede prepararse en otro worktree sin tocar `application/consumer_analysis.rs`, `domain/findings/*` o el catálogo durante la ventana de publicación. La release es serializada por un único integrador.
2. **K0→K1→K2 son dependencias estrictas.** Se pueden paralelizar investigación, fixtures y adaptadores sin inventar un `Complete`. **K3** puede preparar una investigación LSP en paralelo con K2, pero no publicar la semántica hasta disponer de identidades/cobertura correctas.
3. **CU-3 y K3:** un solo equipo dueño de navegación/affordances. Kotlin es una aplicación del contrato CU-3, no un producto UX paralelo. **CU-4:** reutilizar su identidad incremental, N−1 y release para las capacidades Kotlin.
4. El agente debe actualizar las casillas existentes del roadmap CU y el estado observado en el principal; evitar una duplicación `K DONE / CU PENDING` por el mismo comportamiento. Los nombres K0–K3 son agrupadores de trabajo, **no IDs de especificaciones ni documentos nuevos**.
5. Los ADR `ADR-CU-*` son propuestos: respetar su estado. Si un cambio requiere decisión arquitectónica no cubierta, informar `BLOCKED_DECISION`; **no inventar ADR aprobado** ni renumerarlo.

## Protocolo único de calidad, pruebas y publicación — todos los bloques

**Skill existente:** `Rubentxu/agent-skill/skills/delivery-first-testing/`.
Router: `SKILL.md` → `references/navigation.md` / `references/lens-registry.json` → `references/lenses/` **solo con status `ready`**. Si no está instalada en el agente, leerla del repositorio; no fingir invocación. El helper `scripts/affordances.py` **recomienda** rutas, no ejecuta ni certifica nada. `cross-repo-consumer` figura `planned`, no utilizarlo como lente activa.

- **Feature:** `change-impact → ephemeral-uat → regression-promotion`. Ejecutar tests de dominio/contrato afectados y `bash scripts/test-fast.sh`; no full suite tras cada ajuste.
- **Fallo repetido:** `failure-triage`: primera causa `PRODUCT_DEFECT|TEST_COUPLING|INFRASTRUCTURE|FLAKY|STALE_EVIDENCE|MISSING_EVIDENCE`; corregir la causa, no inflar tests ni timeouts.
- **PR / merge:** `merge-assurance`: `bash scripts/test-full.sh` **es obligatorio antes de PR/tag según `AGENTS.md`**, además de `merge-gate.pipeline.kts` real; no sustituir con un «fast» verde. Conservación de contratos CLI/MCP N−1 cuando cambien.
- **Integración:** `integration-assurance`: suite/fixtures transversales en `integration.pipeline.kts`, evitando repetir todos los UAT; si gate rojo, reparar o marcar bloqueo verificable, nunca publicar por inercia.
- **Release:** `release-assurance`: `release-candidate.pipeline.kts` y `release.pipeline.kts` mediante el runner autorizado; verificar SHA/commit del binario, listado **completo** de assets contra inventario esperado (no fijar el número 8), descargas remotas, SHA256, instalación y `cognicode`/`cognicode-mcp` reales. `sources.zip` autogenerado por GitHub **no es producto**.
- **Economía:** `harness-economics`: medir coste solo donde sea problemático; no introducir un harness/pipeline/estado duplicado.
- **Presupuesto de sesión por K:** **1 UAT funcional + 1 AAT negativa y 0–1 E2E adicional** si protege transporte/distribución que no se haya observado. Es mínimo sugerido, no límite cuando exista un riesgo crítico real. Probar resultados y negativos, nunca certificados vacíos ni `SKIP` como PASS.
- **Destino efímero:** directorio `mktemp -d` externo al repo, con cleanup por trap sobre el *path propiedad de la sesión*, salvo error crítico pendiente. Fixtures creados en tiempo de prueba, `timeout`, código de salida, revisión, artefacto y evidencia mínima registrados en `docs/roadmap/JOURNAL.md` según disciplina ya existente.
- **Cierre y borrado obligatorio:** antes de terminar cada K, clasificar los UAT/E2E/AAT **PROMOTE** (regresión mínima estable), **REPLACE** (test barato equivalente), **RETIRE** (duplicado/exploratorio) o **DEFER** (no se puede retirar aún); entonces eliminar físicamente solo las pruebas **efímeras propias** y validar `git status` limpio. No borrar pruebas permanentes de integridad, aislamiento, seguridad, concurrencia, persistencia ni autoridad sin reemplazo demostrable.
- **DONE:** consumidor demostrado + `test-full.sh` PASS real + `merge-gate` verde + PR merged a `origin/main` + RC e integración verificadas + GitHub Release remota + assets/completitud/checksums/instalación verificados + registro `JOURNAL` y `CURRENT` coherentes + test efímeros retirados o justificados como `DEFER`. Si no se cumple, `BLOCKED/PAUSED/NOT_RUN` explícito. No dar por hecha firma humana C8-R.

## Contratos funcionales y normas de diseño

- Programación funcional estilo Haskell **dentro de Rust**: núcleo puro de `clasificar archivo → extraer observaciones → normalizar identidad → resolver candidato → decidir cobertura`; efectos (FS, Tree-sitter, LSP, git, CLI/MCP, publish) confinados a adaptadores. `Result`/enums/ADTs con casos exhaustivos, smart constructors y listas no vacías para gaps cuando proceda; no crear nuevos tipos que dupliquen `AnalysisOutcome` o `TieredOutcome`.
- Una única autoridad para `LanguageConfig`, símbolos y aristas, sin convertir recuentos de símbolos en «porcentaje de precisión». `ObservedCall ≠ ResolvedCall`; `NoObservedCall ≠ NoCall`; `Partial ≠ Complete`; `CapabilityAdvertised ≠ CapabilityVerified`.
- Compatibilidad de `cognicode`, `cognicode-mcp`, `cogh`, catálogo y manifest; core read-only sin red, stdout MCP protocolo puro, logs stderr; capacidades de escritura y ejecución separadas.
- Preferir corregir el *seam* real a introducir arquitectura especulativa; **cero** test suites que solo comprueben un manifiesto documental sin ejecutar parser/caso de uso/binary cuando se afirme soporte Kotlin.

## Glosario — lenguaje ubicuo

| Término | Significado para el agente |
|---|---|
| **Alcance (scope)** | Archivos y capacidades que una operación prometió analizar, incluidos excluidos declarados |
| **Cobertura** | Qué fuentes relevantes se pudieron analizar para un alcance y cuáles no; **no** porcentaje de líneas de test |
| **Complete** | Todas las fuentes relevantes del alcance declaradas y procesadas con las garantías requeridas |
| **Partial** | Resultados útiles pero faltan archivos/capacidades relevantes, indicados con gaps |
| **Unsupported** | El lenguaje u operación no se puede atender; **no** significa resultado vacío |
| **Unknown** | No hay evidencia suficiente para una conclusión, sin inventar `NoImpact` |
| **Símbolo** | Declaración identificada por ubicación, lenguaje y contexto, no únicamente por nombre |
| **Invocación observada** | Expresión de llamada reconocida sintácticamente; su destino puede ser desconocido |
| **Arista resuelta** | Relación entre caller y callee sustentada por evidencia adecuada |
| **Precisión S0/S1/S2** | S0 Tree-sitter heurístico; S1 índice local; S2 LSP. S3 SCIP y S4 compilador están reservados, no activos |
| **ADTs** | Tipos algebraicos (enums/sum types) que hacen explícitos estados mutuamente excluyentes |
| **UAT / E2E / AAT** | Aceptación del usuario / recorrido integral de componentes / aceptación adversarial-negativa; preferentemente efímeros |
| **Regresión permanente** | Test mínimo que permanece por proteger un defecto o contrato importante |
| **RC** | Artefacto candidato de release construido sobre SHA conocido, certificado antes de publicar |
| **Agent team** | Equipo de agentes en worktrees disjuntos; un integrador posee decisiones, merges y releases |
| **Lente** | Procedimiento de `delivery-first-testing` que ayuda a elegir las pruebas adecuadas según contexto |

## Handoff de inicio (copiar al agente)

> Abre `AGENTS.md`, `docs/roadmap/ROADMAP.md` §11 y `docs/roadmap/consumer-contracts/ROADMAP.md`, verifica `origin/main`, los últimos commits y tags/releases. CU-2.5 se encontraba abierto durante la revisión y la release `v0.101.20` estaba publicada; reconcilia ambos hechos antes de reservar la siguiente versión. Integra los cuatro bloques Kotlin anteriores **dentro del programa CU existente**, sin nuevas referencias ni un segundo roadmap, y ejecuta el primer bloque de valor habilitado. Valida código y consumidores reales antes de marcar TODO como cumplido. Usa el router y lentes `ready` de `delivery-first-testing`, tests afectados en inner loop y full exigido por el proyecto en frontera PR/tag. En cada bloque entrega PR remoto merged, release instalada y smoke, evidencia concreta y retirada controlada de UAT/E2E/AAT efímeros. Si falta una garantía crítica, un permiso o un gate, informa `PAUSED/BLOCKED`, no declares DONE.
