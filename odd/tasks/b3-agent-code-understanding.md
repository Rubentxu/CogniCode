# B3 — Agent Code Understanding (v0.101.13)

> **Estado**: ACTIVE. Desbloqueado por B2 (motor compartido + catálogo unificado).
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

- [ ] **B3.W1** — Navegación semántica unificada. Consolidar 8 tools
  (`get_file_symbols`, `query_symbol_index`, `go_to_definition`,
  `find_references`, `find_usages`, `get_call_hierarchy`,
  `get_implementors`, `get_type_references`). Identidades no ambiguas
  para símbolos homónimos. NO devolver AST heurístico como si tuviera
  garantía LSP.
- [ ] **B3.W2** — Contexto progresivo. Aprovechar `codebase_map`,
  `smart_search`, `get_symbol_code`, `read_file`, `search_content`,
  `retrieve_and_verify`. Una consulta inicial debe conducir a fragmentos
  relevantes sin que el agente conozca todos los nombres. Evaluar
  `cognicode inspect` / `cognicode context` (contrato, compatibilidad,
  necesidad real).
- [ ] **B3.W3** — Descubrimiento orientado a agentes. Extender catálogo
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
| 2026-10-12 | B3.W2 | `146f9da` + `d8cad0a` | Cierra UAT-B3-04 (cadena de lectura paginada reconstruye original). 6 tools composables (`codebase_map`, `smart_search`, `get_symbol_code`, `read_file`, `search_content`, `retrieve_and_verify`) enrutan via `Intent::{Locate,Summarize,Verify,Read}` -> `Vec<ToolRef>`. Scaffold `cognicode inspect` (PROPOSED, B3.W5 cierra shape). Catálogo lleva `intent_tags` por tool. 11 unit + 12 integration tests verdes. `bash scripts/test-fast.sh` exit 0. Archivos: crates/cognicode-core/src/interface/{intent.rs, catalog.rs (intent_tags), cli/commands.rs (Inspect), mod.rs (intent)}, crates/cognicode-core/tests/b3_w2_progressive_context.rs, product/tools.json. |
| _pendiente_ | B3.W3 | _pendiente_ | _pendiente_ |
| 2026-10-12 | B3.W4 | `dde6f5b` | Cierra AAT-B3-02 (blake3 detecta edit con mtime+size preservados) y AAT-B3-03 (CacheStore per-workspace no comparte estado). 4 archivos: lib.rs + incremental/{mod,content_hash}.rs + tests/b3_w4_incremental_analysis.rs. 11 integration + 4 unit tests verdes. `bash scripts/test-fast.sh` exit 0. |
