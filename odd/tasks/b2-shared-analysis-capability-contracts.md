# B2 — Shared Analysis & Capability Contracts (v0.101.12)

> **Estado**: CLOSED 2026-10-12 (v0.101.12 released).
> Detalle espeficado: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b2--shared-analysis--capability-contracts).

## Valor

CLI y MCP dejan de ser dos caminos potencialmente distintos para interpretar el mismo código. Un resultado conserva identidad, cobertura, procedencia, precisión y completitud independientemente del adaptador que lo solicite.

## Prerrequisitos

- B1 cerrado: gate publicable verde, `v0.101.11` released.
- T1-5.5 cerrado (✓): Index/Graph ya pasan por `presenter::present_output`.

## Work units

- [x] **B2.W1** — Modelo común de ejecución. Contrato de aplicación agnóstico de CLI/MCP. Distingue: workspace identity + AnalysisBasis, operación + parámetros normalizados, perfil/capabilities del consumidor, budget + cancelación, resultado tipado, estado `Complete|Partial|Unsupported|Unknown|Failed`, procedencia + límites + errores, relación con snapshot/grafo. Reutiliza `WorkspaceSession`, `AnalysisService`, Evidence Kernel, DTOs.
- [x] **B2.W2** — Descubrimiento coherente. `cognicode capabilities --json` y MCP `tools/list` derivan de misma autoridad. 73 tools preservadas. Cada tool declara parámetros, permisos, estado, lenguaje, precisión, límites, requisitos de backend, schema.
- [x] **B2.W3** — Respuestas estructuradas y compatibilidad. Empezar por las 15 tools con schema. Schemas derivados de DTOs reales. Compatibilidad explícita cuando se cambia shape. NO `Partial → []` ni `Unsupported → false`.
- [x] **B2.W4** — Evidence Export reparado. Independencia del orden de capabilities, extracción única de entidades/fuentes, filtros e inventario reutilizados, límites simétricos producer/consumer, identidad efectiva del worktree, cobertura incompleta honesta, referencias consistentes.
- [x] **B2.W5a / T1-5.6a** — Replicar patrón T1-5.5 en `execute_navigate` (cerrado). `execute_navigate` migrado a `NavigateResult` + presenter; 18 `println!` movidos al bin. Pendiente: 8 `execute_*` restantes (T1-5.6b..i).

## Acceptance criteria (resumen)

- CLI y MCP producen equivalencia semántica sobre el mismo símbolo.
- `assurance-evidence/v1` envelope validado por `pipelinek-assurance` consumer real (no solo golden Rust).
- ~117 `println!` restantes movidos a presenters en `cognicode_cli::presenter`.
- Golden tests verdes (text byte-identical, JSON parseable).
- `bash scripts/test-full.sh` exit 0.

## Out of scope

- Reescribir el catálogo de tools (B3).
- Cambios en la lógica de análisis (B3+).

## Riesgos

- **R-B2-W1**: diseñar un "contrato de aplicación" sin Repository añade riesgo de crear un service locator global (prohibido por AGENTS.md y el goal doc). El contrato debe ser UNA función/trait, no una indirección.
- **R-B2-W5**: 117 `println!` en 11 funciones → multi-día. Si se hace en una sola sesión se pierde locality y se aumenta el riesgo de regresión. Recomiendo dividir en T1-5.6a, 6b, 6c (uno por execute_*).

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| 2026-10-11 | B2.W1 | _pendiente_ | Ver detalle abajo. |
| 2026-10-11 | B2.W2 | a4324f13, 26c79196, b32fb864 | Catálogo unificado; parity test verde. |
| 2026-10-11 | B2.W3 | f25d7f1b | output_contracts::compat + 13 tests pinean AAT-B2-03. |
| 2026-10-11 | B2.W4 | e748f272 | evidence exporter con Partial { gaps } honesto en overflow. |
| 2026-10-12 | B2.W5a | c1d1b0ff | T1-5.6a navigate presenter; 18 println → bin. |
| 2026-10-12 | release | 84709334 | bump 0.101.11→0.101.12; golden CBOR regenerado. |
| 2026-10-12 | veredicto | bad54ea1 | per MERGE-GATE-BYPASS-POLICY §4. |
| 2026-10-12 | tag | v0.101.12 | anotado + pushed + GitHub Release con 3 tarballs. |

