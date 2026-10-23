# K0–K4: roadmap de valor, complementario al vigente

> **Única agenda:** `docs/roadmap/ROADMAP.md`. Este documento es un anexo técnico; el estado, prioridad y work item siguiente se actualizan **solo en el roadmap principal y journal**. Al importar, iniciar como `PROPOSED` y activar K0 tras reconciliar trabajo existente. `docs/prf/` es histórico.

## Contrato común de un hito

Todo hito termina en **funcionalidad observable usada por un consumidor**, un PR a `main` con `merge-gate` real verde, integración remota verificada, versión SemVer decidida desde versión remota actual, tag, GitHub Release con binarios y manifiestos completos, instalación/uso real de artefacto y recibo append-only en `docs/roadmap/JOURNAL.md`. Si falta cualquiera: `PAUSED/BLOCKED`, sin fingir `RELEASED`. No publicar ni hacer push/merge sin autorización operativa; esta solicitud autoriza la *planificación*, no credenciales o bypass de los gates.

**Gates existentes:** en desarrollo `bash scripts/test-fast.sh`; antes de PR y de tag `bash scripts/test-full.sh` según `AGENTS.md`; CI `merge-gate` obligatorio, no agregar GitHub Actions ni otro pipeline. Para cada release, la skill `delivery-first-testing` activa `release-assurance` y comprueba assets esperados, checksums, instalación y la nueva capacidad en binarios distribuidos. Regresión minúscula y crítica se queda; UAT/E2E/AAT temporales se retiran tras decidir `PROMOTE|REPLACE|RETIRE` y registrar resultados.

## K0 — La cobertura ya no miente | BUGFIX / primera release

> **Estado al integrar (2026-10-17, baseline `main@755e97a6` v0.101.21):**
> K0 está en PR #377 (`k0/honest-coverage` → `main`) con
> `merge-gate` en curso al cierre. Los criterios 1 (parcial), 2, 3
> (parcial) y 6 están cerrados en el commit; los criterios 3
> (resto), 4 y 5 quedan para **K0.1** (no iniciado). El cierre
> formal acontece al squash-merge. Ver
> [`INTEGRATION.md`](INTEGRATION.md) §"K0 — qué se cerró y qué
> queda".

**Valor:** al consultar un repositorio mixto, CogniCode no dice `Complete` cuando no ha considerado Kotlin/Java u otros lenguajes relevantes.

- [x] Reconciliar `main`, tag publicado, `CURRENT`, roadmap actual y el enlace CLI/MCP 2026-Q4 inexistente; no reabrir PRF ni bloquear indefinidamente por un snapshot stale. _Main y `CURRENT` reconciliados a `755e97a6` post-CU-2.5; el enlace `2026-Q4-CLI-MCP-ROADMAP.md` permanece ausente y NO se crea (decisión upstream: no inventar)._
- [x] Unificar la selección de fuentes del full/per-file en una función pura que consulte el catálogo canónico `LanguageConfig` + política de exclusión. No mantener listas rígidas `rs|py|js|ts` en `strategy.rs`. _Cerrado en `per_file_graph::classify_walk_extension` único; `strategy.rs` ×3 sitios y `analysis_service.rs` derivados._
- [ ] Representar por fichero `Indexed`, `Unsupported`, `IgnoredByPolicy`, `ParseFailed`, `ReadFailed`, `Cancelled`, `Unknown` y razones acotadas, distinguiendo **universo descubierto, elegible y analizado**. `Complete` solo relativo a alcance explícito; no inferir `Complete` de `Vec<SkippedFile>::is_empty()` si se filtró el universo antes. _K0 introduce `WalkClassification::{Analyze, KnownUnsupportedGap, LegitExclusion}` (3 variantes, no 7+). Taxonomía completa y `CompleteForScope` quedan en **K0.1**._
- [ ] Conservar distinción `SkippedUnchanged` incremental ≠ `SkippedUnsupported`; validar hashes/identidad y preservar `PRF-ANA-03`. _Sin cambios; preservado por status quo. Queda en **K0.1**._
- [ ] Publicar diagnóstico CLI/MCP vía DTO existente (ampliación compatible) con conteos, extensiones y rutas omitidas recortadas/seguras; si no puede exponerse sin ruptura, nuevo campo opcional/documentado. _Sin cambios. Queda en **K0.1**._
- [x] Actualizar `product/languages.json` *solo por contrato generado*; Kotlin todavía `unsupported/not_advertised`, jamás marcarlo supported con K0. _Cerrado: `kotlin` fuera de la lista de lenguajes que el producto lee; la lista se deriva del registro de parsers y `product/languages.json` se genera desde el mismo registro, así que un parser que no está cableado no puede anunciarse._

