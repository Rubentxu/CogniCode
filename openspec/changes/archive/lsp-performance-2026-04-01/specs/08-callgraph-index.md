# Spec: CallGraph Auxiliary Index

## Requirements
- Añadir índice auxiliar `name_index: HashMap<String, Vec<SymbolId>>` que mapea base_name (lowercase) a lista de SymbolIds
- Actualizar `add_symbol()` para mantener el índice sincronizado
- Actualizar `remove_symbol()` para limpiar entradas del índice
- `dependents()` debe usar el índice auxiliar en vez de recorrer todos los símbolos con contains()
- `find_all_dependents()` debe usar el índice auxiliar en vez de contains() en cada nivel BFS
- `find_symbol_id_by_name()` nuevo método público que use el índice

## Scenarios
- Dado un CallGraph con símbolos "foo:1:0" y "foo_bar:2:0", buscar "foo" retorna ambos IDs
- `dependents()` con ID exacto usa reverse_edges directamente (sin cambio de comportamiento)
- `dependents()` con ID no encontrado usa name_index en vez de contains() sobre todos los símbolos
- Performance: lookup por nombre es O(1) amortizado en vez de O(S)

## Constraints
- No cambiar la API pública existente (solo añadir métodos)
- Mantener backward compatibility con SymbolId como String-based
- Tests existentes deben pasar sin modificación