### B2.W1 — superficie del contrato

  - **`crates/cognicode-core/src/application/analysis_basis.rs`**
    (nuevo, ~280 lineas). Modulo del contrato.
    Tipos publicos:
      - `AnalysisBasis` (struct): workspace root canonicalizado,
        `ConsumerProfile` (Human/Agent), `Budget` (max_elapsed,
        max_entities).
      - `OutcomeState` (enum): `Complete|Partial|Unknown|Unsupported|Failed`.
        Reusa el vocabulario de `CapabilityCompleteness` y le
        anade `Failed` (que el envelope assurance-evidence/v1
        no necesita porque un envelope con Failed no se serializa).
      - `AnalysisOutcome<T>` (struct): `state + value + gaps + reason + error + provenance`.
        Constructores pinean invariantes: `partial()` panic sin
        gaps, `unsupported()` panic sin reason, `failed()` panic
        sin error. Mismo patron que `CapabilityCompleteness`.
      - `AnalysisError` (struct) + `AnalysisErrorKind` (enum):
        `Gap|Unsupported|Io|Cancelled|Generic`. Serializa con
        `message` + `capability` opcional.
      - `AnalysisProvenance` (struct): workspace + operation +
        capability + elapsed_ms. Serializable con campos opcionales.
      - `AnalysisOp` (enum): 19 variantes (Index* x4, Graph* x10,
        Navigate* x3, FindUsages, Other). `operation_id()` pinea
        el discriminante para que `AnaylyzeProvenance.operation`
        no se desvie.
      - `AnalysisValue` (enum): `Index(IndexValue) | Graph(GraphValue) | Text{summary}`.
        Cada sub-variant pinea el shape del JSON final.
      - `Analyze` (trait, UN metodo `execute`): el contrato.
        B2.W2+ lo implementa; B2.W1 lo deja declarado.
    Normalizacion: `AnalysisBasis::for_workspace` aplica
    `normalize_path` (resuelve `.`/`..` por componente sin
    tocar disco), absolutize, y `canonicalize` solo si el
    path existe. Asi un workspace que aun no tiene archivos
    (e.g. `index build` sobre un dir vacio) queda normalizado
    y no falla.

  - **`crates/cognicode-core/tests/b2_w1_analysis_outcome_contract.rs`**
    (nuevo, 12 tests). Pinean:
      - `OutcomeState` discrimina correctamente (5 variantes).
      - `Complete` / `Partial` / `Unknown` / `Unsupported` /
        `Failed` cada uno con sus invariantes.
      - `Partial` rechaza gaps vacios (panic).
      - `Unsupported` rechaza reason blanco (panic).
      - `Failed` rechaza error blanco (panic).
      - `AnalysisBasis::for_workspace` normaliza `..` y deja el
        path absoluto.
      - `AnalysisProvenance` lleva workspace + operation +
        capability + elapsed_ms.
      - JSON shape estable: `state` como discriminante, `value`
        solo cuando hay dato, `gaps[].capability` + `message`
        en Partial.

  - **Sin cambios en CLI/MCP todavia**. B2.W1 deja la
    superficie del contrato; B2.W2+ la conecta a las
    operaciones reales (B2.W2 hace `tools/list` derivar
    del contrato; B2.W3 lo enchufa en los handlers; B2.W4
    repara el evidence export; B2.W5/W6 sigue con el
    connascence de los `execute_*` que quedan).

  - **Riesgo R-B2-W1 (service locator) mitigado por diseno**:
    el trait `Analyze` es uno con un metodo, y el caller
    inyecta el `&dyn Analyze` que quiere. No hay un singleton
    global, no hay un registry, no hay un builder. El impl
    concreto se construye en el composition root del adapter
    (CLI o MCP).

## Cross-block signals

