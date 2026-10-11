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
- [ ] **B3.W4** — Análisis incremental fiable. Invalidación dependiente
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
| _pendiente_ | B3.W1 | _pendiente_ | _pendiente_ |
| _pendiente_ | B3.W2 | _pendiente_ | _pendiente_ |
| _pendiente_ | B3.W3 | _pendiente_ | _pendiente_ |
| _pendiente_ | B3.W4 | _pendiente_ | _pendiente_ |
