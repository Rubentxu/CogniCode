# Spec: Fallback Provider — Use LightweightIndex

## Requirements
- `TreesitterFallbackProvider` debe aceptar `Arc<LightweightIndex>` como dependencia
- `get_definition()` debe usar `index.find_symbol(identifier)` para obtener candidatos
- Solo leer archivos que contengan definiciones del símbolo buscado
- Mantener verificación de "es definición" (fn/def/struct/class)
- Fallback a walkdir solo si el índice está vacío o no contiene el símbolo

## Scenarios
- Dado un índice con "greet" en src/main.rs:1, get_definition para "greet" solo lee src/main.rs
- Dado un índice vacío, get_definition usa walkdir como antes
- Dado un índice sin el símbolo, get_definition retorna None sin escanear disco
- Performance: de O(N) walkdir+read a O(1) lookup + O(1) reads

## Constraints
- `CompositeProvider` debe crear/inyectar el índice
- Constructor sin índice sigue funcionando (walkdir fallback)
- Tests existentes pasan