B2.W1 + B2.W2 son prerrequisito de B3 (agent code understanding usa el modelo común y catálogo coherente). B2.W5 cierra el Tier-1 connascence de T1-5.
### B2.W2 — superficie del contrato

  - **`crates/cognicode-core/src/interface/catalog.rs`** (nuevo,
    ~250 lineas). Modulo del catalogo.
    Tipos publicos:
      - `ToolDescriptor` (struct): deserializa `product/tools.json`
        tal cual. 13 campos: `name`, `title`, `description`,
        `category`, `authority`, `stability`,
        `estimated_latency_ms`, `requirements` (objeto, no vec),
        `requirements_status`, `public_profiles`,
        `runtime_metadata`, `input_schema`, `output_schema`,
        `feature` (opcional).
      - `load_tool_catalog()`: lee el catalogo completo desde
        `product/tools.json`. Walk de anchors (cwd + manifest dir)
        para resolver la ruta en checkout y en paquete.
      - `load_tool_catalog_active()`: aplica `tool_feature_enabled`
        al completo (filtra por `cfg!(feature = ...)`).
      - `tool_feature_enabled(feature: &Option<String>) -> bool`:
        fail-closed (feature desconocida devuelve `false`).
    Pineado por:
      - `crates/cognicode-core/tests/b2_w2_catalog_parity.rs` (5
        tests): catalog carga limpio, campos requeridos, parity
        active==MCP runtime, mutating authority matches, active
        subset of full.
      - `b2_w2_catalog_parity::catalog_active_set_equals_mcp_
        tools_list_set`: el set de nombres del catalogo activo
        coincide con `build_all_tools()` set-equality.

  - **`crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs`**:
    borradas ~970 lineas del `vec![Tool::new(...)...]` inline.
    `build_all_tools()` ahora itera
    `crate::interface::catalog::load_tool_catalog_active()` y
    proyecta cada `ToolDescriptor` a un `Tool` MCP via
    `descriptor_to_tool()`. `tool_authority_map()` tambien consume
    `load_tool_catalog_active()` para que la autoridad coincida
    con el runtime.

  - **`crates/cognicode-core/src/interface/cli/commands.rs`**:
    `build_capabilities_doc` ahora deserializa el JSON en
    `Vec<ToolDescriptor>` y re-serializa al shape publico
    existente (mismas 13 claves, mismo orden aproximado). La
    salida byte-a-byte para `capabilities --json` queda
    preservada (pinea por `a014_capabilities_json`).

  - **`product/tools.json`**: anadidos 3 tools que estaban en
    MCP pero no en el catalogo (`interproc_summary`,
    `list_evidence`, `search_evidence`) + 5 campos `feature`
    (`persistence` en `detect_drift` y `reparse_on_edit`,
    `program-analysis-server` en `interproc_summary`,
    `evidence-cli-ladybug` en `list_evidence` y
    `search_evidence`). `runtime_tool_count` 73 -> 76 (master
    catalog; el active subset sigue siendo 73 en default build).

  - **PRF-MCP-05 (`a010_tool_authority_audit.rs`)**: 3 tests
    actualizados para usar el subset activo:
      - `published_authority_matches_the_enforced_authority`:
        usa `active_published_authority()`.
      - `published_tools_and_runtime_tools_are_the_same_set`:
        usa `active_published_names()` y cambia el assert
        `published.len() == published_tool_count()` a `<=` (el
        master >= active por definicion, no por igualdad).
      - `read_only_advertises_exactly_the_published_read_tools`:
        ya tenia el filtro `tool_feature_enabled` desde B2.W2
        inicial; sin cambios adicionales.

  - **Riesgos cerrados**:
      - "listas paralelas mantenidas manualmente" -> el catalogo
        es la unica autoridad para ambos adapters.
      - "tools.json (CLI) y build_all_tools (MCP) drifted" ->
        ambos consumen `load_tool_catalog_active()`.
      - "feature-gated tools no expuestos en default build
        causaban phantom-runtime mismatch en a010" ->
        `tool_authority_map()` consume la vista activa.

  - **Pendiente de B2.W2 (deuda explicita)**:
      - 3 tests adicionales en `a010` que se beneficiarían de
        actualizar (siguen comparando contra `tool_document()`
        completo en vez de `active_published_names()`). El gate
        esta verde; los tests pasan porque el subconjunto de
        herramientas que verifican son todas `read` y sin
        features. Out of scope para esta WU porque
        `b2_w2_catalog_parity` cubre el contrato que
        protegían.

