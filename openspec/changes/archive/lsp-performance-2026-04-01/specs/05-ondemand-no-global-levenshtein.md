# Spec: OnDemandGraphBuilder — Avoid Global Levenshtein

## Requirements
- `find_related_files()` debe buscar coincidencias exactas y prefix primero (O(1) via HashMap)
- Fuzzy matching solo sobre subconjunto candidato (símbolos que comparten prefijo >= 2 chars)
- Evitar allocación de matriz en levenshtein cuando no es necesario

## Scenarios
- Símbolo "build_project_graph" encuentra coincidencia exacta en índice: 0 levenshtein calls
- Símbolo "bld_proj" sin coincidencia exacta: busca prefix "bl" en índice, fuzzy solo sobre coincidencias
- Índice con 1000 símbolos: máximo ~50 levenshtein calls en vez de 1000

## Constraints
- API pública no cambia
- Tests existentes pasan
