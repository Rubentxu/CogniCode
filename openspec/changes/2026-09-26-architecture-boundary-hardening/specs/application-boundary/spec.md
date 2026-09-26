# Spec — Application boundary

## Requirement 1 — Application does not import infrastructure
Production code bajo `crates/cognicode-core/src/application/` MUST NOT depender directamente de `crate::infrastructure::*`, salvo excepciones temporales explícitas y versionadas.

## Requirement 2 — Application does not import interface
Production code bajo application MUST NOT depender de `crate::interface::*`.

## Requirement 3 — Exceptions expire
Toda excepción MUST incluir owner, rationale y expiry; una excepción sin esos campos es inválida.

## Requirement 4 — Negative control
Un test MUST plantar una dependencia prohibida y demostrar que la fitness function la detecta.