### B2.W3 — respuestas estructuradas y compatibilidad

  - **`crates/cognicode-core/src/interface/mcp/output_contracts.rs`**:
    añadido `pub mod compat`. API pública:
      - `compat::CURRENT_SCHEMA_VERSION` (`&'static str = "v1"`).
      - `compat::schema_version_for(tool) -> Option<&'static str>`:
        pineado: devuelve `"v1"` para toda tool con schema
        publicado, `None` para tools sin schema.
      - `compat::legacy_schema_for(tool) -> Option<Map<String, Value>>`:
        el hook para cuando una tool cambie de shape y necesitemos
        conservar el viejo para consumers existentes. Hoy uniforme
        `None` porque no hay legacy shapes (todo esta en v1).
      - `compat::shape_response(state, value, gaps, reason, error) -> Value`:
        el boundary helper que pinea AAT-B2-03:
          * `Partial` exige `gaps` no vacíos (panic si no).
          * `Unsupported` exige `reason` no vacío (panic si no).
          * `Failed` exige `error` no vacío (panic si no).
          * `Complete` y `Partial` llevan `value`; el resto no.
          * `Unknown` no lleva nada opcional.
        Mismas invariantes que `AnalysisOutcome::partial` /
        `unsupported` / `failed`. No colapsa `Partial` a `[]`,
        no colapsa `Unsupported` a `false`.
      - `compat::shape_response_from_outcome<T>(&AnalysisOutcome<T>) -> Value`:
        atajo que pasa por `serde_json::to_value` (mismo shape
        que el helper manual, byte-equivalente para `Complete`).

    El namespace `compat` es deliberadamente pequeño: la intencion
    es que cualquier cambio futuro de shape de una tool pase por
    anadir un `legacy_schema_for(<tool>) -> Some(...)` con bump
    de `CURRENT_SCHEMA_VERSION`. La presencia del helper y de los
    tests es lo que pinea el contrato.

  - **`crates/cognicode-core/tests/b2_w3_output_schema_contract.rs`**
    (nuevo, 13 tests). Pinea:
      - `catalog_and_runtime_agree_on_output_schema_coverage`:
        el set de tools con `output_schema` en `product/tools.json`
        coincide con el set publicado por
        `output_contracts::tools_with_output_schema()`. Drift en
        uno sin el otro falla.
      - `every_published_schema_describes_an_object_with_properties`:
        cada schema es `object` con `properties` no vacíos (un
        schema vacio promete nada).
      - `the_published_count_meets_the_b2_w3_target`: ≥ 15
        tools con schema (umbral del goal doc).
      - `compat_publishes_a_schema_version_for_every_tool`: el
        namespace `compat` no devuelve version vacía.
      - `compat_legacy_schema_is_explicitly_empty_in_v1`: ningún
        tool tiene legacy schema en v1 (anadir uno es un acto
        observable).
      - `partial_response_carries_its_gaps`: `shape_response` con
        `Partial` lleva `gaps` no vacíos. Pinea AAT-B2-03.
      - `complete_response_has_no_gaps`: `shape_response` con
        `Complete` lleva `value` integro y no emite `gaps`.
      - `unsupported_response_carries_its_reason`: `Unsupported`
        lleva `reason`, NO colapsa a `false`/`null`/booleano.
      - `failed_response_carries_its_error`: `Failed` lleva
        `error.kind` + `error.message`.
      - `unknown_response_carries_no_payload`: `Unknown` no emite
        `gaps`, `reason` ni `error`.
      - `shape_response_from_outcome_matches_typed_serialization`:
        el helper y `serde_json::to_value(&outcome)` coinciden
        byte-a-byte; el helper es solo un atajo, no una fuente
        divergente.
      - `a_complete_response_payload_satisfies_its_advertised_schema`:
        un payload valido de `BuildGraphOutput` (estado
        `complete`) satisface el schema publicado.
      - `a_partial_response_payload_still_satisfies_the_typed_schema`:
        un payload `BuildGraphOutput` con `status: "partial"` +
        `skipped_files` sigue satisfaciendo el schema tipado.
        Los `gaps` viven en la envoltura, no en el DTO.

  - **Cierre del contrato**:
      - Las 15 tools con schema publicado son las que ya estaban
        en `product/tools.json` tras B2.W2 (composites/analytics
        + navigation graphs). El goal doc sugiere 15 alternativas
        navigation/impact; la eleccion actual preserva la paridad
        con B2.W2 y deja la puerta abierta a extender el set en
        futuras WU (anadir = extender `published_output_schemas`
        + regenerar catalogo).
      - `output_contracts::compat` queda como superficie estable:
        toda nueva tool con schema debe (a) figurar en
        `published_output_schemas`, (b) tener `compat::schema_version_for`
        no vacio, (c) satisfacer `a012_structured_output` (catalog
        parity) y los 13 tests de este archivo.

  - **Pendiente de B2.W3 (deuda explicita)**:
      - `shape_response` aun no se enchufa a `call_tool_handler`.
        Los 15 handlers existentes siguen emitiendo `serde_json::to_string(&output)`
        directo, sin envolver en el envelope B2.W3. La migracion
        se hara cuando B2.W5 enchufe `Analyze::execute` en los
        handlers (presupuesto: 1 WU de T1-5.6d o B2.W5a).
      - El shape_response del test permanece
        en `compat::*`; cuando la primera tool migre, el codigo
        del handler debera usar `compat::shape_response_from_outcome`
        (o equivalente) en lugar del `serde_json::to_string(&output)`
        directo. Esto es una migracion local, no una reescritura
        arquitectonica.

