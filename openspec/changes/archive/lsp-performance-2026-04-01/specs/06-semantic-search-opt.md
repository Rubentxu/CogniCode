# Spec: SemanticSearch Precomputed Index + Top-K

## Requirements
- `IndexedSymbol` debe incluir campo `name_lower: String` precomputado en indexación
- `search()` debe usar `indexed.name_lower` en vez de calcular `to_lowercase()` por iteración
- Usar `BinaryHeap` para top-k parcial en vez de `sort()` + `truncate()`
- Solo construir `Symbol` para los resultados finales (no para todos los candidatos)

## Scenarios
- Indexar 1000 símbolos y buscar "foo": 0 llamadas a to_lowercase() en el loop de búsqueda
- Buscar con max_results=10 sobre 500 resultados: solo 10 Symbol objects creados
- Resultados ordenados correctamente: Exact > Prefix > Contains > Fuzzy

## Constraints
- API pública no cambia
- Tests existentes pasan sin modificación
