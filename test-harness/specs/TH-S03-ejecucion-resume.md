# TH-S03 — Supervisor reanudable y tratamiento de errores

**Estado:** PROPOSED. **Dueño:** `test-harness/src/scheduler`, `adapters/*`, CLI.

## Vida de una ejecución

1. `Prepare` revisa identidad, catálogo, repositorio sin daño, cache Cargo y recursos; fija `RunId` y un `SourceSnapshot`.
2. `Plan` construye DAG de requirements, evalúa recibos, determina `Execute/Reuse/Blocked` con explicación.
3. `Run` asigna trabajos con límite de concurrencia, locks y timeouts; escribe `Running` durable **antes** de lanzarlos.
4. Tras cada trabajo, persiste resultado terminal y artefactos de evidencia **atómicamente**. Un crash no revierte PASS previos.
5. `Resume(runId)` vuelve a calcular plan con el snapshot actual (no reutiliza blindly el del run); repite únicamente suites fallidas, interrumpidas o invalidadas y ejecuta las pendientes.
6. `Aggregate` computa gate, omisiones, matriz efectiva; `ExitCode` honesto.

## Tipos de fallo y comportamiento

| Estado | Reanudación | Exit/Gate |
|---|---|---|
| `Failed` por aserción | Tras corrección, nueva ejecución del test/suite; upstream válido se conserva | No certifica hasta verde |
| `InfraError` (Podman/PG/downloading) | Reintento limitado explícito con backoff; no etiquetar PASS | Gate bloqueado mientras haya obligation |
| `TimedOut`/`Cancelled` | Reejecutar; no cacheable | Gate pendiente |
| `Running` sin heartbeat tras crash | Marcar unknown/interrupted, reconciliar subprocess y cleanup | Nunca como PASS |
| `Passed` con huella idéntica y policy permite | Reusar con recibo | Gate exige cobertura |
| `Passed` con huella distinta | Invalidar sólo su clausura afectada | Reejecutar |
| `Skipped`/`Quarantined` | Sólo si requirement contempla explícitamente excepción autorizada | No se transforma en PASS |

## Contratos de scheduler

- Reanudación idempotente de un mismo `RunId` cuando ninguna operación está en curso; dos procesos concurrentes requieren leasing y tokens anti doble ejecución.
- Si un test falla, **no es obligatorio detener todas las ramas independientes**: modo fail-collect conserva evidencia; modo fast-fail permite parar scheduling pero señala pendientes reales. Política definida por lane.
- Mantener semántica test vs infraestructura de shell: exit 101 de Cargo puede ser compilation, test o harness error según output estructurado; no asumir.
- Fallar por regresión y seguir recogiendo resultados si configurado no significa PASS global.
- `resume` tras modificación de código vuelve a medir invalidación. No usar `--start-at=S3` con PASS de S1/S2 sin huella.
- Para tests seriales o mutadores del entorno, proceso aislado / exclusión; prohibido paralelizar por defecto.
- La lista de tareas y estados es derivada del plan; ninguna etapa anterior se olvida, aunque sea `Reusable`.

## CLI propuesto

```text
plan [--lane change|merge|integration|certification] [--base <gitref>] [--format text|json]
run [--lane ...] [--selection <plan-id>] [--max-workers <N>]
resume --run-id <id> [--mode continue|failed-only]
status [--run-id <id>]
explain --suite <id> [--run-id <id>]
certify --lane ... --candidate <sha|artifact-digest>    # únicamente si política permite
```

## Salida y códigos

`0 = gate satisfecho`, `1 = tests/obligaciones no satisfechos`, `2 = error de plan/harness`, `3 = evidencia indeterminada o stale`, `130 = interrupción`. La clasificación de errores completa es API versionada; no remapear códigos existentes de wrapper sin compat test.

## Prueba adversarial

Dos intentos concurrentes al mismo RunId; SIGKILL después de escribir PASS y antes de agregar; proceso huérfano; disco lleno; recibo roto; cambio de HEAD/dirty mientras un suite RUNNING; cambio de nombre de test entre ejecuciones. El sistema debe reconstruir un estado seguro y reejecutar sólo lo necesario.