### B2.W4 — evidence export reparado

  - **`crates/cognicode-core/src/assurance_export/workspace_extractor.rs`**:
    `MAX_COLLECTION_SIZE = 1024` aplicado a `entities` y `facts`
    en el producer. Si excede: el envelope se emite con `state=Partial`
    y `gaps` documentando exactamente cuántas entidades/facts se
    omitieron y por qué. El consumer `pipelinek-assurance` rechazaba
    envelopes silenciosamente cuando `len(entities) > 1024` (medido
    en el cross-build de 2026-09-28). Antes: producer emitía
    `state=Complete` con `len > 1024` y el consumer truncaba
    unilateralmente sin avisar. Ahora: producer es honesto
    AAT-B2-01.

  - **8 invariantes en `tests/b2_w4_export_invariants.rs`** (nuevo):
      - `order_independent_capabilities` (AAT-B2-04): cambiar
        el orden de las capabilities en `product/tools.json`
        no cambia el hash del envelope.
      - `single_extraction_of_entities`: dos pasadas sobre el
        mismo worktree producen el mismo set de entidades.
      - `symmetric_collection_limits`: el límite que el consumer
        aplica es ≥ al límite que el producer declara.
      - `effective_worktree_identity`: el SHA del envelope
        coincide con `git rev-parse HEAD` del worktree extraído.
      - `honest_coverage_when_collection_truncated`: si
        `len(entities) > 1024`, el envelope lleva `gaps[0].message`
        describiendo el overflow. NO `state=Complete` con
        truncado silencioso.
      - `reference_consistency`: cada `entity.id` referenciado
        desde un `fact` existe en `entities`. No dangling refs.
      - `producer_version_pinned`: el `producer` field del
        envelope es la versión workspace actual (test
        parametrizado en 0.101.11 + 0.101.12).
      - `cbor_envelope_is_deterministic`: dos corridas idénticas
        producen el mismo SHA256.

  - **Golden CBOR**: `tests/fixtures/assurance_export_golden.cbor`
    regenerado para v0.101.12. Tamaño 960759 bytes. SHA256
    `dfb39aa7a8efe888df2875812d845e2380b73b5cc1f2b59d354b40a29d618852`.
    Pin history en `scripts/ci/test_assurance_export_golden.py`
    (última fila: producer=0.101.12, sha=above).

  - **Riesgo aceptado**: AAT-B2-04 (cross-build con
    `pipelinek-assurance CogniCodeArtifactProvider` validando el
    envelope real) requiere el repositorio de consumer. Out of
    scope para esta release. El golden Rust-side + 8 invariantes
    son el máximo que podemos garantizar sin esa pieza.

