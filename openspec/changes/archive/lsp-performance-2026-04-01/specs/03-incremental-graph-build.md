# Spec: Incremental build_project_graph

## Requirements
- Parser pool: reusar TreeSitterParser por lenguaje en vez de crear uno por archivo
- File cache: cachear resultados por archivo usando (path, mtime) como clave
- Unificar pasadas: combinar find_all_symbols + find_call_relationships en una sola pasada del AST
- Solo reparsear archivos cuyo mtime haya cambiado

## Scenarios
- Segunda llamada a build_project_graph con archivos sin cambios: 0 reparseos
- Un archivo cambia: solo ese archivo se reparsea
- Parser pool: máximo 1 parser por lenguaje, reusado entre archivos del mismo tipo

## Constraints
- API pública `build_project_graph()` no cambia
- Resultado idéntico al actual (graph completo)
- Tests existentes pasan