**Release usable:** el consumidor recibe cobertura honesta y motivo de omisión; patch si API compatible. **DoD:** la UAT K0 reproduce un falso `Complete` antes y `Partial` después; compare full/per-file mismo alcance; release remoto verificado. **Fuera:** parser Kotlin y SCI/SCIP.

**Riesgos/seams:** `infrastructure/graph/strategy.rs` y `per_file_graph.rs` tienen caminos duplicados; añadir una sola selección canónica, mantener adaptadores CLI/MCP delgados. **Spec:** `specs/SPEC-COVERAGE.md`. **ADR:** `adr/ADR-K001.md`.

## K1 — Kotlin navegable y fragmentos por símbolo | FEATURE / segunda release

**Valor:** `get_file_symbols`, `query_symbol_index` y `get_symbol_code` funcionan sobre `.kt` reales; scripts `.kts` clasificados explícitamente.

- [ ] Seleccionar y fijar gramática Tree-sitter Kotlin tras spike acotado y compatibilidad Rust/crate; analizar nodos reales antes de añadir nombres a config.
- [ ] Registrar `Language::Kotlin`, extensiones `.kt/.kts`, `LanguageConfig` y parser; unificar nodos múltiples de función/tipo/variable en extractor sin bifurcación ad hoc por lenguaje; evitar dos tablas semánticas contradictorias `Language::*_node_type` vs config.
- [ ] Extraer identificadores con identidad consistente: package, top-level functions/properties, class/interface/enum/sealed/data/value, constructor, `object`, `companion`, extension receiver; scripts `.gradle.kts`/`.pipeline.kts` sin inventar servicios runtime.
- [ ] Dar soporte a código del símbolo y ubicaciones correctas, UTF-8 y límites; cubrir scripts y Unicode sin overflow de coordenadas.
- [ ] Habilitar CLI/MCP para operaciones navegables sin requerir `build_graph`; sincronizar `product/languages.json` y capabilities con distinción `syntax_available` vs `semantics_available`.

**Release usable:** navegar un proyecto Kotlin real desde binarios `cognicode` y `cognicode-mcp`. **DoD:** prueba de consumidor K1 en un proyecto mínimo + paquete distribuido, sin grafo global; matriz declarada `experimental/AST-only` hasta aceptación semántica. **Fuera:** resolver todas las llamadas Kotlin.

**Spec:** `specs/SPEC-KOTLIN-SYMBOLS.md`. **ADR:** `adr/ADR-K002.md`.

## K2 — Grafo Kotlin/Java fiable | FEATURE / tercera release

**Valor:** la pregunta «qué llama a qué» produce aristas Kotlin↔Kotlin y Kotlin↔Java justificadas o `Unresolved`; nunca asigna arbitrariamente un homónimo.

- [ ] Extraer llamadas y tipos Kotlin en AST (`?.`, `::`, infix, constructor, lambda, scope functions, extension function); registrar callsite y relación tipada con procedencia.
- [ ] Resolver por ámbito: import/package, propietario, receiver, firma/arity, scope, overloads, visibilidad y referencias Java; **no** resolver por coincidencia de nombre simple o proximidad de archivo cuando hay alternativas válidas.
- [ ] Modelar `Resolved`, `Ambiguous(candidates)`, `Unresolved(reason)`, `External` como ADT puro. No convertir corrutinas o `Flow` en orden de ejecución ni topología de procesos sin evidencia.
- [ ] Alimentar grafo canónico existente y proyecciones con `Evidence`; un solo dueño de semántica (dominio/ingest), ninguno en formatters/handlers; reconciliar Full vs PerFile, determinismo cold/warm.
- [ ] Corregir representación de aristas omitidas para que cliente/diagrama pueda pedir siguiente consulta estructurada sin raw read.

