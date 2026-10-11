# B5 — Safe Refactoring & Authorized Changes (v0.101.15)

> **Estado**: PENDING. Desbloqueado por B4.
> Detalle espeficado: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b5--safe-refactoring--authorized-changes).

## Valor

Un usuario o agente puede preparar un cambio, inspeccionar sus consecuencias y aplicarlo con autorización explícita, sin perder capacidad de recuperación ante errores. No basta con disponer de `safe_refactor`: hay que demostrar que sus garantías son reales.

## Prerrequisitos

- B4 cerrado: grafo de impacto + evidence export.

## Work units

- [ ] **B5.W1** — Contrato de planificación. Separar planificación y aplicación formalmente. Plan contiene: workspace identity + AnalysisBasis + archivos afectados + ediciones previstas + precondiciones + riesgos + identificador/digest. Preservar interfaces públicas.
- [ ] **B5.W2** — Preview verificable. `safe_refactor` + representación determinista de ediciones y diffs. NUNCA modificar archivos en preview. Mantener el rechazo deliberado de `refactor --apply` (PRF-CLI-05).
- [ ] **B5.W3** — Apply + autoridad + recovery. Autorización explícita separada de la propuesta. Validación del plan y del estado. Comprobación de rutas y workspace autorizado. Detección de modificaciones concurrentes. Escritura transaccional o recuperación comprobable. Rollback si no se completa. Registro de archivos + resultado. MCP read-only NO adquiere permisos de escritura.
- [ ] **B5.W4** — Revalidación posterior. Análisis incremental post-apply + verificación de invariantes. Registro de qué cambió + qué pruebas se ejecutaron. NO atribuir garantías no comprobadas. NO ejecución arbitraria de comandos como consecuencia de hallazgo/comentario.

## Acceptance criteria (resumen)

- UAT-B5-01..04: rename preview correcto; preview sin auth no muta; cambios post-preview invalidan plan; CLI/MCP política unificada.
- AAT-B5-01..04: escritura fallida restaura; symlink externo rechazado; MCP read-only no aplica; fallo de verificación NO se reporta como éxito.
- `bash scripts/test-full.sh` exit 0.

## Out of scope

- Editor UI / preview visual (eso es Explorer — excluido).
- Ejecución arbitraria de comandos del repo (prohibido por seguridad).

## Riesgos

- **R-B5-W3**: la escritura transaccional es difícil en Rust (no try-finally para archivos). Mitigación: journal con effectos aplicados y comprobación de reversibilidad pre-apply.
- **R-B5-W3**: detectar modificaciones concurrentes sin locks globales (que escalarían mal). Mitigación: comparar mtime + content hash en cada paso.

## Recibo (append-only)

| Fecha | WU | SHA | Notas |
|---|---|---|---|
| _pendiente_ | B5.W1 | _pendiente_ | _pendiente_ |
| _pendiente_ | B5.W2 | _pendiente_ | _pendiente_ |
| _pendiente_ | B5.W3 | _pendiente_ | _pendiente_ |
| _pendiente_ | B5.W4 | _pendiente_ | _pendiente_ |

## Cross-block signals

B5.W3 + B5.W4 son prerrequisito de B6.W3 (lifecycle de cogh debe respetar la misma política de autorización).