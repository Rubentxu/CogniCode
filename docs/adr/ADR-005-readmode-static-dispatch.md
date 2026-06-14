# ADR-005: ReadMode Static Dispatch

**Fecha:** 2026-06-11  
**Estado:** PROPOSED  
**Decisión:** Enum con dispatch estático para ReadMode  
**Fuente:** auto-grill-loop Q004-P1  

---

## Context

`file_operations.rs` (3226 líneas) tiene dispatch de ReadMode via múltiples puntos de branching. El enum `ReadMode` ya existe en `dto/file_ops.rs` con 4 variantes. La pregunta era si usar dispatch estático (enum + match) o dinámico (trait objects).

## Decision

Usar **enum con dispatch estático:**

- ReadMode es un enum cerrado con 4 variantes: `Raw`, `Outline`, `Symbols`, `Compressed`
- Dispatch via `match` exhaustivo en `file_ops_handlers.rs`
- Sin trait objects — el conjunto es cerrado y conocido en compile time
- ReadMode se mueve/refactoriza según el plan de C5 (Wave 1)

## Syntax

```rust
match mode {
    ReadMode::Raw => read_file_raw(path)?,
    ReadMode::Outline => read_file_outline(path)?,
    ReadMode::Symbols => read_file_symbols(path)?,
    ReadMode::Compressed => read_file_compressed(path)?,
}
```

## Rationale

- **Static dispatch es más rápido:** sin indirectión vía vtable, el compilador puede inlinear y vectorizar
- **Closed set:** no hay necesidad de open-ended extensibility — los 4 modos son el conjunto completo
- **Exhaustiveness checking:** el compilador强制 exhaustividad del match, previniendo casos olvidados cuando se agrega un nuevo modo
- **Sin heap allocation:** trait objects requieren `Box<dyn>`, heap allocation, y dynamic dispatch overhead

## Consequences

- `file_operations.rs` refactoriza su dispatch para usar match estático
- Los 4 ReadMode handlers se especializan sin overhead de virtual call
- Si se necesita un 5to modo en el futuro, es un cambio de enum — review explícito garantizado

## Alternatives Considered

- **Trait objects `ReadModeStrategy`:** rechazado — runtime overhead de vtable dispatch; el set es cerrado, no extensible por diseño
- **Function pointers:** rechazado — menos expresivo que enum para 4 variantes con datos asociados

## Validation

- [ ] Los 4 modos compilan y pasan tests existentes
- [ ] El enum es verdaderamente cerrado (ningún `#[non_exhaustive]`)
- [ ] Benchmark muestra zero overhead de dispatch vs implementación actual
