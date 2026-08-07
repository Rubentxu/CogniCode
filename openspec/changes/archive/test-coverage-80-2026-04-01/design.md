# Design: Test Coverage to 80%

## Batch 1 — Traits y mocks (4 agents, ~900 líneas)
Los traits son fáciles de testear creando implementaciones mock.

### D1: search_provider.rs (264 líneas)
- Crear MockSearchProvider que implementa SearchProvider
- Test de get_definition, hover, find_references, get_symbols
- Test de error handling

### D2: refactor_strategy.rs (248 líneas)
- Crear MockRefactorStrategy que implementa RefactorStrategy
- Test de validate/prepare_edits/execute con mock returns
- Test de error paths

### D3: code_intelligence.rs (219 líneas)
- Crear MockCodeIntelligence que implementa CodeIntelligence
- Test de get_definition, hover, find_references, get_hierarchy, find_symbols

### D4: parser.rs + file_system.rs + dependency_repository.rs (250 líneas combinadas)
- MockParser que implementa Parser trait
- MockFileSystem que implementa FileSystem
- Test de cada método del trait

## Batch 2 — DTOs y errores (3 agents, ~500 líneas)
Los DTOs y tipos de error son puramente de datos — tests de creación y conversión.

### D5: refactor_dto.rs + symbol_dto.rs + impact_dto.rs (255 líneas)
- Test de from_symbol, constructors, Display
- Test de conversión entre tipos

### D6: error.rs (app) + lsp/error.rs (218 líneas)
- Test de AppError variants, Display, Error trait
- Test de LspError variants, From conversions

### D7: refactor_commands.rs (119 líneas)
- Test de conversión SymbolKind → SearchSymbolKind
- Test de to_refactor_kind y helpers

## Batch 3 — Infrastructure sin LSP (3 agents, ~600 líneas)
Estos módulos pueden testearse sin un LSP server real.

### D8: graph_cache.rs (93 líneas)
- Test de new, get, set, update, apply_events
- Test de clear, queue_event, flush_events

### D9: virtual_file_system.rs (137 líneas)
- Test de get_content, set_content, apply_edits
- Test de edge cases (missing files, empty content)

### D10: ast_scanner.rs (96 líneas)
- Test de scan, find_nodes_by_type
- Test con source Rust y Python
- Test de edge cases (empty source, invalid syntax)

### D11: lsp/providers/lsp.rs + lsp/process.rs + protocol.rs (586 líneas)
- Test de LspProvider con mocks (sin LSP real)
- Test de LspProcess initialization
- Test de protocol types (Position, Range, etc.)