**Release usable:** references/call-subgraph/implementors cuando estén comprobados sobre Kotlin; si `get_implementors` necesita LSP declararlo parcialmente soportado, no mentir. **DoD:** UAT K2 con overloads y homónimos (un caso positivo y uno negativo) y un caso cross-language; release y uso del binario instalado. **Fuera:** CFG, DFG y precisión compilador completa.

**Spec:** `specs/SPEC-KOTLIN-RELATIONS.md`. **ADR:** `adr/ADR-K003.md`.

## K3 — Proveedor LSP Kotlin con degradación honesta | FEATURE / cuarta release

**Valor:** en proyectos JVM configurados el consumidor obtiene definiciones y referencias más precisas con LSP; si no está disponible, navegación AST continúa.

- [ ] Implementar descriptor/probe Kotlin sobre el `LspProcessManager` y `CompositeProvider`, sin cargar proceso LSP por defecto en operaciones que no lo necesitan.
- [ ] Investigar instalación/protocolo reales del servidor elegido; manejar stdio, root/workspace, inicialización, capabilities, cierre, budgets, crash limiting, cancelación y errores tipados.
- [ ] Corregir mapeo de offset byte/row AST ↔ LSP UTF-16/line/column, URIs y documentos múltiples. Elegir severidad de coincidencia por tier existente S2/S1/S0.
- [ ] Solo mejorar identidades y aristas cuando el LSP observó símbolo inequívoco; degradación `Unresolved` con diagnostics, nunca fallback textual ni «LSP disponible = todo Kotlin completo».
- [ ] Mostrar `provider`, `tier`, estado y limitaciones en CLI/MCP, preservando compatibilidad N-1.

**Release usable:** Kotlin con navegación semántica precisa en proyecto Gradle/Maven JVM, pero S0 funciona sin LSP. **DoD:** AAT K3 instala binario, deshabilita servidor y comprueba fallback sin falsa arista; release con capacidad advertida como opcional. **Fuera:** obligar JDK/Gradle para el core; PSI como dependencia del binario Rust.

**Spec:** `specs/SPEC-PROVIDERS.md`. **ADR:** `adr/ADR-K004.md`.

## K4 — Índice SCIP opcional y preguntas agent-first | FEATURE / quinta release

**Valor:** consumer puede resolver una relación ambigua importando SCIP real cuando su entorno lo produce, y consultar símbolo/collaboradores/cobertura con una respuesta estructurada pequeña.

- [ ] **Gate de decisión:** ejecutar un diferencial sobre ambigüedades no resueltas con K3. Si SCIP no mejora un caso de usuario, **no** construir un importador especulativo; sustituir esa subtarea por experiencia agent-first verificable y registrar `NOT_TRIGGERED`.
- [ ] Si hay valor, importar `index.scip` generado externamente por proveedor Kotlin/Java vigente, a través de adaptador de `S3` existente/reservado. Validar símbolos, ocurrencias, archivos, revisión, versiones y duplicados. Sin descarga/red o ejecución de Gradle implícitas en CogniCode core.
- [ ] Reconciliar SCIP con identidad de símbolos/callsite existente mediante funciones puras; registrar conflictos y referencias no resolubles sin sobrescribir evidencia AST/LSP.
- [ ] Añadir respuestas por pregunta: `symbol → members/references/subgraph → evidence/coverage → recommended_next_queries`; usar herramientas MCP/CLI y PageRank existentes, sin nueva base/vector DB ni nuevo motor de grafos.
- [ ] AAT efímera en cliente MCP real comprueba escoger la herramienta pertinente, consultar solo lo necesario y **no** reconstruir arquitectura con `read_file(raw)` o documentación.

