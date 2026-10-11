# B6 — Agent-Ready Runtime & Portable Distribution (v0.101.16)

> **Estado**: PENDING. Desbloqueado por B5. Cierra el programa.
> Detalle espeficado: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b6--agent-ready-runtime--portable-distribution).

## Valor

CogniCode puede instalarse, actualizarse y utilizarse de forma reproducible desde agentes reales sobre repositorios de distintos tamaños. Las consultas son acotadas, cancelables y observables, sin depender de Explorer ni de servicios externos obligatorios.

## Prerrequisitos

- B5 cerrado: refactor transaccional + política de autorización unificada.

## Work units

- [ ] **B6.W1** — Robustez MCP. Verificar con servidor real: init/cierre limpio, framing JSON-RPC por stdout, errores estructurados, timeouts + cancelación, límites (concurrencia, profundidad, fanout, memoria), paginación + continuación, aislamiento entre sesiones/workspaces, ausencia de procesos huérfanos. NO daemon adicional si stdio MCP actual basta.
- [ ] **B6.W2** — Rendimiento reproducible. Benchmarks en corpus pequeño/mediano/grande sobre análisis frío/caliente/incremental/búsqueda/navegación/grafos. Baselines comparables y presupuestos por categoría. Usar mecanismos e91 existentes. Completar evidencia G5.
- [ ] **B6.W3** — Lifecycle de `cogh`. Instalación limpia, actualización, pinning, compatibilidad IDE, detección de errores, doctor, rollback. Primera consulta real sin compilar CogniCode.
- [ ] **B6.W4** — Skills y autodescubrimiento. Skills alineadas con capacidades reales. Evaluaciones agent-first: descubrir, seleccionar, interpretar `Partial/Unknown/Unsupported/Failed`, obtener info útil. NO presuponer experimentales como estables.
- [ ] **B6.W5** — Certificación de distribución. Verificar paquetes Linux publicados y canales adicionales. Plataformas no disponibles → `NOT_RUN` o `SKIP_NOT_APPLICABLE` con justificación. Completar CP2/CP4/CP5 aplicables (sin CP6 web/landing).
- [ ] **B6.W6** — Cierre arquitectónico. Medir deuda residual en grandes coordinadores. Completar ST-02/ST-04 que afecten CLI/MCP. NO dividir por tamaño. Cerrar separación solo si elimina acoplamiento medido y conserva contratos. Preparar C8-R reproducible y dejar firma humana a la autoridad correspondiente.

## Acceptance criteria (resumen)

- UAT-B6-01..05: instalación desde artefactos, agente externo completa análisis, CLI equivalente, upgrade/rollback, perf scorecard.
- AAT-B6-01..06: cancelar libera, desconectar limpia, entrada grande limitada, telemetría opt-in no bloquea, paquete manipulado rechazado, agente distingue estados.
- `bash scripts/test-full.sh` exit 0.
- GitHub Release `v0.101.16` con assets.
- C8-R firmado por autoridad humana (decisión operador).

## Out of scope

- C6 (web/landing).
- Nuevo infra cloud (prohibido).
- Cierre contractual sin firma del operador.

## Riesgos

- **R-B6-W1**: el servidor MCP actual puede no soportar algunos requisitos (cancelación, paginación). Si el gap es grande, replantear el alcance antes de cerrar.
- **R-B6-W6**: el operador puede no firmar C8-R en el momento. Documentar y no cerrar el programa hasta tener la firma.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| _pendiente_ | B6.W1 | _pendiente_ | _pendiente_ |
| _pendiente_ | B6.W2 | _pendiente_ | _pendiente_ |
| _pendiente_ | B6.W3 | _pendiente_ | _pendiente_ |
| _pendiente_ | B6.W4 | _pendiente_ | _pendiente_ |
| _pendiente_ | B6.W5 | _pendiente_ | _pendiente_ |
| _pendiente_ | B6.W6 | _pendiente_ | _pendiente_ |

## Cross-block signals

B6.W6 cierra el programa con la firma contractual de C8-R. Sin firma, el programa queda `BLOCKED` con disparador explícito (firma del operador pendiente).