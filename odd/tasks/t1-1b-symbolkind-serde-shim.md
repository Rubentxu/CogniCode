# Goal — T1-1b SymbolKind schemas newtype + serde shim lowercase

> **Estado**: PENDING (autorizado en el goal `v1015-consolidation-and-promote`,
> subdividido de T1-1 original tras el variant mismatch `Type` vs `TypeAlias`).
> Bounded ~2h, bajo riesgo, independiente de T1-1c.

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `odd/tasks/v1015-consolidation-and-promote.md` §T1-1 | subdivisión que rechazó la consolidación monolítica y partió en T1-1a/b/c |
| `crates/cognicode-core/src/interface/mcp/schemas.rs` | **target** — la capa MCP que necesita un newtype con shim serde |
| `crates/cognicode-core/src/application/dto/common.rs` | T1-1a ya hace `pub use crate::domain::value_objects::SymbolKind;` — fuente canónica |
| `crates/cognicode-core/src/domain/value_objects/symbol_kind.rs` | canonical `SymbolKind` con `TypeAlias` (renombrado de `Type` por T1-1a) |

## 1. Objetivo

Hacer que el `SymbolKind` que la capa MCP serializa (schemas.rs) **no sea el
canónico directamente**, sino un newtype con shim serde que mantiene el wire
format lowercase que el consumer espera, **mientras que el canónico vive en
`domain/value_objects/symbol_kind.rs` con su propio `FromStr` y
serialización snake_case/PascalCase** (la del dto).

## 2. Problema

T1-1a colapsó 4 enums `SymbolKind` en uno canónico. El dto `common.rs` ahora
es `pub use crate::domain::value_objects::SymbolKind;` — el canónico. Pero
`interface/mcp/schemas.rs:1979` también apunta al dto, y el `serde` derive
del canónico usa **PascalCase** (porque viene de `domain` donde se prefiere
la convención interna).

El wire format que el consumer (MCP clients + e2e tests) espera es
**lowercase** (`"type"`, `"function"`, `"module"`, etc.). Sin un shim, el
serde derive del canónico rompe `mcp_roundtrip_tests` y el contrato
`schemas_roundtrip_contract`.

## 3. Por qué ahora

T1-1a quedó a medias: reexportó el canónico pero la serialización rompió
mcp_roundtrip_tests. T1-1b cierra eso con un shim dedicado en la frontera
MCP, sin contaminar el canónico con conocimiento del transporte.

## 4. Scope

### 4.1 In-scope

* Crear `McpSymbolKind` (newtype) en `interface/mcp/schemas.rs` (o un submódulo
  si la longitud lo pide) con `#[serde(rename_all = "lowercase")]` y `From<SymbolKind>`.
* Actualizar todos los call-sites en `interface/mcp/schemas.rs` que actualmente
  usan `SymbolKind` (canónico) a usar `McpSymbolKind`.
* Test contractual: `mcp_roundtrip` ve lowercase en JSON; `From<SymbolKind>`
  preserva las 15 variantes.
* `cargo test -p cognicode-core --test mcp_roundtrip_contract` verde.

### 4.2 Out-of-scope

* T1-1c (caller migration fuera de `schemas.rs`). Ese es otro goal.
* Cambiar la serialización del canónico. La decisión de T1-1a es que el
  canónico usa la convención interna; el shim es el puente al transporte.

## 5. Constraints

* **Sin breaking change público**: el wire format lowercase del consumer
  debe quedar idéntico byte-a-byte.
* TDD red → green → refactor. RED test primero.
* Sin nuevos `unimplemented!()`.
* Sin cambios al canónico `SymbolKind` (T1-1a ya cerró su diseño).

## 6. Work items

| ID | Tarea | Verificación |
|----|-------|--------------|
| **T1-1b.1** | RED: añadir test que serializa un `McpSymbolKind` a JSON y espera lowercase (`"function"`, `"type_alias"`, etc.) | `cargo test` RED antes de implementar |
| **T1-1b.2** | Crear `McpSymbolKind` newtype en `interface/mcp/schemas.rs` con `#[serde(rename_all = "lowercase")]` + `From<SymbolKind>` (15 variantes) | compila, test GREEN |
| **T1-1b.3** | Sustituir call-sites de `SymbolKind` en `schemas.rs` por `McpSymbolKind` | `cargo test --workspace` verde, sin regresiones |
| **T1-1b.4** | Verificación final: `cargo test --workspace` + `cargo test --workspace --doc` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0 | acceptance criteria §7 |

## 7. Acceptance criteria

1. T1-1b.1..4 en DONE con su verificación pasada.
2. `cargo test -p cognicode-core --test mcp_roundtrip_contract` exit 0.
3. `cargo test --workspace` exit 0 sin regresión vs baseline (4858 tests).
4. `cargo clippy --workspace --all-targets -- -D warnings` exit 0.
5. Recibos append-only en este doc + update a
   `odd/tasks/v1015-consolidation-and-promote.md` T1-1b row.

## 8. Risks

* **R-1**: el shim podría no cubrir todas las 15 variantes si T1-1a
  añadió alguna sin documentar. Mitigación: T1-1b.1 test usa TODAS las
  variantes, no solo las 5 más comunes.
* **R-2**: call-sites de `SymbolKind` en `schemas.rs` pueden no estar
  todos obvios (uso indirecto vía `Deref` o tipo asociado). Mitigación:
  `grep -rn "SymbolKind" crates/cognicode-core/src/interface/mcp/`
  antes de empezar.

## 9. Recibo (append-only)

| Fecha | WU | Commit | Notas |
|-------|----|--------|-------|
| _pendiente_ | T1-1b.1 | _pendiente_ | RED test del shim |
| _pendiente_ | T1-1b.2..4 | _pendiente_ | implement + migrate + verify |
