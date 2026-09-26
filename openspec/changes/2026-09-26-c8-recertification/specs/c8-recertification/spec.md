# Spec — C8 reproducible recertification

## Requirement 1 — Git is the complete input
La certificación MUST fallar si un source/config/doc requerido no está trackeado en el SHA candidato.

### Scenario — declared binary source is untracked
GIVEN un bin target declarado cuyo source existe localmente pero no en Git  
WHEN se ejecuta el preflight  
THEN el resultado es FAIL antes de compilar.

## Requirement 2 — Clean clone execution
La campaña MUST ejecutarse sobre un fresh clone/checkout limpio del SHA candidato.

## Requirement 3 — Control Plane semantic evidence
El UAT MUST comprobar `status=evaluated` y el set completo de IDs canónicos esperados, no solo HTTP 200.

## Requirement 4 — Immutable candidate
Si se corrige código o documentación requerida durante la campaña, MUST generarse un SHA nuevo y repetirse los gates afectados.
