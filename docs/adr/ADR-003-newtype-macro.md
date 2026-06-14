# ADR-003: Declarative Newtype Macro with Per-Type Derives

**Fecha:** 2026-06-11  
**Estado:** PROPOSED  
**Decisión:** Macro de atributo proc `#[newtype]` en `cognicode-macros`  
**Fuente:** auto-grill-loop Q008-P1, Q012-P2  

---

## Context

CogniCode necesita un patrón consistente para crear semantic newtypes (wrappers fuertemente tipados alrededor de tipos primitivos o estructurales). Actualmente, los newtypes se escriben a mano con contratos derive inconsistentes, y los tipos mirror schema/DTO duplican boilerplate entre dos capas.

## Decision

Usar una macro de atributo proc `#[newtype]` en `cognicode-macros` que:

- Auto-deriva: `Debug`, `Display`, `From`, `Serialize`, `Deserialize`
- Acepta derives opt-in via `#[newtype(derive(Clone, Copy, Eq, Ord, Hash, Default))]`
- Ruta los spans de error a través de `proc_macro2::Span` hacia el código usuario (no macros internals)
- Se testa via trybuild para casos compile-fail y snapshots de expansión

## Syntax

```rust
#[newtype]
#[newtype(derive(Clone, Eq, PartialEq, Hash))]
pub struct UserId(i64);
```

## Rationale

- **Attribute macro sobre derive macro:** `#[newtype]` se lee naturalmente en el struct; derive macros solo pueden agregar items, no reemplazar la definición del tipo
- **Opt-in derives sobre set estandarizado:** derives codifican semántica; agregar `Clone` ciegamente a un resource handle es incorrecto
- **Span hygiene con proc_macro2:** los errores de rustc deben apuntar al código usuario, no al código generado
- **trybuild:** patrón estándar de Rust para testar comportamiento de compilación de proc macros

## Consequences

- Los 24 pares mirror schema/DTO usan esta macro
- Nueva macro proc `#[newtype]` en el crate `cognicode-macros`
- Boilerplate de newtypes escritos a mano eliminado del codebase

## Alternatives Considered

- **newtype_derive / nutype crate:** rechazado — constriñe evolución a diseño del crate upstream; `cognicode-macros` ya existe
- **macro_rules! newtype!:** rechazado — no puede generar derives a nivel de atributo ni errores con Span-aware
- **Set de derives estandarizados:** rechazado — semánticamente incorrecto; derives son contratos, no boilerplate

## Validation

- [ ] Tests trybuild pasan para casos compile-fail y snapshots de expansión
- [ ] Los 24 pares schema/DTO compilan con la macro
- [ ] Los spans de error apuntan al código usuario, no al código generado
- [ ] La salida de `cargo expand` es legible e intencional
