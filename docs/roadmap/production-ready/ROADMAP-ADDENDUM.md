# Roadmap addendum — Production Ready Program

Este bloque está diseñado para incorporarse al roadmap activo sin reabrir PRF histórico.

> **Reconciliado 2026-09-30** (auditoría del operador + verificación por
> medición) y de nuevo el **2026-10-01** (las filas PR-G1, PR-ARCH, PR-SEC y
> PR-DEPTH describían un estado que git contradecía; cada corrección cita el
> commit que la respalda). La columna "Estado inicial" se conserva como
> registro del arranque; la columna **Estado real** es la que manda. Criterio de
> cierre: una acción sólo es CLOSED cuando su acceptance criterion se demuestra
> contra código/runtime real, el test que la protege ha demostrado RED ante
> una mutación relevante, y ese test corre dentro de un gate obligatorio.

| ID | Objetivo | Estado inicial | Estado real (2026-09-30) | Exit |
|---|---|---|---|---|
| PR-G1 | Reproducible governance | PENDING | **PARTIAL** — QW-03/04/06/07 PASS; QW-02 FAIL (drift ROADMAP/CURRENT/ADDENDUM detectado por auditoría; reconciliación en curso este ciclo); QW-05 **PASS** (reconciliado 2026-10-01: los 19 refs `dtolnay/rust-toolchain@1.96.0` pineados al SHA `ebb3d167…` verificado por PGP en `4bb86de7`, y los 2 refs `rootful/setup-podman@v4` —repo 404— retirados en `c5034b03`. `action_ref_pin_contract` deriva la cobertura del directorio de workflows: quedan **0 refs mutables**, y el contrato falla si se añade un workflow nuevo sin cubrir) | QW-01..07 cerrados |
| PR-G2 | C8-R | PENDING | **PARTIAL** — C8-R técnicamente PASS sobre SHA antiguo (`3954b8b7`, 5579/0/37); falta recertificación final sobre el SHA que cierre Production Ready | clean-clone certification PASS + firma pendiente/realizada explícitamente |
| PR-PERF | e91 Graph Insights | PENDING | **PARTIAL** — W1-W9 CLOSED (metadata honesta, regression budget, optimizaciones); falta scorecard GREEN ×3 consecutivos sobre fixture definido | G5 GREEN + regression budget |
| PR-ARCH | Application boundary | PENDING | **PARTIAL** — CR-06 reabierto por auditoría (falso negativo: `LayerId` sin `Interface`; corregido en PR #311 con UAT del import productivo); los 2 drifts reales inventariados; **ST-01 CERRADA** (`4b7de348` + `ba031da5`, en rama local `fix/st01-file-operations-ports`, aún sin push a `origin/main`) y **ST-02 con su rebanada 1 cerrada** (`0ecdd415`: cero acoplamiento `application → interface` en producción); ST-03/04/05 sin ejecutar | fitness functions activas + primer vertical remediado |
| PR-SEC | Supply-chain hardening | PENDING | **OPEN** — `RUSTSEC-2024-0437` sigue en `deny.toml` ignore (OTel 0.27, diferido deliberadamente: romper `/metrics` en producción sería peor que el advisory, `JOURNAL:3584-3588`); **refs de Actions mutables: ninguno**, reconciliado 2026-10-01 (`4bb86de7`, `c5034b03`); los 4 ignores de advisory con respaldo verificable en `docs/debts/DEBT-SEC-001-advisory-ignores.md` y contrato que lo exige (`f2d94d24`); `cargo deny check advisories` **verde** desde `c5f47197`; el fallback de autoridad de tools fail-open corregido a fail-closed en PR #312 | protobuf advisory eliminado |
| PR-DEVEX | Adaptive CI & coverage governance | PENDING | **CLOSED (enforcement side)** — CR-08 selector determinista + CR-09 coverage gates PASS; ver ROADMAP fila PR-DEVEX | selector probado + no-regression coverage |
| PR-DEPTH | Deep modules | PENDING | **OPEN** — **ST-01 CERRADA** (`4b7de348` + `ba031da5`): `file_operations` programado contra puertos inyectados, seam cerrado, contrato de allowlist auditado. **ST-02 en curso**: rebanada 1 cerrada (`0ecdd415`, cero `application → interface` de producción); siguen abiertos `workspace_session` (4.5k líneas, 10 campos) y su composition root. **ST-03/04/05 FAIL por medición directa** (`analysis_service` 3.8k multi-owner; HandlerContext 21 campos públicos; graph semantics multi-owner) | ST-01..05 cerrados |

## Completion
El programa queda COMPLETED cuando PR-G2, PR-PERF, PR-ARCH, PR-SEC y PR-DEVEX están cerrados y PR-DEPTH ha completado al menos los verticales ST-01/ST-02/ST-04; ST-03/ST-05 pueden continuar como evolución si sus contracts ya están protegidos y no bloquean producción.
