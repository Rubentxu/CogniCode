# Fase 1 — Quick Wins

**Duración:** 3–5 días-persona.  
**Objetivo:** restaurar una cadena de evidencia confiable antes de ejecutar trabajo crítico.

## Orden
1. QW-01 — expediente C8.
2. QW-03 — guard de tracked files.
3. QW-04 — clean-clone preflight.
4. QW-02 — reconcile de docs/OpenSpec.
5. QW-05 — SHA pinning Actions.
6. QW-06 — dependency updater.
7. QW-07 — higiene `.env`.

## Hito de salida F1

`PR-READY-G0`: el repositorio puede demostrar automáticamente que un bin/documento requerido por una certificación existe en Git y que la certificación puede comenzar desde un clone limpio.

## No hacer en esta fase
- no optimizar graph insights todavía;
- no dividir `AnalysisService`;
- no publicar v0.99.0;
- no marcar C8 como firmable antes de CR-01.
