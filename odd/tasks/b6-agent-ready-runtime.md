# B6 — Agent-Ready Runtime & Portable Distribution (v0.101.16)

> **Estado**: PENDING. Desbloqueado por B5 (safe refactor + authorized apply).
> Detalle: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b6--agent-ready-runtime--portable-distribution).

## Valor

CogniCode puede instalarse, actualizarse y usarse de forma
reproducible desde agentes reales sobre repositorios de distintos
tamaños. Las consultas son acotadas, cancelables y observables,
sin depender de Explorer ni de servicios externos obligatorios.

## Prerrequisitos

- B5 cerrado (✓ plan): safe refactor + authorized apply.
- B6.W1 (B1.W3b absorbido): smoke MCP `initialize` / `tools/list`
  en proceso separado.

## Work units

- [ ] **B6.W1** — Robustez MCP. Verificar con servidor real:
  init + cierre limpio, JSON-RPC solo por stdout, errores
  estructurados, timeouts + cancelación, límites de concurrencia /
  profundidad / fanout / memoria, paginación + continuación,
  aislamiento entre sesiones, no procesos huérfanos. NO daemon
  adicional si stdio MCP actual cubre.
- [ ] **B6.W2** — Rendimiento reproducible. Benchmarks en corpus
  pequeño, mediano, grande: cold / hot / incremental / search /
  navigation / graphs. Baselines + presupuestos por categoría.
  Mecanismos e91 existentes, no framework nuevo. Evidencia G5.
- [ ] **B6.W3** — Lifecycle `cogh`. Instalación limpia, update,
  pinning por versión, compatibilidad entre IDEs, error
  detection, doctor, rollback. Usuario nuevo obtiene primera
  consulta real sin compilar CogniCode ni leer instrucciones
  internas.
- [ ] **B6.W4** — Skills y autodescubrimiento. Alinear skills
  existentes con capacidades reales CLI/MCP. Evaluaciones
  agent-first: descubre tools, selecciona, interpreta parcial,
  obtiene info útil. NO instrucciones que presupongan
  experimentales como estables.
- [ ] **B6.W5** — Certificación de distribución. Verificar
  paquetes Linux publicados y canales adicionales soportados.
  Plataformas no disponibles: `NOT_RUN` o `SKIP_NOT_APPLICABLE`
  con justificación. NO declarar certificación multiplataforma.
  CP2/CP4/CP5 aplicables; CP6 (web/landing) out of scope.
- [ ] **B6.W6** — Cierre arquitectónico. Medir deuda residual en
  grandes coordinadores. Completar separaciones ST-02/ST-04
  que afecten CLI/MCP. NO dividir por tamaño. Solo separar si
  elimina acoplamiento medido y conserva contratos de
  aplicación. C8-R con evidencia reproducible.

## Acceptance criteria

- UAT-B6-01: instalación desde artefactos publicados, sin
  toolchain Rust.
- UAT-B6-02: agente externo establece conexión MCP y completa
  análisis de código.
- UAT-B6-03: cliente CLI obtiene resultados equivalentes sobre
  el mismo workspace.
- UAT-B6-04: update y rollback de versión funcionan.
- UAT-B6-05: campaña de rendimiento con scorecard reproducible.
- AAT-B6-01: cancelar análisis largo libera recursos.
- AAT-B6-02: desconectar cliente MCP no deja operaciones activas.
- AAT-B6-03: entrada grande o maliciosa no supera límites.
- AAT-B6-04: fallo de telemetría opt-in no impide iniciar el core.
- AAT-B6-05: paquete manipulado es rechazado por lifecycle.
- AAT-B6-06: agente distingue `Partial|Unknown|Unsupported|Failed`.

## Out of scope

- CP6 (web/landing).
- Nuevos lenguajes.
- Multi-tenancy.

## Riesgos

- **R-B6-W1**: introducir un daemon para resolver "robustez"
  es un atajo que duplica la superficie de fallo. Antes de
  añadir, medir que stdio MCP actual no satisface.
- **R-B6-W4**: skills prescriptivas que asumen capacidades
  experimentales como estables engañan al agente. Toda skill
  que afirme disponibilidad de una capability feature-gated
  debe declarar la feature explícitamente.
- **R-B6-W6**: dividir módulos por tamaño sin acoplamiento
  medido produce fragmentación sin valor. Esperar evidencia
  antes de separar.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| 2026-10-12 | B6.W1 | _pendiente_ | Robustez MCP verificada contra `cognicode-mcp` real (subprocess stdio): framing JSON-only stdout, init+shutdown rc=0 sin huérfanos, paginación cursors reales, timeouts fail-fast (<5s vs techo 60s), sesiones paralelas sin cruce, errores estructurados. NO daemon: `ServerLimits`/`ServerDiagnostics` viven en `crates/cognicode-core/src/interface/mcp/robustness.rs` (mismo proceso). 4 unit tests + 8 integration. `core_gate_coverage_contract` ANCHOR_SUITES += b6_w1_mcp_robustness. |
| _pendiente_ | B6.W2 | _pendiente_ | _pendiente_ |
| 2026-10-12 | B6.W3 | _pendiente_ | lifecycle.rs + cogh install --version/--rollback + tests + catalog |
| _pendiente_ | B6.W4 | _pendiente_ | _pendiente_ |
| _pendiente_ | B6.W5 | _pendiente_ | _pendiente_ |
| _pendiente_ | B6.W6 | _pendiente_ | _pendiente_ |
