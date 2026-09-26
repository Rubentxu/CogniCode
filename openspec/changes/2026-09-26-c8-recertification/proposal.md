# Proposal — C8 reproducible recertification

## Problem
La certificación C8 original se emitió sobre un SHA que no contenía el source del binario `cognicode-control-plane`, aunque el working tree local sí lo contenía. El expediente referenciado por ROADMAP tampoco está versionado.

## Value
Restablecer la propiedad más importante de una certificación: que un tercero pueda reproducirla desde el SHA declarado.

## Scope
- clean-clone preflight;
- verificación de inputs trackeados;
- full verification sobre SHA congelado;
- CP live contract con canonical constraint IDs;
- expediente C8-R versionado.

## Out of scope
- refactor arquitectónico;
- e91;
- nueva feature de producto;
- publicación automática v0.99.0.

## Acceptance
Todos los requisitos en `specs/c8-recertification/spec.md` pasan desde un fresh clone.
