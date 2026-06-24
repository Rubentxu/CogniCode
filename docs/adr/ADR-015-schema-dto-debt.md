# ADR-015: Schema/DTO Boundary — Violación Documentada

**Fecha:** 2026-06-15
**Estado:** ACCEPTED (con deuda explícita)
**Decisión:** Mantener los 10 re-exports de `application::dto` en `interface::mcp::schemas`; diferir la unificación hasta que se necesite.
**Fuente:** Revisión de C1-C6 (jun-15), estado real de C4

---

## Revisión 2026-06-24 (v0.12.6)

**Estado actual:** Aceptada — sigue sin haber beneficiario concreto.

**Verificación empírica (v0.12.6):**
- Los 10 re-exports siguen en `schemas.rs:11-20`
- Aparecen en 88+ sitios del MCP layer (`dto_mapping.rs`, `handlers/*.rs`, `file_ops_handlers.rs`, `mcp_roundtrip_tests.rs`)
- Cero cambios recientes (v0.12.0–v0.12.6) introducen divergencia wire-vs-DTO
- `mcp_roundtrip_tests.rs` sigue siendo el firewall válido

**Análisis de beneficiario:**
- v0.12.0 (ADR-045 Phase 1): removió `ExplorationPath`; no afecta los 10 re-exports
- v0.12.2 (ADR-039 reconcile): removió dead `chain` field; no afecta
- v0.12.3 (Type divergence): boundary ACL para `ViewDescriptor`/`ViewSpec` (separado de los 10)
- v0.12.6 (ADR-045 Debt 3): nueva tabla `exploration_sessions`; no toca los 10

**Conclusión:** La deuda sigue válida. ADR-015 reaffirmed. El refactor C4 (extraer los 10 tipos a `schemas.rs` con conversiones explícitas) sigue siendo 1-2 semanas de trabajo arriesgado, sin beneficiario concreto que lo justifique hoy.

**Trigger para reabrir C4:** Si en el futuro un DTO necesita divergir del wire format, ese es el momento natural para implementar C4.

---

## Context

ADR-001 (jun-11) propuso eliminar la importación de `application::dto` desde `interface::mcp::schemas`, definiendo que el wire format MCP debe ser propiedad de la capa de schemas, no de la capa de aplicación. La violación concreta (jun-15) está en `crates/cognicode-core/src/interface/mcp/schemas.rs` líneas 11-20:

```rust
pub use crate::application::dto::AnalysisMetadata;
pub use crate::application::dto::ContentMatch;
pub use crate::application::dto::EditValidation;
pub use crate::application::dto::FileEdit;
pub use crate::application::dto::FileEntry;
pub use crate::application::dto::FileMetadata;
pub use crate::application::dto::ListFilesResult;
pub use crate::application::dto::RiskLevel;
pub use crate::application::dto::SourceLocation;
pub use crate::application::dto::SyntaxIssue;
```

**Por qué importa:** Esta violación rompe la disciplina de capas de Clean Architecture. Cambios en la forma serializada de un DTO se propagan al wire format MCP, y viceversa, sin firewall de compilación. ADR-002 declaró que "C4 gates C1" — la existencia de `#[cognicode_macros::aix_tool]` (C1) sin C4 limpio es un smell arquitectónico.

**Por qué se mantiene:** Eliminar la duplicación requiere definir los 10 tipos en `schemas.rs` (o en un módulo `wire_format/`), con sus propias impls serde (probablemente idénticas a las de los DTOs), y agregar conversiones explícitas DTO → wire en cada handler que los usa. Es 1-2 semanas de trabajo arriesgado, sin un beneficiario concreto identificado.

**Por qué no es urgente:** Ningún DTO está cambiando su forma serializada en el corto plazo. La cobertura de tests de MCP round-trip es alta. El riesgo de regresión por esta duplicación es bajo, no alto.

## Decision

**Aceptar la violación como deuda arquitectónica explícita**, con las siguientes condiciones:

1. **No expandir la frontera rota**: ningún DTO adicional debe re-exportarse desde `schemas.rs` sin justificación.
2. **Tests de round-trip son el firewall**: los tests `mcp_roundtrip_tests.rs` validan que el wire format no cambia accidentalmente. Mantenerlos verdes.
3. **Revisar si surge un beneficiario concreto**: si en el futuro un DTO necesita divergir del wire format, ese es el momento natural para implementar C4 (extraer ese DTO a `schemas.rs`).

## Rationale

- **YAGNI aplicado correctamente**: no construir una abstracción (los 10 newtypes vía `#[newtype]`) hasta que haya un caso concreto que la justifique.
- **Coste/beneficio desfavorable**: 1-2 semanas de refactor riesgoso vs. ningún cambio de comportamiento observable.
- **Tests cubren el riesgo real**: si el wire format cambia, los tests fallan, no hay regresión silenciosa.
- **ADR-003 (`#[newtype]` macro) existe pero no se usa**: la herramienta está disponible cuando se decida ejecutar C4.

## Alternatives Considered

- **Eliminar la violación ahora (extraer los 10 tipos)**: rechazado — coste/beneficio desfavorable; ningún cambio planeado lo necesita.
- **Volver a C1-C6 "deferrred" indefinidamente**: rechazado — C1 ya está implementado de facto, C3 se acaba de consolidar, C5 ya estaba hecho, C2 ya estaba hecho, C6 ya estaba hecho. La tabla "DEFERRED" del roadmap era engañosa.
- **Archivar ADR-001-006 sin más análisis**: rechazado — C4 es la única que queda con trabajo real, documentarla como deuda es más honesto que ignorarla.

## Consequences

- ADR-001-006 marcados como "ARCHIVED" en el roadmap (decisión jun-15).
- ADR-015 documenta la única decisión arquitectónica pendiente: la frontera schema/DTO.
- Si en el futuro alguien necesita divergencia wire vs DTO, este ADR es el punto de partida para reabrir C4.

## Validation

- [x] Los 10 re-exports están documentados como deuda explícita
- [x] Tests de round-trip pasan (`mcp_roundtrip_tests.rs` validados jun-15)
- [x] No se agregaron nuevos re-exports desde la última auditoría
- [ ] Revisar si surge beneficiario concreto (futuro)