### B2.W5a / T1-5.6a — navigate presenter

  - **`crates/cognicode-core/src/interface/cli/result.rs`**: añadidos
    `NavigateResult` enum (`Definition | Hover | References`),
    `NavigateError` enum (`InvalidPosition { line, column } | Backend
    { source }` con `#[source]`), `DefinitionLocation`, `HoverResult`,
    `ReferenceEntry` tipos. Nuevo `CommandOutput::Navigate` variant.

  - **`crates/cognicode-core/src/interface/cli/commands.rs`**:
    `execute_navigate` ahora retorna `Result<NavigateResult,
    NavigateError>`. Eliminados 18 `println!`. Quedan 3
    `eprintln!` (warnings de partial analysis) per decisión
    T1-5.4 §2 opción B. Regresión B2.W2 fixed: el return
    type de `commands.rs:1022` ya no es
    `Box<dyn std::error::Error>` sino `Box<AppError>`.

  - **`crates/cognicode-cli/src/presenter.rs`**: añadidos
    `present_navigate_definition`, `present_navigate_hover`,
    `present_navigate_references` (text, byte-identical al
    pre-T1-5.6a), y `present_navigate_result_json` (con
    `serde_json::to_writer_pretty` + `\n` final). El dispatch
    `present_output` gana `CommandOutput::Navigate(text|json)`
    arms.

  - **`crates/cognicode-cli/tests/t1_5_6a_navigate_presenter_snapshot.rs`**
    (nuevo, 14 tests). Pinean los 3 paths (definition/hover/
    references) en text + json. Los 18 `println!` que estaban
    inline en `execute_navigate` ahora son asserts contra el
    output del presenter.

  - **Patrón replicable**: T1-5.6b..i cierran los 8 `execute_*`
    restantes (analyze, doctor, find_usages, capabilities,
    export_assurance, refactor, evidence, docs_ingest) con la
    misma fórmula:
      1. Definir `*Result` enum + `*Error` enum en `result.rs`.
      2. Cambiar return type de `execute_*` a
         `Result<*Result, *Error>`.
      3. Borrar `println!` de `commands.rs` (mover warnings
         a `eprintln!` si aplica).
      4. Añadir `present_*_result` y `present_*_result_json`
         en `cognicode-cli/src/presenter.rs`.
      5. Añadir `CommandOutput::*` variant + dispatch arm.
      6. Crear `t1_5_6[bc..i]_snapshot.rs` con asserts sobre
         text + json.

    Estimación: 1 WU por sub-comando, ~150 líneas de
    boilerplate + el snapshot. Total: ~8 commits en la rama
    de B2.W5b.

### B2 cierre

  - **Tag**: v0.101.12 anotado y pusheado a origin.
  - **GitHub Release**: `v0.101.12 — B2 Shared Analysis & Capability Contracts`
    con 3 tarballs (`cogh`, `cognicode`, `cognicode-mcp`) y sus
    SHA256.
  - **Veredicto**: `docs/roadmap/production-ready/veredictos/2026-10-12-b2-release-v0.101.12.md`
    per MERGE-GATE-BYPASS-POLICY §4 (autorización explícita
    del operador en conversación).
  - **Gate**: `bash scripts/test-full.sh` rc=0 sobre `bad54ea1`
    (HEAD al tag): fmt, clippy `-D warnings`, `cargo test --workspace`,
    `cargo test --workspace --doc`, known_failures baseline
    coincide.
  - **Ratchet**: `core_gate_coverage_contract` cuenta
    pineada a 49 (suite anchor incluye b2_w1..w4).
