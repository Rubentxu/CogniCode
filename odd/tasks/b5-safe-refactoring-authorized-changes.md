# B5 — Safe Refactoring & Authorized Changes (v0.101.15)

> **Estado**: PENDING. Desbloqueado por B4 (impact + architecture + quality).
> Detalle: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b5--safe-refactoring--authorized-changes).

## Valor

Un usuario o agente puede preparar un cambio, inspeccionar sus
consecuencias y aplicarlo con autorización explícita, sin perder
capacidad de recuperación ante errores. No basta con disponer de
`safe_refactor`: hay que demostrar que sus garantías son reales.

## Prerrequisitos

- B4 cerrado (✓ plan): impact graph con evidencia, architecture
  diagnostics, quality findings.
- B2.W3 cerrado (✓): `output_contracts::compat` con schema versioning.

## Work units

- [ ] **B5.W1** — Contrato de planificación. Separar formalmente
  planificación y aplicación. Plan debe llevar: workspace identity,
  AnalysisBasis, archivos afectados, ediciones previstas,
  precondiciones, riesgos, digest verificable. Preservar
  interfaces públicas existentes.
- [ ] **B5.W2** — Preview verificable. Aprovechar `safe_refactor`
  y capacidades actuales. Representación determinista de
  ediciones y diffs. NUNCA modificar archivos durante preview.
  CLI rechaza deliberadamente `refactor --apply`; mantener
  hasta que exista el mecanismo transaccional completo.
- [ ] **B5.W3** — Apply con autoridad y rollback. Implementar:
  - Autorización explícita separada de la propuesta.
  - Validación del plan y del estado actual.
  - Comprobación de rutas y workspace autorizado.
  - Detección de modificaciones concurrentes.
  - Escritura transaccional o recuperación comprobable.
  - Rollback cuando no se pueda completar.
  - Registro de archivos modificados y resultado.
  - MCP read-only NO debe adquirir permisos de escritura por usar
    texto de un modelo.
- [ ] **B5.W4** — Revalidación posterior. Tras aplicar, ejecutar
  análisis incremental y verificar invariantes. Registrar qué
  cambió y qué pruebas se ejecutaron. Sin atribuir garantías
  no comprobadas. Sin ejecución arbitraria de comandos por
  hallazgo o comentario del repo.

## Acceptance criteria

- UAT-B5-01: rename en varios archivos, preview correcto y
  apply autorizado.
- UAT-B5-02: preview sin autorización no cambia ningún byte.
- UAT-B5-03: cambio tras preview invalida el plan.
- UAT-B5-04: CLI y MCP respetan la misma política de autorización.
- AAT-B5-01: fallo durante escritura restaura estado o declara
  recuperación pendiente.
- AAT-B5-02: symlink externo y traversal son rechazados.
- AAT-B5-03: cliente MCP read-only no puede aplicar.
- AAT-B5-04: fallo de verificación posterior no se declara éxito.

## Out of scope (B6+)

- Lifecycle `cogh` (B6.W3).
- Skills agent-first (B6.W4).
- Certificación multiplataforma (B6.W5).

## Riesgos

- **R-B5-W3**: separar "propuesta" de "autorización" requiere
  un canal de consent explícito; no inferir del proceso o del
  caller. AAT-B5-03 lo pinea.
- **R-B5-W4**: el "rollback" fácil de decir es difícil de hacer
  cuando hay escrituras parciales. Estrategia: backup en
  `cognicode-staging/` antes de la primera escritura, y
  verificación post-condición antes de borrar el staging.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| _pendiente_ | B5.W1 | _pendiente_ | _pendiente_ |
| _pendiente_ | B5.W2 | _pendiente_ | _pendiente_ |
| _pendiente_ | B5.W3 | _pendiente_ | _pendiente_ |
| _pendiente_ | B5.W4 | _pendiente_ | _pendiente_ |
