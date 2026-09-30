# Roadmap addendum — Production Ready Program

Este bloque está diseñado para incorporarse al roadmap activo sin reabrir PRF histórico.

> **Reconciliado 2026-09-30** (auditoría del operador + verificación por
> medición). La columna "Estado inicial" se conserva como registro del
> arranque; la columna **Estado real** es la que manda. Criterio de cierre:
> una acción sólo es CLOSED cuando su acceptance criterion se demuestra
> contra código/runtime real, el test que la protege ha demostrado RED ante
> una mutación relevante, y ese test corre dentro de un gate obligatorio.

| ID | Objetivo | Estado inicial | Estado real (2026-09-30) | Exit |
|---|---|---|---|---|
| PR-G1 | Reproducible governance | PENDING | **PARTIAL** — QW-03/04/06/07 PASS; QW-02 FAIL (drift ROADMAP/CURRENT/ADDENDUM detectado por auditoría; reconciliación en curso este ciclo); QW-05 PARCIAL (pins SHA mayoritarios, quedan refs mutables: `dtolnay/rust-toolchain@1.96.0`, `rootful/setup-podman@v4`) | QW-01..07 cerrados |
| PR-G2 | C8-R | PENDING | **PARTIAL** — C8-R técnicamente PASS sobre SHA antiguo (`3954b8b7`, 5579/0/37); falta recertificación final sobre el SHA que cierre Production Ready | clean-clone certification PASS + firma pendiente/realizada explícitamente |
| PR-PERF | e91 Graph Insights | PENDING | **PARTIAL** — W1-W9 CLOSED (metadata honesta, regression budget, optimizaciones); falta scorecard GREEN ×3 consecutivos sobre fixture definido | G5 GREEN + regression budget |
| PR-ARCH | Application boundary | PENDING | **PARTIAL** — CR-06 reabierto por auditoría (falso negativo: `LayerId` sin `Interface`; corregido en PR #311 con UAT del import productivo); los 2 drifts reales inventariados; ST-01..05 sin ejecutar | fitness functions activas + primer vertical remediado |
| PR-SEC | Supply-chain hardening | PENDING | **OPEN** — `RUSTSEC-2024-0437` sigue en `deny.toml` ignore (OTel 0.27); refs de Actions mutables pendientes; el fallback de autoridad de tools fail-open corregido a fail-closed en PR #312 | protobuf advisory eliminado + release Actions pinneadas |
| PR-DEVEX | Adaptive CI & coverage governance | PENDING | **CLOSED (enforcement side)** — CR-08 selector determinista + CR-09 coverage gates PASS; ver ROADMAP fila PR-DEVEX | selector probado + no-regression coverage |
| PR-DEPTH | Deep modules | PENDING | **OPEN** — ST-01..05 FAIL por medición directa (file_operations 4.0k líneas + imports interface; workspace_session 4.5k composition root; analysis_service 3.8k multi-owner; HandlerContext 21 campos públicos; graph semantics multi-owner) | ST-01..05 cerrados |

## Completion
El programa queda COMPLETED cuando PR-G2, PR-PERF, PR-ARCH, PR-SEC y PR-DEVEX están cerrados y PR-DEPTH ha completado al menos los verticales ST-01/ST-02/ST-04; ST-03/ST-05 pueden continuar como evolución si sus contracts ya están protegidos y no bloquean producción.
