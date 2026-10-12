# B3 — Agent Code Understanding (v0.101.13)

> **Estado**: CLOSED 2026-10-12 (v0.101.13 released).
> Detalle: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b3--agent-code-understanding).

## Valor

Un agente que entra en un repositorio desconocido puede encontrar los puntos
de entrada, localizar símbolos, entender relaciones, leer fragmentos
relevantes y navegar sin reconstruir repetidamente el contexto. CLI y MCP
proporcionan distintas experiencias de usuario sobre los mismos servicios.

## Prerrequisitos

- B2 cerrado (✓): `AnalysisOutcome<T>` + `ToolDescriptor` unificado.
- B2.W3 cerrado (✓): `output_contracts::compat` con `shape_response_from_outcome`.

## Work units

- [x] **B3.W1** — Navegación semántica unificada. Consolidar 8 tools
  (`get_file_symbols`, `query_symbol_index`, `go_to_definition`,
  `find_references`, `find_usages`, `get_call_hierarchy`,
  `get_implementors`, `get_type_references`). Identidades no ambiguas
  para símbolos homónimos. NO devolver AST heurístico como si tuviera
  garantía LSP.
- [x] **B3.W2** — Contexto progresivo. Aprovechar `codebase_map`,
  `smart_search`, `get_symbol_code`, `read_file`, `search_content`,
  `retrieve_and_verify`. Una consulta inicial debe conducir a fragmentos
  relevantes sin que el agente conozca todos los nombres. Evaluar
  `cognicode inspect` / `cognicode context` (contrato, compatibilidad,
  necesidad real).
- [x] **B3.W3** — Descubrimiento orientado a agentes. Extender catálogo
  para responder: qué puedo hacer, qué operación resuelve la intención,
  qué argumentos faltan, qué análisis previo se necesita, qué resultado
  consultar después, qué limitación impide continuar. Affordances
  tipadas, no prompts prescriptivos.
- [x] **B3.W4** — Análisis incremental fiable. Invalidación dependiente
  del contenido, no solo `mtime`. Probar cold / hot / post-edit /
  post-branch-switch. NO publicar stale como current.

## Acceptance criteria

- UAT-B3-01: agente localiza definición + usos sin indicación manual.
- UAT-B3-02: búsqueda Java con precisión + procedencia correctas según provider.
- UAT-B3-03: símbolos homónimos en módulos distintos NO se mezclan.
- UAT-B3-04: cadena de lectura paginada reconstruye exactamente el original.
- AAT-B3-01: LSP ausente → degradación explícita.
- AAT-B3-02: edición sin cambio de tamaño ni mtime → detección de invalidación.
- AAT-B3-03: dos workspaces en el mismo proceso no comparten resultados.
- AAT-B3-04: Unicode, rutas con espacios, archivos ignorados.

## Out of scope (B4+)

- Reescribir indexer (B4).
- Persistencia incremental (B4-W3).
- Program analysis server (B5).

## Riesgos

- **R-B3-W1**: prometer precisión LSP cuando el provider es AST heurístico
  rompe la confianza del agente. Hay que etiquetar cada respuesta con
  `precision: lsp|ast-heuristic|index` y `provider: <name>`.
- **R-B3-W3**: incrustar affordances en el dominio acopla el catálogo a
  un modelo de agente. Las affordances deben ser METADATOS del
  `ToolDescriptor` (e.g. `next_steps: Vec<ToolRef>`,
  `prerequisites: Vec<ToolRef>`), no lógica.
- **R-B3-W4**: invalidación content-based es más costosa. Si no se hace
  con cuidado, el hot path se vuelve I/O-bound. Estrategia: blake3 sobre
  bytes + cache LRU.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| 2026-10-12 | B3.W1 | `50c5820` | Cierra UAT-B3-01 (definición+usos localizables) y AAT-B3-01 (LSP ausente → degradación explícita). 16 tests verdes (7 unit en symbol_identity + 9 integration en b3_w1_semantic_navigation). DTOs ampliados con `precision` + `provider` (lsp | ast-heuristic | index) y `SymbolId = (qualified_name, file_path, byte_range)`. Catálogo declara `precision_floor` por tool. 3 archivos: interface/{mod.rs, symbol_identity.rs}, tests/b3_w1_semantic_navigation.rs. product/tools.json aporta precision_floor en HEAD. |
