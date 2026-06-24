# DRAFT: Declarative Newtype Macro with Per-Type Derives

**Status:** DRAFT — requires human review before promotion to numbered ADR
**Date:** 2026-06-11
**Source:** auto-grill-loop Q008-P1, Q012-P2

## Context
CogniCode needs a consistent pattern for creating semantic newtypes (strongly-typed wrappers around primitive or structural types). Currently, newtypes are hand-written with inconsistent derive contracts, and schema/DTO mirrored types duplicate boilerplate across two layers.

## Decision
Use a `#[newtype]` attribute proc macro in `cognicode-macros` that:
- Auto-derives: `Debug`, `Display`, `From`, `Serialize`, `Deserialize`
- Accepts opt-in derives via `#[newtype(derive(Clone, Copy, Eq, Ord, Hash, Default))]`
- Routes error spans through `proc_macro2::Span` to user code (not macro internals)
- Is tested via trybuild for compile-fail and expansion snapshot cases

## Rationale
- **Attribute macro over derive macro**: `#[newtype]` reads naturally on the struct; derive macros can only add items, not replace the type definition
- **Opt-in derives over standardized set**: derives encode type semantics; blindly adding `Clone` to a resource handle is incorrect
- **proc_macro2::Span hygiene**: rustc errors must point at user code, not generated code
- **trybuild**: standard Rust pattern for testing proc macro compilation behavior

## Syntax
```rust
#[newtype]
#[newtype(derive(Clone, Eq, PartialEq, Hash))]
pub struct UserId(i64);
```

## Consequences
- All 24 schema/DTO mirrored pairs use this macro
- New `#[newtype]` proc macro in `cognicode-macros` crate
- Hand-written newtype boilerplate eliminated across the codebase

## Alternatives Considered
- **newtype_derive / nutype crate**: rejected — constrains evolution to upstream crate's design; cognicode-macros already exists
- **macro_rules! newtype!**: rejected — can't generate attribute-level derives or Span-aware errors
- **Standardized derive set**: rejected — semantically incorrect; derives are contracts, not boilerplate

## Validation
- [ ] trybuild tests pass for compile-fail and expansion snapshot cases
- [ ] All 24 schema/DTO pairs compile with macro
- [ ] Error spans point to user code, not generated code
- [ ] cargo expand output is readable and intentional
