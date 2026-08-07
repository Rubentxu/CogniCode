# Spec: LightweightIndex — Zero-Copy Lookups + File Index

## Requirements
- `find_symbol()` debe devolver `&[SymbolLocation]` (zero-copy) en vez de `Vec<SymbolLocation>` (clone)
- Añadir `file_index: HashMap<String, Vec<usize>>` que mapea file_path → lista de índices en index entries
- `find_in_file()` usa file_index en vez de recorrer todo el índice
- Parser pool: reusar TreeSitterParser por lenguaje en build_index/build_from_sources

## Scenarios
- find_symbol("foo") retorna referencia sin allocar Vec
- find_in_file("src/main.rs") es O(1) lookup en vez de O(N) scan
- build_index crea 1 parser por lenguaje máximo

## Constraints
- Cambiar find_symbol retorno de Vec a &[SymbolLocation] puede romper callers — auditar todos los usos
- Tests existentes pasan