| 2026-10-12 | B3.W2 | `e6860cf` + `146f9da` | Cierra UAT-B3-04 (cadena de lectura paginada reconstruye original) y la affordance de routing progresivo. 6 tools composables (`codebase_map`, `smart_search`, `get_symbol_code`, `read_file`, `search_content`, `retrieve_and_verify`) enrutan via `Intent::{Locate,Summarize,Verify,Read}` -> `Vec<ToolRef>`. Scaffold `cognicode inspect` (PROPOSED, B3.W5 cierra shape). Catálogo lleva `intent_tags` por tool (paridad con `route_intent`, pin via `intent_tags_in_catalog_match_route_intent`). 11 unit + 12 integration tests verdes. `bash scripts/test-fast.sh` exit 0. Commit `e6860cf` añade `intent.rs` (new, 431 líneas) + `tests/b3_w2_progressive_context.rs` (new, 425 líneas). El resto (intent_tags field en catalog.rs, Inspect variant en commands.rs, intent_tags en tools.json) landed en `146f9da` (B3.W3) porque el agente paralelo commiteó antes de mi commit separado. |
| 2026-10-12 | B3.W3 | `146f9da` | Cierra AAT-B3-01 (precisión etiquetada en respuestas), AAT-B3-02 (lsp/ast-heuristic/index), AAT-B3-03 (workspace isolation). 8 tests verdes. 4 campos affordance en ToolDescriptor: `prerequisites`, `follow_ups`, `required_args`, `limitations`. Catálogo actualiza 76 entries. DAG check sin ciclos. |
| 2026-10-12 | B3.W4 | `dde6f5b` | Cierra AAT-B3-02 (blake3 detecta edit con mtime+size preservados) y AAT-B3-03 (CacheStore per-workspace no comparte estado). 4 archivos: lib.rs + incremental/{mod,content_hash}.rs + tests/b3_w4_incremental_analysis.rs. 11 integration + 4 unit tests verdes. `bash scripts/test-fast.sh` exit 0. |
| 2026-10-12 | post-merge | `6d0d1e4` | clippy + fmt: Precision::default derive, parse_intent_args collapse, b3_w3 cycle check, b3_w4 git_in deref, find_usages parity. |
| 2026-10-12 | ratchet | `a547bf5`, `8619ad5` | dispatch-arm 11→12 (Inspect); coverage 49→53 + ANCHOR_SUITES += b3_w1..w4. |
| 2026-10-12 | doctest | `52e8522` | Inspect doc comment simplificado para no disparar rustdoc code parse. |
| 2026-10-12 | release | _pendiente_ | bump 0.101.12 → 0.101.13; golden CBOR regen. |
| 2026-10-12 | veredicto | _pendiente_ | per MERGE-GATE-BYPASS-POLICY §4. |
| 2026-10-12 | tag | v0.101.13 | anotado + pushed + GitHub Release con 3 tarballs. |

### B3 cierre

  - **Gate** `bash scripts/test-full.sh` rc=0 sobre el commit de
    bump (fmt, clippy `-D warnings`, cargo test --workspace,
    --doc, known_failures baseline coincide).
  - **Ratchet**: `core_gate_coverage_contract` cuenta pineada a
    53; ANCHOR_SUITES incluye b3_w1..b3_w4.
  - **Ratchet**: `cli_exit_code_propagation` cuenta pineada a 12
    (Inspect).
  - **Tag**: v0.101.13 anotado y pusheado a origin.
  - **GitHub Release**: `v0.101.13 — B3 Agent Code Understanding`
    con 3 tarballs (`cogh`, `cognicode`, `cognicode-mcp`) y sus
    SHA256.
  - **Veredicto**: `docs/roadmap/production-ready/veredictos/2026-10-12-b3-release-v0.101.13.md`
    per MERGE-GATE-BYPASS-POLICY §4.
  - **Pendiente explícito (B3.W5)**:
      - `cognicode inspect` queda PROPOSED; B3.W5 cierra el shape.
      - Per-entry `SymbolId` injection en collection DTOs
        (CallEntry, SymbolInfo, UsageEntry, ReferenceEntry,
        SymbolLocationEntry, TypeRefRecord).
      - Catálogo chain cross-validation: `follow_ups` DAG
        (B3.W3) + `intent_tags` (B3.W2) + `precision_floor`
        (B3.W1) son el input del reranker B4.W1 va a consumir
        para arrancar la capa incremental.
