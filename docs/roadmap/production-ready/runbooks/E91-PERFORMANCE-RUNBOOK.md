# Runbook — e91 Graph Insights Performance

## Hipótesis verificables
H1: `CommunityDetector::detect` domina el coste.  
H2: `surprising_connections` domina el coste.  
H3: ambos son secundarios y el coste viene de construcción/proyección/repetición.

No elegir solución antes de discriminar H1/H2/H3.

## Paso 1 — Fixture
Fijar fixture multi-repo determinista equivalente al caso que produjo ~367 s p95.

## Paso 2 — Instrumentación
Capturar por invocation:
- graph load/projection;
- god nodes;
- SCC;
- feedback arc set;
- community detect;
- surprising connections;
- serialization.

## Paso 3 — Decisión

- Si community detect >50%: bound de iteraciones + early termination por delta de modularidad.
- Si surprising >50%: preindex adjacency/cross-community candidates.
- Si repeat-call domina: estudiar cache con invalidation explícita.
- Si coste está distribuido: aplicar la optimización de mayor ratio impacto/riesgo y volver a medir.

## Paso 4 — Semantic equivalence
Golden fixture debe mantener:
- comunidades dentro de la equivalencia definida por spec;
- god nodes;
- violations relevantes;
- output shape.

No usar tiempo como única prueba.

## Paso 5 — Budget
El proposal existente sugiere objetivo inicial de **≤30 s** para fixture Tier-2 frente a 367 s actual. Mantenerlo como target de primera iteración; el budget final de producto se documenta tras profiling.

## Paso 6 — Scorecard
- ejecutar scorecard;
- G5 debe ser GREEN;
- reiniciar/construir streak requerido por el contrato vigente.

## Cierre
WU1..WU5 con evidencia, test de regresión y ADR/spec de budget.
