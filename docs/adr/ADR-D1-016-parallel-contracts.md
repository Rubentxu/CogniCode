---
title: "ADR — D1: la etapa de contratos corre en paralelo, con los mismos checks"
slug: "ADR-D1-016-parallel-contracts"
status: accepted
date: 2026-10-17
deciders: Maintainer
related:
  - "[[ADR-CI-ORCHESTRATOR-CUTOVER]]"
context:
  - "Aritmética publicada por harness report sobre la corrida 2f9d972d: contracts era 371,6 s de 556,8 s (el 67% del merge); las tres mayores suites de producto juntas sumaban 99,5 s"
  - "La etapa contracts es serie: scripts/ci/run-all-contracts.sh invoca scripts/run_contract_tests.py con 37 ficheros (6 de producto, 31 de CI) uno detrás de otro en un solo proceso"
  - "La cabecera del propio fichero dice que son small, linear, dependency-free contract checks. Su trabajo es lanzar procesos y esperarlos"
---

# ADR — D1: la etapa de contratos corre en paralelo, con los mismos checks

## Decision

1. **`run_contract_tests.py` acepta `--jobs N`** (por defecto 1: el comportamiento de siempre). Con N > 1 ejecuta un contrato por proceso, hasta N a la vez, usando el mismo runner para cada uno: el aislamiento de cada fichero es el que tendría invocado a mano, y no hay estado compartido entre ellos.
2. **La semántica de fallo no cambia.** Cada hijo devuelve su código; el padre devuelve 1 si alguno falló, y nombra en stderr los ficheros que salieron distinto de cero. Un contrato que falla sigue tumbando la etapa.
3. **La salida se agrega en el orden de entrada**, no según terminan: un log de gate que cambia de orden entre corridas es un log que nadie puede diffear.
4. **`run-all-contracts.sh` pasa `--jobs`** con el número de CPUs (tope 8, porque por encima no acelera más y sí compite con el resto de la máquina). `CONTRACTS_JOBS=1` vuelve al modo serie, y un valor no numérico es error de uso (rc=2).
5. **El contrato del runner cubre el modo nuevo**: un contrato que falla hace fallar también la corrida paralela, y los contratos sanos se ejecutan y se reportan.

## Aplicación en D1

- `1de96ef3 perf(ci): la etapa de contratos corre un contrato por proceso, hasta 8 a la vez` cherry-pickeado desde `worktree/test-harness`. Aplica `--jobs` por defecto al wrapper y deja `CONTRACTS_JOBS=1` para diagnóstico.
- La decisión de **no adoptar el motor harness** (`TEST_FAST_ENGINE=harness`) ya está tomada y medida en `ADR-TH-013` (criterio de D1: el harness tarda 337 s contra 26 s de CR-08 en el caso más común). El motor paralelo es el runner de contratos, no el del wrapper `fast`.

## Medición (medida en D1 sobre d1-harness-lean-v2, 2026-10-17, misma máquina)

| Modo | Pared | Notas |
|---|---|---|
| Serie (`CONTRACTS_JOBS=1`, post-D1) | **239 s** | 28 UAT files ya no compilan ni se cuentan |
| Paralelo (`--jobs 8`, defecto post-D1) | **78,5 s** | rc=0, 9 checks en cola final |

**3,0× más rápido que el modo serie post-D1; 4,0× más rápido que el baseline 316 s pre-D1 (ADR-TH-016).** La equivalencia con la medición de ADR-TH-016 está comprobada fichero a fichero: 36 de los 37 ficheros reportan exactamente el mismo recuento.

## Lo que este ADR no decide

- El presupuesto de merge en general ni la selección por impacto (W6 del overlay).
- Paralelizar las suites de producto del gate: cada una es un proceso `cargo` que ya compite por el mismo directorio de target, y su ganancia no está medida.
- Cambiar el defecto del wrapper `fast` (eso es el perfil `dev` del harness, decidido en otra parte).
