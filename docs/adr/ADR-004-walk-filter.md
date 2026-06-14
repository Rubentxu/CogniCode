# ADR-004: WalkFilter Value Object

**Fecha:** 2026-06-11  
**Estado:** PROPOSED  
**Decisión:** Value object WalkFilter en `domain/value_objects/walk_filter.rs`  
**Fuente:** auto-grill-loop Q005-P1  

---

## Context

SKIP_DIRS está duplicado 9 veces (no 5 como se pensó inicialmente) a través de 3 capas con 3 variantes no idénticas. Las duplicaciones viven en: handlers de análisis, utilidades de walk de filesystem, y constantes de seguridad. La ausencia de un value object centralizado causa inconsistencias y riesgo de seguridad cuando se agregan nuevas rules.

## Decision

Crear `domain/value_objects/walk_filter.rs` con:

- **WalkDecision enum:** `Include | Skip | Prune`
  - `Skip`: yield el path pero no descending
  - `Prune`: skip entire subtree
- **WalkFilter struct:** con composed builder pattern
  - `.with_security_blocklist()` — paths que NUNCA deben tocarse (`.git`, `.ssh`, `credentials`)
  - `.with_performance_skips()` — paths de build artifacts y caches (`target`, `node_modules`, `.venv`)
- **Tipo:** `fn(&Path) -> WalkDecision` (function pointer, zero-cost abstraction)

## Rationale

- Function pointer como tipo de filtro es el pattern más simple que funciona — no necesita trait objects
- Composed builder hace las dos fuentes de exclusión (security + performance) explícitas y ortogonales
- Ubicación en `domain/` sigue hexagonal architecture — es un concepto de dominio, no de infraestructura
- Elimina 9 bloques duplicados con una sola fuente de verdad

## Syntax

```rust
// Uso
let filter = WalkFilter::default()
    .with_security_blocklist()
    .with_performance_skips();

match filter.should_walk(path) {
    WalkDecision::Include => { /* walk */ }
    WalkDecision::Skip => { /* skip this, descend */ }
    WalkDecision::Prune => { /* skip and don't descend */ }
}
```

## Consequences

- 9 bloques SKIP_DIRS duplicados se consolidan en uno
- Los handlers de análisis reciben `WalkFilter` via inyección de dependencia
- Los tests de integración verifican que ningún path de seguridad sea skipped por performance

## Alternatives Considered

- **Trait objeto `WalkStrategy`:** rechazado — el set de estrategias es cerrado (security + performance); no necesita open-ended extensibility
- **Config-driven (YAML/JSON):** rechazado — overkill; los valores de SKIP_DIRS son constantes de código, no configuración de negocio
- **Single flat constant:** rechazado — mezcla responsabilidades de seguridad y performance

## Validation

- [ ] Todos los 9 sitios que usan SKIP_DIRS se actualizan a WalkFilter
- [ ] Los tests de seguridad verifican que `.ssh`, `.git/hooks` nunca sean incluidos
- [ ] Benchmark de walk de filesystem no regressa >5%
