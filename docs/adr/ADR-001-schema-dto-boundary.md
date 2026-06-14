# ADR-001: Schema/DTO Boundary Enforcement

**Fecha:** 2026-06-11  
**Estado:** PROPOSED  
**Decisión:** 5-wave execution order  
**Fuente:** auto-grill-loop Q001-P1, Q008-P1  
**Confianza:** alta  

---

## Context

El codebase de CogniCode tiene dos capas de tipos que overlapped para transferencia de datos MCP:

- `cognicode-core/src/interface/mcp/schemas.rs` — tipos de wire-format
- `cognicode-core/src/application/dto/` — objetos de transferencia internos

Estas capas crecieron independientemente con ~5400 líneas de tipos casi duplicados y un archivo puente `dto_mapping.rs` que es código muerto (cero callers en producción). Actualmente `schemas.rs` importa directamente desde `application::dto`, rompiendo la disciplina de capas de Clean Architecture.

## Decision

1. **Schemas owners el wire format.** DTOs son objetos de transferencia internos. Regla de frontera: `schemas.rs` NO DEBE importar desde `application::dto`.

2. **Newtypes, no aliases.** Los 24 pares mirror schema/DTO usan newtypes generados por una macro de atributo `#[newtype]`. Sin type aliases — 87.5% de los pares difieren semánticamente en sus contratos derive/serde.

3. **Derives por tipo.** Cada newtype declara sus derives explícitamente. Sin set estandarizado — los derives codifican semántica, no boilerplate.

4. **BuildGraphInput pertenece a schemas.rs.** Los 53 sitios de llamada en handlers/mod.rs deben actualizarse cuando se mueva.

## Rationale

- Type aliases acoplan silenciosamente el wire format MCP a tipos internos
- Newtypes proveen un firewall de compilación — la evolución de MCP no filtra a consumidores internos
- 87.5% de los pares mirror ya difieren en derives, atributos serde, o tipos anidados
- La macro `#[newtype]` elimina boilerplate mientras preserva control semántico por tipo

## Alternatives Considered

- **Type aliases donde son idénticos:** rechazado — aliases borran el límite MCP/domain, arriesgando acoplamiento silencioso
- **Derives estandarizados:** rechazado — derives codifican semántica; homogeneización a ciegas es un anti-pattern
- **Crate separado `cognicode-serde`:** rechazado — YAGNI; serde ya es dependencia del workspace

## Consequences

- `schemas.rs` elimina su import de `application::dto`
- `dto_mapping.rs` se elimina (código muerto)
- Nueva macro `#[newtype]` en `cognicode-macros`
- C4 (Schema/DTO Unification) se convierte en prerequisito de Wave 3 para C1 (Tool Registry)

## Validation

- [ ] Los 24 pares newtype compilan con la macro `#[newtype]`
- [ ] `schemas.rs` tiene cero imports de `application::dto`
- [ ] `dto_mapping.rs` se elimina sin romper el build
- [ ] El wire format MCP round-trippea correctamente (serde sin cambios)
