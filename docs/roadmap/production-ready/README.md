# CogniCode — Production Ready Action Plan

**Baseline:** auditoría técnica sobre `main@2991e5e227d938edf14422295cdf644daf3f1fab`  
**Fecha del plan:** 2026-09-26  
**Propósito:** convertir los hallazgos verificados de la auditoría en un programa ejecutable, medible y secuenciado para dejar CogniCode production ready sin mezclar deuda, certificación y evolución estratégica.

## Principios de ejecución

1. **Reproducibilidad antes que nuevas features.** Ninguna certificación se acepta si no puede repetirse desde un clone limpio del SHA certificado.
2. **Una sola autoridad de agenda.** Este paquete complementa `docs/roadmap/ROADMAP.md`; no crea una segunda fuente de verdad. Su contenido debe incorporarse como programa de trabajo activo hasta su cierre.
3. **Testing progresivo.** En desarrollo se ejecutan tests afectados; en integración/release se ejecutan full suite, all-features y gates de distribución.
4. **Arquitectura ejecutable.** Las reglas importantes se convierten en fitness functions; no quedan solo en ADRs.
5. **Módulos profundos.** Se reduce superficie pública antes de fragmentar archivos mecánicamente.
6. **Evidence first.** Cada acción tiene evidencia, KPI, criterio de aceptación y recibo de cierre.

## Entregables de este paquete

- `EXECUTIVE-SUMMARY.md` — alcance, esfuerzo, timeline y criterio de éxito global.
- `PRIORITIZATION-MATRIX.md` — impacto/esfuerzo de todos los hallazgos.
- `EXECUTION-PLAN.md` — acciones atómicas, dependencias, roles y orden recomendado.
- `DEPENDENCY-MAP.md` — grafo Mermaid del programa.
- `ROLE-ASSIGNMENT.md` — responsables por rol y prerequisitos técnicos.
- `RISK-REGISTER.md` — riesgos de ejecución y mitigaciones.
- `METRICS-AND-ACCEPTANCE.md` — KPIs y Definition of Done.
- `UAT-CHECKLIST.md` — pruebas de aceptación del programa.
- `phases/` — plan operativo por fases.
- `runbooks/` — C8 y e91 paso a paso.
- `adr/` — propuestas de decisión arquitectónica derivadas de la auditoría.
- `openspec/changes/` — cambios nuevos para recertificación C8 y boundary hardening.

## Secuencia ejecutiva

1. **Fase 1 — Quick Wins:** restaurar trazabilidad, eliminar drift documental y añadir guardas de reproducibilidad.
2. **Fase 2 — Críticos:** recertificar C8, cerrar e91, ampliar fitness functions, supply-chain y CI adaptativa.
3. **Fase 3 — Estratégicos:** profundizar módulos y retirar acoplamientos estructurales sin big-bang.

## Regla de parada

No iniciar Fase 3 si cualquiera de estos puntos sigue rojo:

- C8 no reproducible desde clone limpio.
- e91 sin budget de rendimiento verde.
- `application` sin fitness functions ejecutables.
- release gate con advisory crítico no justificado.
