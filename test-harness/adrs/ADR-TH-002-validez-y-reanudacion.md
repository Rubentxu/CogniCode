# ADR-TH-002 — Reutilizar evidencia, no estados nominales

- **Estado:** PROPOSED.
- **Decisión:** recibos inmutables con identidad completa de inputs, entorno y resultado; reanudación sólo tras reevaluar validez. Prohibido cachear PASS por `suite_id` o `HEAD` exclusivamente. `Unknown => Execute` en lanes críticas.
- **Conflicto:** rapidez deseada frente a seguridad por cambios indirectos de código, fixtures, macros, dependencias, toolchain y entorno.
- **Opciones:** reejecutar todo (seguro, lento); confiar en Nextest rerun (rápido, no invalida por código); conservar PASS por SHA (insuficiente con dirty y dependencias); registrar clausura verificable (elegida).
- **Límites:** pruebas no deterministas, performance, orden, cambio temporal externo o instalaciones reales se reejecutan por política. `ReleaseArtifactIdentity` independiente del source digest.
- **Prueba:** mutaciones del test, del código upstream, Cargo.lock, flags, variables, scripts y untracked invalidan exactamente las obligaciones afectadas; un área ajena no invalida otra en modos permisivos cuando el grafo se conoce.
