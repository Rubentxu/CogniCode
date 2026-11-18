# SEAM — WorkspaceSession (medicion, no opinion)

- Herramienta: `python3 scripts/measure_workspace_session.py --write`
- HEAD medido: `7f5ce7d4` — fichero: `crates/cognicode-core/src/application/workspace_session.rs` (4584 lineas)
- Regions `#[cfg(test)]` inline: 2; ficheros `tests/` leidos: 397

La regla de extraccion (CGH-010 AC#2): un grupo solo se extrae
cuando este instrumento lo marca READY: cada metodo publico tiene
al menos un punto de caracterizacion y sus ayudantes privados no
estan compartidos con metodos de fuera. Lo demas es opinion.

| grupo | metodos | LOC | crates consumidores | puntos de llamada | sin caracterizar | ayudantes compartidos | veredicto |
|---|---|---|---|---|---|---|---|
| lifecycle | 2 | 52 | 1 | 5 | — | — | READY |
| graph store | 6 | 104 | 0 | 0 | require_graph, ensure_graph_built | — | NO EXTRAER AUN |
| indexing | 4 | 238 | 0 | 0 | — | ensure_graph_built, ensure_semantic_search, held_graph, require_graph | NO EXTRAER AUN |
| files | 5 | 41 | 1 | 50 | resolve_path | — | NO EXTRAER AUN |
| search | 6 | 61 | 1 | 18 | semantic_search, map_kind_strings, map_kind_string | — | NO EXTRAER AUN |
| symbol queries | 7 | 163 | 2 | 21 | get_outline, get_complexity, ranked_symbols_ai | ensure_graph_built, get_hot_paths, require_graph, resolve_path | NO EXTRAER AUN |
| graph queries | 9 | 238 | 2 | 11 | get_call_hierarchy, export_mermaid | ensure_graph_built, held_graph, require_graph | NO EXTRAER AUN |
| architecture | 1 | 62 | 1 | 1 | — | ensure_graph_built, require_graph | NO EXTRAER AUN |
| refactoring | 6 | 224 | 2 | 12 | refactor_basis, move_symbol, extract_function, validate_syntax | resolve_path | NO EXTRAER AUN |
| lsp surface | 4 | 70 | 2 | 20 | go_to_definition, hover, find_references | resolve_path | NO EXTRAER AUN |
| agent insight | 5 | 277 | 0 | 0 | smart_overview, auto_diagnose, suggest_onboarding_plan, ask_about_code | check_architecture, ensure_graph_built, get_graph_stats, get_hot_paths, held_graph, query_symbol_index, require_graph, trace_path | NO EXTRAER AUN |

## Lectura

- Grupos READY para extraer hoy: lifecycle.
- Resto: sin caracterizacion suficiente o con ayudantes compartidos
  que hay que mover primero; extraerlos ahora romperiamos la
  fachada publica con un cambio que ningun test notaria.

