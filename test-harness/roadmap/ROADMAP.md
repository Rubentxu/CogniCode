# ROADMAP — evolución paralela del test harness de CogniCode

**Estado inicial:** SPECIFIED / NOT_IMPLEMENTED. Cada WU registra evidencia real, PR remoto y gate de integración antes de considerarse CLOSED. Hitos con entrega de valor y versión del componente (no fingir release de producto CogniCode). No modificar el roadmap PRF histórico: el agente enlazará esto desde la unidad activa de `docs/roadmap/ROADMAP.md` cuando corresponda.

## Resumen para desarrollador novel — TODO ejecutable

- [ ] **W0: Confirmar base y medir.** Sin tocar pipelines, verificar HEAD/estado de ramas, detectar consumidores de scripts, inventariar targets y medir tiempos de referencia. Añadir pruebas que reproduzcan A01-A14, con resultados RED esperados. **Entrega:** informe reproducible del tiempo actual + mapa real de cobertura. **Release:** milestone de especificación/diagnóstico, PR documental si la gobernanza autoriza.
- [ ] **W1: Hacer fiable el sandbox y limpiar obsolescencias.** Arreglar rc en campañas, estados `COMPLETE/INCOMPLETE/FAILED`, resolver binarios desde Cargo; reparar la ruta obsoleta `test-all.sh` con compatibilidad demostrada y limpiar versiones hardcodeadas de scorecard. Tests de fallo de worker y recibo ausente. **Entrega:** no hay PASS falso; sandbox estable. **Release:** bump del componente o release integrado según política, PR + merge.
- [ ] **W2: Centralizar ejecutores y medir solapes.** Crear esqueleto `test-harness/{src,adapters,bin,tests}` y delegaciones compat desde `scripts/`; paridad de argumentos/rc; reusar observación para baseline en lugar de correr otra vez. Tests de paridad de inventario y métricas. **Entrega:** un front de tests conservando `full` original. **Release:** PR + merge.
- [ ] **W3: Selector incremental conservador.** Rehacer `ChangeSet` de merge-base + dirty + untracked; grafo Cargo y reverse dependencies; acciones `plan/explain` con justificación. Comparación automática con CR-08; mutaciones de fallos de selección. **Entrega:** desarrollador obtiene planes correctos sin suite global en cada cambio menor. **Release:** PR + merge; modo `plan` opt-in.
- [ ] **W4: Ledger, fingerprints y reanudación.** Introducir `SuiteStatus`, `EvidenceReceipt`, `ReuseDecision`, `ExecutionPlan`, adaptadores estándar y almacenamiento externo durable. `run/resume/status` implementados; locks, corte SIGKILL y invalidación. **Entrega:** tras fallo se retoma desde suite afectada. **Release:** PR + merge; opt-in hasta UAT.
- [ ] **W5: Nextest piloto y sandbox por escenario.** Integrar optional Nextest/grabaciones por ID, mantener cargo doc/serial y fallback. Sandbox por escenario y fixtures inmutables, no exponer forks por path. **Entrega:** TDD y campañas pueden repetir únicamente fallidos sin falsos PASS. **Release:** PR + merge; selección de runner declarativa.
- [ ] **W6: Pilotar con PipelineK, demostrar paridad, aceptar gate.** Shadow plan en lanes vigentes, UAT/AAT y comparativa de velocidad; demostrar cada obligación del full y cada artifact hash; ADR de política para admitir reutilización en gates oficiales si se aprueba. Adaptar stages progresivamente, sin tocar la fábrica release salvo interfaz permitida. **Entrega:** aceptación oficial o `NOT_CERTIFIED` explícito. **Release:** PR + merge sólo con matrices completas.
- [ ] **W7: Consolidar, retirar duplicados y documentar operación.** Eliminar lógica antigua, conservar shims mínimos cuando haya clientes. Ratchets anti-regresión, benchmark antes/después, guía operadores, limpieza de deuda documental en documentos vigentes. **Entrega:** una sola autoridad técnica en `test-harness/`, rutas antiguas compatibles o retiradas justificadamente. **Release:** PR + merge.

## Estado de las WU (2026-10-15)

La primera implementación del harness cubre parte de W2, W3, W4 y W6. Ninguna de
las cuatro está CLOSED: el protocolo de bloque exige revisión y decisión de
adopción, y eso no se ha hecho. Lo que hay, y lo que falta:

