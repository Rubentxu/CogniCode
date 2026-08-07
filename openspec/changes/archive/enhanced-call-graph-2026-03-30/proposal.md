# Proposal: Enhanced Call Graph LSP Features

## Intent

Implementar funcionalidades inspiradas en LSP call hierarchy para CogniCode: depth traversal real, visualización Mermaid, detección de hot paths, metrics de complejidad, y exposición de entry points/leaf functions via MCP.

## Scope

### In Scope
- `get_call_hierarchy` con depth traversal recursivo real (no solo 1 nivel)
- Soporte para direction `incoming` (callers) con depth traversal
- Nuevos MCP tools: `get_entry_points`, `get_leaf_functions`
- Visualización Mermaid export para call graphs
- `trace_execution_path` - mostrar camino entre dos funciones
- Análisis de hot path (funciones más llamadas)
- Métricas de complejidad (fan-in, fan-out, nesting depth)

### Out of Scope
- Visualización ASCII interactiva (futuro)
- Integration con LSP real (solo simulación local)
- Persistencia de call graphs en disco

## Approach

1. Extender `CallGraph` aggregate con métodos para metrics y traversal
2. Crear `CallGraphAnalyzer` domain service para análisis avanzado
3. Añadir MCP handlers para nuevos tools
4. Implementar visualización Mermaid usando templates
5. Tests de integración con código real del proyecto

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/domain/aggregates/call_graph.rs` | Modified | Añadir methods para metrics, traversal con depth |
| `src/domain/services/impact_analyzer.rs` | Modified | Reusar para hot path analysis |
| `src/application/services/analysis_service.rs` | Modified | Añadir methods para entry points, leaf functions |
| `src/interface/mcp/handlers.rs` | Modified | Nuevos handlers para tools |
| `src/interface/mcp/schemas.rs` | Modified | Nuevos input/output schemas |
| `src/domain/services/cycle_detector.rs` | Reference | Reusar para complexity metrics |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Depth recursion causing stack overflow | Low | Limitar depth máximo a 10 |
| Performance en proyectos grandes | Medium | Cachear resultados, async |

## Rollback Plan

Eliminar los nuevos handlers de `handlers.rs`, revertir cambios en `schemas.rs`, y remover métodos añadidos a `call_graph.rs`. Ningún cambio es destructivo.

## Dependencies

- tree-sitter-parser ya funciona para extraer calls
- Graph cache ya existe para caching

## Success Criteria

- [ ] `get_call_hierarchy` funciona con depth > 1 y direction=incoming
- [ ] `get_entry_points` retorna todos los símbolos sin incoming edges
- [ ] `get_leaf_functions` retorna todos los símbolos sin outgoing edges
- [ ] `trace_path` muestra el camino entre main y cualquier función
- [ ] Hot path detection identifica funciones con mayor fan-in
- [ ] Mermaid export genera diagramas válidos
- [ ] Todos los tests pasan (212+)
