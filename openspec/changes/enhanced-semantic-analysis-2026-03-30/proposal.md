# Proposal: Enhanced Semantic Analysis

## Intent

Implementar funcionalidades de análisis semántico de alta velocidad inspiradas en LSP Code Analysis: outline jerárquico, symbol code retrieval, search con filtros, y context lines. Todo optimizado para IA agents.

## Scope

### 1. Hierarchical Outline
- Estructura completa: archivo → modules → classes → methods → properties
- Información de tipos y signatures
- Navegación rápida sin parsing completo

### 2. Symbol Code Retrieval  
- Obtener código completo de un símbolo
- Incluir docstrings y comments
- Rápido con caching

### 3. Search with Filters
- Filtrar por kind: function, class, method, variable, trait, struct, enum
- Búsqueda fuzzy para typos
- Resultados ordenados por relevance

### 4. Context Lines
- Añadir N líneas de contexto en references
- Configurable por usuario
- Evitar "where is this used?"

## Technical Approach

### Crates a usar:
- `symbolicar` - para parsing semántico rápido (similar a LSP)
- `rowan` - para AST manipulation
- `dashmap` - para cache concurrente
- `smol_str` - para strings interning

### Performance Target:
- Outline: < 10ms para archivo de 1000 líneas
- Symbol retrieval: < 5ms
- Search: < 50ms para 10k archivos
- Context lines: < 10ms

## Success Criteria

- [ ] Outline jerárquico funcional
- [ ] Symbol retrieval con código completo
- [ ] Search con filtros por kind
- [ ] Context lines en references
- [ ] Tests y benchmarks
