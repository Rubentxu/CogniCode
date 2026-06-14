# CONTEXT.md Patch Candidates — Architecture Deepening (2026-06-11)

## New Domain Terms

### WalkFilter
Un value object de dominio (`domain/value_objects/walk_filter.rs`) que decide qué paths del filesystem deben ser excluidos durante el análisis estático. Compone dos fuentes de exclusión independientes:
- **Security blocklist** (BLOCKED_DIRS): paths que NUNCA deben ser tocados (`.git`, `.ssh`, directorios de secrets)
- **Performance skips** (SKIP_DIRS): paths de build artifacts y caches que pueden ser ignorados para velocidad (`target`, `node_modules`, `.venv`)
- **WalkDecision**: `Include` | `Skip` (yield but don't descend) | `Prune` (skip entire subtree)
- Tipo: `fn(&Path) -> WalkDecision` (function pointer, zero-cost)

### Schema/DTO Boundary
La capa de schemas (`interface/mcp/schemas.rs`) define los tipos de wire-format para el protocolo MCP. La capa de DTOs (`application/dto/`) define objetos de transferencia internos. La regla de frontera: **schemas.rs NO debe importar de `application::dto`**. Si un tipo necesita ser compartido, pertenece a schemas y los DTOs lo consumen, no al revés. La unificación se hace via `#[newtype]` attribute macro, no con type aliases.

### ReadMode
Enum cerrado con 4 variantes (`Raw`, `Outline`, `Symbols`, `Compressed`) que define cómo `file_operations::read_file` procesa el contenido. Dispatch via `match` exhaustivo con static dispatch. No es un trait extensible — es una decisión de producto, no un punto de extensión.

## Dependency Rules

### Candidate Execution Order (5 Waves)
Los 6 candidatos de profundización tienen dependencias estructurales documentadas en `docs/adr/drafts/DRAFT-candidate-execution-order.md`:
1. Wave 1: C3 (WalkFilter) + C5 (ReadMode) + C6 (Mock crate) — independientes, paralelizables
2. Wave 2: C2 Builder — aditivo, sin riesgo
3. Wave 3: C4 (Schema/DTO Unification) — prerequisite para C1
4. Wave 4: C2 ContextGraphStore deletion — limpieza post-Builder
5. Wave 5: C1 (Tool Registry `#[aix_tool]`) — último, depende de C4
