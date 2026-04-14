# Proposal: Test Coverage to 80%

## Intent
Llevar la cobertura de tests unitarios al 80% mínimo. Actualmente ~21% del código (6,036 líneas en 58 archivos) no tiene tests. Se priorizan los archivos más grandes y de mayor riesgo.

## Scope
### Archivos a cubrir (priorizados por tamaño)
**Alta prioridad (>200 líneas, sin tests):**
- `interface/mcp/handlers.rs` (2107 líneas) — NO cubrir (integration, requiere LSP)
- `interface/cli/commands.rs` (1010) — NO cubrir (integration, requiere LSP)
- `infrastructure/lsp/providers/lsp.rs` (275) — Tests unitarios sin LSP
- `infrastructure/lsp/process.rs` (271) — Tests unitarios sin LSP
- `domain/traits/search_provider.rs` (264) — Tests de mock implementers
- `domain/traits/refactor_strategy.rs` (248) — Tests de mock
- `domain/traits/code_intelligence.rs` (219) — Tests de mock

**Media prioridad (100-200 líneas):**
- `infrastructure/vfs/virtual_file_system.rs` (137) — Tests de get/set/apply
- `domain/traits/parser.rs` (120) — Tests de mock
- `application/commands/refactor_commands.rs` (119) — Tests de conversión
- `application/dto/refactor_dto.rs` (111) — Tests de DTOs
- `infrastructure/lsp/error.rs` (110) — Tests de error types
- `application/error.rs` (108) — Tests de AppError

**Baja prioridad (<100 líneas):**
- `infrastructure/parser/ast_scanner.rs` (96) — Tests de scan
- `infrastructure/graph/graph_cache.rs` (93) — Tests de cache
- `application/dto/impact_dto.rs` (76) — Tests de DTOs
- `domain/traits/dependency_repository.rs` (72) — Tests de mock
- `application/dto/symbol_dto.rs` (68) — Tests de DTOs
- `domain/traits/file_system.rs` (58) — Tests de mock
- `infrastructure/lsp/protocol.rs` (40) — Tests de protocol types

## Not covered (integration, requiere LSP/processes)
- handlers.rs, commands.rs (CLI) — requieren entorno completo
- bin/lsp_server.rs, bin/mcp_server.rs — integration tests
