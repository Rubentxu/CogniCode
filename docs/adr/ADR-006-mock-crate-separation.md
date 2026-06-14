# ADR-006: Mock Crate Separation

**Fecha:** 2026-06-11  
**Estado:** PROPOSED  
**Decisión:** Crate separado `cognicode-core-mock` con versionado lockstep  
**Fuente:** auto-grill-loop Q006-P1  

---

## Context

Los domain traits de CogniCode (`~370 líneas`) tienen implementaciones mock inline en el propio trait file o en blocks `#[cfg(test)]`. `mockall` está en `Cargo.toml` pero deliberadamente sin usar — el equipo prefiere mocks escritos a mano. Esto crea tensión entre la necesidad de mocks para testing y la pureza del código de producción.

## Decision

Crear un crate separado `cognicode-core-mock`:

- **Ubicación:** `crates/cognicode-core-mock/`
- **Versionado:** lockstep con `cognicode-core` (misma versión menor)
- **Contenido:** todos los mocks de domain traits exportados desde `domain::testing::mocks`
- **Re-export:** `pub use cognicode_core::domain::traits::*` para backwards compatibility
- **Sin feature flags** en el crate de producción — los tests del crate core usan `#[cfg(test)]` para sus propios mocks internos

## Crate Structure

```
crates/cognicode-core-mock/
├── Cargo.toml        # version = "0.x.0" (match cognicode-core)
├── src/
│   └── lib.rs       # re-exports + mock implementations
└── tests/
    └── integration  # mocks contra domain traits
```

## Rationale

- **Separación limpia:** mocks viven en su propio crate; el crate de producción no contiene código de test
- **Lockstep versioning:** `cognicode-core-mock v0.3.1` siempre corresponde a `cognicode-core v0.3.1` — imposible desincronización
- **mockall disponible pero no requerido:** el crate puede usar `mockall` internamente si el equipo lo decide en el futuro
- **Sin feature flags:** feature flags en crates de producción filtran preocupaciones de testing al binary

## Consequences

- Nuevo crate `cognicode-core-mock` en el workspace
- Tests de integración de otros crates pueden depender de `cognicode-core-mock`
- Los `#[cfg(test)]` mocks internos del crate core se mantienen como están
- Workspace `Cargo.toml` agrega `cognicode-core-mock` como miembro

## Alternatives Considered

- **mockall con feature flag `mock`:** rechazado — leaky abstraction; el feature flag expone preocupación de testing en el binary de producción
- **Mocks inline en domain traits con `#[cfg(test)]`:** rechazado — viola single responsibility; el trait file se convierte en test harness
- **Mocks en `infrastructure/testing/` del mismo crate:** rechazado — aún viviría en el crate de producción

## Validation

- [ ] `cognicode-core-mock` compila independientemente
- [ ] Tests de integraciónusan `cognicode-core-mock` sin cambios en el crate core
- [ ] Lockstep versioning verificado en CI: la versión de mock-core == versión de core
