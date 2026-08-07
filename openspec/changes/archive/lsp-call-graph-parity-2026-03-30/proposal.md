# Proposal: Close Gaps with LSP Call Graph

## Intent

Cerrar los gaps restantes entre CogniCode y el skill LSP call graph para tener una implementación completa y alineada.

## Gaps Identificados

### Gap 1: prepareCallHierarchy
LSP usa `prepareCallHierarchy` para obtener el item de call hierarchy de una función antes de pedir incoming/outgoing calls.

**Solución**: Añadir método `prepare_call_hierarchy(symbol_name)` que:
- Busca la ubicación exacta del símbolo
- Devuelve un `CallHierarchyItem` con file, line, column
- Es el paso requerido antes de hacer queries recursivas

### Gap 2: Recursive Expansion Automática
LSP expande recursivamente hasta depth N automáticamente.

**Solución**: Añadir `recursive_expand(item, max_depth)` que:
- Dado un CallHierarchyItem, obtiene incoming/outgoing calls
- Recursivamente expande cada resultado hasta max_depth
- Devuelve árbol completo de calls

### Gap 3: Visualización ASCII Interactiva
LSP tiene tree view y box diagram además de Mermaid.

**Solución**: Añadir `to_ascii_tree()` y `to_box_diagram()` que:
- Generan visualización ASCII del call graph
- Soportan múltiples niveles de profundidad
- Formato tipo "├──" "└──"

## Implementation

| Task | Description |
|------|-------------|
| 1 | Add `prepare_call_hierarchy` method |
| 2 | Add `CallHierarchyItem` struct |
| 3 | Add `recursive_expand` with auto-traversal |
| 4 | Add `to_ascii_tree()` for tree visualization |
| 5 | Add `to_box_diagram()` for box visualization |
| 6 | Expose via MCP tools |

## Success Criteria

- [ ] `prepare_call_hierarchy` returns exact location
- [ ] `recursive_expand` auto-traverses up to max_depth
- [ ] `to_ascii_tree` generates tree visualization
- [ ] `to_box_diagram` generates box visualization
