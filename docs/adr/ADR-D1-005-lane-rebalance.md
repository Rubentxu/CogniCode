---
title: "ADR — D1: una suite, un dueño; las construcciones release salen del gate de merge"
slug: "ADR-D1-005-lane-rebalance"
status: accepted
date: 2026-10-17
deciders: Maintainer
related:
  - "[[ADR-CI-ORCHESTRATOR-CUTOVER]]"
  - "[[ADR-D1-006-uat-retirement]]"
context:
  - "merge-gate.pipeline.kts declaraba 4 construcciones en perfil release (cognicode-mcp, cognicode-control-plane, cognicode-cli, bin cognicode) con un coste medido de ~676 s por iteración"
  - "El único consumidor verificable de esos artefactos en esa lane era un conjunto de 6 suites que taran, generan y verifican bytes de perfil release"
  - "El resto de la lane ejecuta binarios que cargo test resuelve por CARGO_BIN_EXE_* en perfil debug"
---

# ADR — D1: una suite, un dueño; las construcciones release salen del gate de merge

## Decision

Una suite pertenece a la lane que puede producir sus precondiciones. Esto se aplica en D1 con tres movimientos coordinados:

1. `merge-gate.pipeline.kts` construye los binarios de perfil **debug** que sus propias suites ejecutan (`cognicode-mcp`, y `--bins` de `cognicode-cli`) y verifica que existen y arrancan. **No construye nada en release**.
2. `release-candidate.pipeline.kts` ejecuta las 3 suites de perfil release que sobreviven a la retirada de la clase UAT (ver `ADR-D1-006`) y las apunta explícitamente a los artefactos de esa lane con `COGNICODE_RELEASE_BIN` y `COGNICODE_RELEASE_BIN_DIR`. Las 3 suites que se retiran con la clase UAT no necesitan dueño: la garantía que cubrían pasa a estar respaldada por el contrato superviviente y por la evidencia histórica en `docs/prf/`.
3. Las suites de perfil release fallan en voz alta cuando el artefacto que se les señala no existe o no es de perfil release. No hay modo de reutilizar un binario debug "que resultó estar ahí".

## Invariantes que NO cambian

- **PipelineK sigue siendo la única autoridad de CI.** No se introducen GitHub Actions, ni un segundo orquestador, ni una capa de caché o reanudación.
- **Ninguna suite crítica se elimina y ninguna aserción se sustituye por un skip.** El conjunto de `crates/cognicode-cli/tests/*.rs` y `crates/cognicode-mcp/tests/*.rs` se compara contra el sistema de ficheros: exhaustivo entre las dos lanes y sin que la lane de merge nombre una suite que no puede ejecutar.
- **Las suites que ejecutan binarios por ruta fallan en voz alta si el binario falta**, y la lane de merge les entrega el binario debug que ella misma construyó, no el que aparezca en el directorio compartido de artefactos.
- La garantía de `verify-release-binaries` ("el artefacto existe, es ejecutable y arranca") se conserva: en la lane de candidato ya existía como `binary-smoke-$target`, que además sabe ejecutar un target extranjero con su emulador y su sysroot.

## Aplicación en D1

D1 cherry-pickea los commits codificados en la rama `worktree/test-harness` (PR #349) que aplican esta decisión:

- `c3895d64 ci(merge-gate): mover las suites de perfil release al gate de candidato (B1)` — el cambio de lane.
- `49aa6b16 test(uat): retirar la clase UAT del repositorio (ADR-TH-006)` — la retirada de la clase que duplicaba 3 de las 6 suites release.
- `0b47d14d perf(gate): el gate de rustdoc se paga una vez, no dos (−737 s por merge)` — ahorros paralelos que no relajan cobertura.

## Consecuencias

- **El coste por iteración de merge baja**: se dejan de construir 4 artefactos release y se construyen 2 binarios debug que el propio gate ya necesitaba compilar para sus suites.
- **El candidato no paga una construcción extra**: reutiliza los binarios que ya produce para empaquetar. Las suites pasan a ejecutarse contra los bytes del candidato, que es más fiel que un artefacto construido en otra lane.
- Un fallo de las 3 suites release que sobreviven deja de bloquear el merge y pasa a bloquear la lane de candidato, que es obligatoria antes de publicar: `release.pipeline.kts` no reconstruye, publica lo que el candidato verificó.
- El riesgo aceptado es explícito: entre el merge y la publicación, un cambio que rompa el perfil release se detecta en la lane de candidato y no antes. Eso ya era así para todo lo que la lane de candidato construye y verifica.

## Lo que este ADR no decide

- La política de seguridad ni de certificación, que se mantiene vigente.
- El significado de `full` en `AGENTS.md`: no autoriza solape, reutilización de resultados ni certificación de artefactos sin verificar.
- La retirada de la clase UAT, que es una decisión independiente (`ADR-D1-006`) tomada el mismo día por el operador.

## Fundamento

- `ADR-CI-ORCHESTRATOR-CUTOVER` distingue lanes con responsabilidades explícitas.
- La lane de merge es "lo que un cambio debe pasar para fusionarse"; la lane de candidato es "lo que un artefacto debe probar antes de publicarse". Una suite que necesita el candidato pertenece a la segunda.

## Prueba que la sostiene

- Los contratos de cobertura de CLI y core (propiedad exhaustiva por lane, detección medida desde el código de las suites que resuelven artefactos release).
- Las mutaciones adversariales: quitar una suite obligatoria, quitar la construcción debug o cambiar el dueño de una suite debe poner el gate rojo.
- La corrida del merge-gate con D1 aplicado (a verificar en la integración de D1).
