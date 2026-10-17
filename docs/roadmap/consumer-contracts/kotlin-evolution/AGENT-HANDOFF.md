# Instrucción para el agente implementador — ejecutar el siguiente Kx con valor

**META:** entregar en `Rubentxu/CogniCode` el siguiente hito pendiente del programa Kotlin **integrado dentro del roadmap activo**, cerrando con release binaria real, merge remoto, evidencias de consumidor y limpieza de UAT/E2E/AAT efímeros. No hacer solo infraestructura ni extender harness sin capacidad utilizable.

## Recuperar autoridad antes de editar

1. Leer `AGENTS.md`, `docs/roadmap/ROADMAP.md`, últimas dos entradas **pertinentes** del `docs/roadmap/JOURNAL.md`, `docs/roadmap/CURRENT.md`, y este anexo. Consultar work items en curso; seguir prioridades y bloqueos. Verificar SHA `HEAD`, `origin/main`, versión local y últimas tags/releases remotas reales. Este paquete fue elaborado sobre `main@88f6d02a`, release `v0.101.17`; estos datos **no** sustituyen una inspección actual.
2. Resolver contradicciones: `CURRENT.md` obsoleto, enlace `2026-Q4-CLI-MCP-ROADMAP.md` ausente en baseline, programas CP en curso y cualquier hito/feature que ya exista. **No duplicar ni reabrir PRF/C8**.
3. Usar `MILESTONES.md` y `TRACEABILITY.md` para seleccionar **solo una** unidad Kx no bloqueada, con scope, entregables de consumidor y ADR aplicable; incorporar su estado al roadmap principal y recibo al JOURNAL, nunca usar este archivo como segunda autoridad de progreso.

## Ciclo feature → merge → release (sin loops)

4. Leer specs de Kx, el código de producción indicado y el contrato de la skill `delivery-first-testing` desde `Rubentxu/agent-skill/skills/delivery-first-testing` (o copia instalada comprobada). Router `scripts/affordances.py` con estado factual `feature` → `change-impact` → `ephemeral-uat` → `regression-promotion`. Los lentes no ejecutan las pruebas. Consultar `references/lenses` al necesitar la pregunta.
5. Definir un solo oráculo reproducible por garantía relevante; crear corpus/proyectos en un `mktemp -d` **fuera del repositorio**. Añadir un test permanente mínimo si protege un bug/contrato sin equivalente. Iterar tests afectados y `bash scripts/test-fast.sh`, nunca full suite tras cada ajuste.
6. Implementar mediante funciones puras + ADTs para clasificar, extraer, resolver, agregar; concentrar IO en adapters y orquestación en aplicación; mantener los puertos y ontología canónica existentes. **Prohibido** resolver llamadas por nombre de forma especulativa, decorar `Unknown` como `Complete`, crear stores/motores duplicados, reimplementar LSP/SCIP sin reutilizar tiers.
7. Paralelizar por equipo **solo** los seams que no comparten archivos y fijar contrato común antes (`AGENT-TEAMS.md`). Comprobar y registrar flags, comandos y schemas reales; no confiar en un nombre por conversación.
8. Ejecutar UAT CLI/MCP/E2E/AAT de `uat/ACCEPTANCE.md` sobre comportamiento real; no generar suites efímeras permanentes de forma automática. Fallo → lente `failure-triage`, causa primero, no retries ilimitados.
9. Antes de PR y tag: `bash scripts/test-full.sh` real conforme `AGENTS.md`; `merge-gate` verde y código integrado en `origin/main`. Luego `release-assurance` contra binarios descargables, inventario + SHA256SUMS completos, instalación limpia y el oráculo del hito. Si requiere operador, `PAUSED_OPERATOR` con evidencias, **no CLOSED**.
10. Decidir `PROMOTE/REPLACE/RETIRE/DEFER` para cada UAT/E2E/AAT de sesión; añadir resultado al journal, retirar del workspace temporal tras cierre. Preservar solo regresión pequeña imprescindible; no borrar pruebas críticas heredadas.
11. Actualizar puntero del roadmap y `CURRENT` tras medir estado final, version y release; verificar publicación/consumo reales antes de pasar a Kx+1. Un único recibo breve, sin 20 documentos de certificación redundantes.

## Resultado que debes devolver

`Kx/consumer value | code SHA / tag/version | PR/merge URL | release URL+assets | tests affected/full/merge | UAT/E2E/AAT outcomes | promoted regression | temporary tests retired | remaining gaps | next unit.`

**ANTIBUCLE:** dos iteraciones sin nueva observación útil sobre causa/consulta/prueba → `PAUSED` y razón; no simular progreso por redactar tests o recibos. Si bloquea el runtime, registrar `NOT_RUN/BLOCKED` con exactamente la precondición faltante. El código de producto, no el número de gates, es la salida principal.