**Release usable:** semántica complementaria y navegación agent-friendly incluso si SCIP es `NOT_TRIGGERED` (siempre que exista una capacidad pública nueva medible). **DoD:** MCP publicado conserva schema/autoridad y produce evidencias mínimas para C4; release remoto comprobado. **Fuera:** protocolo alternativo al MCP, ontología paralela, servicios remotos obligatorios.

**Spec:** `specs/SPEC-PROVIDERS.md` + `specs/SPEC-AGENT-API.md`. **ADR:** `adr/ADR-K004.md`.

## Dependencias y criterios de prioridad

`K0 → K1 → K2 → K3 → K4`. Son **releases secuenciales**, aunque dentro de cada una se paralelicen implementaciones independientes. Si un bloque queda sin valor consumible, reducir el alcance *dentro* del bloque; no publicar un tag para «cerrar documentación». No impulsar K3/4 antes de solucionar falsos resultados K0/K2. CP0/CP2 (product truth y MCP) deben incorporar el nuevo soporte sin abrir otro programa. El operador coordina versiones y releases con los demás equipos.

**Entrega medida**: cada bloque añade al menos una pregunta antes imposible y ahora contestable honestamente; latencias y regresión se miden respecto del baseline, no se fijan cifras inventadas. Preferir throughput de releases correctas a expansión del harness.

## K0.1 — La cobertura se distingue de "no hay nada" | BUGFIX / release intermedio

> **Estado:** PROPOSED. Depende de K0 cerrado en `main` con
> `merge-gate` verde. WU acotada, no requiere cambios de gramática ni
> nuevos lenguajes.

**Valor:** los tres criterios que K0 deja entreabiertos
(representación por fichero, `CompleteForScope`, diagnóstico CLI/MCP)
se cierran en una sola pasada para que el veredicto de cobertura
deje de confundir "no había nada" con "lo que había, lo cubrí". El
consumidor distingue hoy: "CogniCode vio N archivos y completó M"
de "CogniCode vio N archivos, completó M, dejó de ver K, e intentó P
de los cuales F fallaron al parsear".

- [ ] Taxonomía por fichero ampliada: `Indexed`, `Unsupported` (gap
  honesto, ya vía `KnownUnsupportedGap`), `IgnoredByPolicy`,
  `ParseFailed`, `ReadFailed`, `Cancelled`, `Unknown` con razones
  acotadas y `provenance` por variante. Sustituye el enum actual
  `WalkClassification` de 3 variantes; `BuildStatus::Complete` se
  redefine como `CompleteForScope` con `ScopeManifest` explícito.
- [ ] `CompleteForScope` exige `Indexed + Unsupported.count = universe.size`
  ∧ `ParseFailed ∩ scope = ∅` (o falla tipada y reportada). Deja
  de inferirse de `Vec<SkippedFile>::is_empty()`. PRF-ANA-03
  preservado (hashes e identidad de SkippedUnchanged incremental).
- [ ] DTO CLI/MCP: `cohort.universe`, `cohort.eligible`, `cohort.indexed`,
  `cohort.skipped{by_reason}`, `cohort.first_failures[]` (recortado
  y sanitizado). Compatibilidad N-1: campo opcional nuevo,
  consumidores existentes leen lo que ya leían.
- [ ] Tests RED→GREEN: tres casos en
  `crates/cognicode-core/tests/k0_1_coverage_taxonomy.rs`
  pineando la nueva distinción; suite efímera en K0.1 que se
  retira al cierrevía `promote` lens.

**Release usable:** misma versión SemVer que K0 (PATCH); binarios
`cognicode` y `cognicode-mcp` exponen los nuevos campos sin perder
consumidor. **DoD:** AAT K0.1 construye, valida el campo, y el
veredicto ya no dice "Complete" cuando hay `.kt/.kts` descubiertos
y no analizados. **Fuera:** cualquier cambio de gramática,
mecanismo de inferencia de scope, o segundo canal de cobertura.

**Spec:** nuevo `specs/SPEC-COVERAGE.md` §k0_1_taxonomy, revisión
del actual. **ADR:** `adr/ADR-K005-coverage-taxonomy.md` (nuevo,
PROPOSED hasta aceptación).

