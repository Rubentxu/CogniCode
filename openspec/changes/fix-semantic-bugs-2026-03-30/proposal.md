# Proposal: Fix Semantic Analysis Bugs

## Intent

Corregir los 4 bugs en la implementación de semantic analysis para dejar el código production-ready.

## Bugs Identificados

### Bug 1: JavaScript outline no encuentra métodos en clases
- **File**: `outline.rs`
- **Test**: `test_javascript_outline`
- **Issue**: `class_declaration` no está mapeado correctamente, o los children no se procesan
- **Fix**: Añadir `class_declaration` al mapping y verificar que el body se procesa

### Bug 2: Python docstring extraction falla
- **File**: `symbol_code.rs`
- **Test**: `test_extract_python_docstring`
- **Issue**: El parser no encuentra docstrings de Python
- **Fix**: Verificar que el comment extraction funciona para Python

### Bug 3: Rust docstring extraction falla
- **File**: `symbol_code.rs`
- **Test**: `test_extract_rust_docstring`
- **Issue**: El parser no encuentra docstrings de Rust (/// o /** */)
- **Fix**: Verificar que el comment extraction funciona para Rust

### Bug 4: Single line comment extraction falla
- **File**: `symbol_code.rs`
- **Test**: `test_extract_single_line_comment`
- **Issue**: El parser no encuentra comentarios de una línea
- **Fix**: Implementar búsqueda hacia atrás de comentarios

## Tasks

1. Fix `outline.rs`: añadir `class_declaration` y verificar child processing
2. Fix `symbol_code.rs`: docstring extraction para Python
3. Fix `symbol_code.rs`: docstring extraction para Rust  
4. Fix `symbol_code.rs`: single line comment extraction
5. Verificar todos los tests pasan
6. Benchmark de performance

## Success Criteria

- [ ] `test_javascript_outline` pasa
- [ ] `test_extract_python_docstring` pasa
- [ ] `test_extract_rust_docstring` pasa
- [ ] `test_extract_single_line_comment` pasa
- [ ] Todos los tests de semantic pasan
- [ ] Build sin errores