| WU | Estado | Qué existe hoy | Qué falta para cerrarla |
|---|---|---|---|
| W0 | NOT_STARTED | — | Informe de tiempos de referencia y mapa de cobertura |
| W1 | NOT_STARTED | — | Fiabilidad del sandbox, `test-all.sh` |
| W2 | EN_PROGRESO | Esqueleto `test-harness/{src,adapters,bin,tests}` con front propio, 60 tests dependency-free, contrato de gate | Delegaciones compat desde `scripts/`; paridad de argumentos y rc con `full` |
| W3 | EN_PROGRESO | `ChangeSet` de merge-base + dirty + untracked con `-z`; grafo de `cargo metadata` con dependientes transitivos; `plan`/`explain` con motivo por decisión; fail-closed en ruta desconocida y borrado de test; **comparación automática con CR-08** (`parity --corpus`, 16 clases, 0 debilidades sin declarar) y acoplamientos del oráculo respetados | Mutaciones de fallos de selección como suite propia; migrar `test-fast.sh` |
| W4 | EN_PROGRESO | Ledger append-only fuera del checkout, recibos inmutables, huellas con `policy_version`, `run/resume/status`, lease con reconciliación de dueños muertos, marcador `RUNNING` por intento, **concurrencia con cerrojos declarados** (`--max-workers`), **pin de inventario** (`inventory.json`) y **GC** que conserva evidencia; SIGKILL + reanudación y dos runs al mismo id cubiertos por test | Invalidación por cambio de features declarado en el plan (hoy entra vía argv/huella), retención del ledger por antigüedad, target dir por worker si se decide paralelizar compilación |
| W5 | NOT_STARTED | — | Nextest opcional, sandbox por escenario |
| W6 | PARCIAL | Paridad con el oráculo medida y gated en la etapa de contratos (`parity --corpus`: 0 debilidades sin declarar en `change`, `merge` e `integration`) | Decidir si el plan sustituye alguna selección; ADR de política de reutilización en gates oficiales |
| W7 | NOT_STARTED | — | Retirada de duplicados y guía de operación |

## Dependencias

`W0 -> W1 -> W2 -> W3 -> W4 -> W5 -> W6 -> W7`. W1 puede prepararse en rama corta separada y fusionarse pronto sin esperar W3/W4. W3 y W4 tienen interfaces negociadas desde W2; se pueden desarrollar en worktrees distintos sin modificaciones simultáneas a scripts de autoridad. W6 nunca comienza sin W4/5 certificados y paridad con W0.

## Protocolo de bloque de valor

1. `git status`, HEAD, journal SDDK y roadmap actual; rebase limpio y contrato vigente.
2. Caracterización RED / mutación real; señalar el fallo que se busca eliminar.
3. Implementación mínima con funciones puras, ADTs y adaptadores de efectos.
4. Tests quirúrgicos; UAT de la unidad; `plan/explain`; full que exija política actual.
5. Comparar cobertura con inventario viejo; publicar recibos, duraciones, fallos y pendientes.
6. Commits atómicos Conventional Commits; PR a rama principal; gate real; merge remoto; tag/release si la política del componente lo exige.
7. Reconciliar nuevo SHA de main con worktree activo; siguiente WU desbloqueada. Sin cierre si el gate es rojo.

## Riesgos que bloquean la aceptación

- Degradar `full` sin ADR aceptada.
- Introducir nueva fuente de verdad de suites con listas duplicadas.
- Tratar Nextest rerun como invalidación por código.
- Suprimir/ocultar fallos sandbox o tests 0.
- Mover tests Rust fuera de rutas Cargo o modificar perfiles globales Cargo.
- Crear un segundo orquestador de CI independiente de PipelineK.
- Usar recibos de tests como sustituto de validación de los tarballs publicados.

## Traza de referencias existentes

`AGENTS.md`, `docs/roadmap/ROADMAP.md`, `docs/roadmap/MAINTENANCE.md`, `docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md`, `scripts/test-fast.sh`, `scripts/test-full.sh`, `scripts/test-all.sh`, `scripts/ci/select-suites.sh`, `scripts/check_known_failures.py`, `scripts/ci/test_fast_full_wrappers.py`, `scripts/ci/test_select_suites.py`, `scripts/ci/pipeline_authority.py`, `scripts/ci/run-all-contracts.sh`, `scripts/run_contract_tests.py`, `sandbox/scripts/run_campaign.sh`, `sandbox/justfile`, `merge-gate.pipeline.kts`, `integration.pipeline.kts`, `certification.pipeline.kts`, `release-candidate.pipeline.kts`, `release.pipeline.kts`.

## Definición de DONE por hito

Una WU está `DONE` sólo si sus tests unitarios+contratos+UAT están verdes, ninguna guarda fue eliminada, el trabajo existe en remoto, PR fusionada (si es integrable en ese momento), y la salida reporta cambio de versión conforme a norma. Una WU con incertidumbre o infra bloqueada queda `PAUSED/BLOCKED`, pero los recibos previos correctos sobreviven.
