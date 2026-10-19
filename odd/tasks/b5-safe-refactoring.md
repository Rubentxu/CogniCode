# B5 — Safe Refactoring & Authorized Changes (v0.101.15)

> **Estado**: CLOSED 2026-10-12 (v0.101.15 released).
> Estado derivado de `docs/roadmap/ROADMAP.md` §10, que es la autoridad;
> este fichero no lo declara por su cuenta.
> Detalle espeficado: [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](../docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md#bloque-b5--safe-refactoring--authorized-changes).

## Valor

Un usuario o agente puede preparar un cambio, inspeccionar sus consecuencias y aplicarlo con autorización explícita, sin perder capacidad de recuperación ante errores. No basta con disponer de `safe_refactor`: hay que demostrar que sus garantías son reales.

## Prerrequisitos

- B4 cerrado: grafo de impacto + evidence export.

## Work units

- [x] **B5.W1** — Contrato de planificación. Separar planificación y aplicación formalmente. Plan contiene: workspace identity + AnalysisBasis + archivos afectados + ediciones previstas + precondiciones + riesgos + identificador/digest. Preservar interfaces públicas.
- [x] **B5.W2** — Preview verificable. `safe_refactor` + representación determinista de ediciones y diffs. NUNCA modificar archivos en preview. Mantener el rechazo deliberado de `refactor --apply` (PRF-CLI-05).
- [x] **B5.W3** — Apply + autoridad + recovery. Autorización explícita separada de la propuesta. Validación del plan y del estado. Comprobación de rutas y workspace autorizado. Detección de modificaciones concurrentes. Escritura transaccional o recuperación comprobable. Rollback si no se completa. Registro de archivos + resultado. MCP read-only NO adquiere permisos de escritura.
- [x] **B5.W4** — Revalidación posterior. Análisis incremental post-apply + verificación de invariantes. Registro de qué cambió + qué pruebas se ejecutaron. NO atribuir garantías no comprobadas. NO ejecución arbitraria de comandos como consecuencia de hallazgo/comentario.

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
| 2026-10-12 | B5.W1 + B5.W2 | `d6bbb748` | Un solo commit para las dos: `refactor/plan.rs` (+ `b5_w1_plan_contract.rs`) y `refactor/preview.rs` (+ `b5_w2_preview_verifiable.rs`). El título del commit solo nombra W2; por eso un recibo posterior le atribuyó a un SHA distinto (ver "Corrección de recibo" más abajo). |
| 2026-10-12 | B5.W3 | `e11016ff` | `refactor/apply.rs` + `b5_w3_apply_with_authority.rs`: `AuthorizationToken` por canal separado, `workspace_state_digest` recomputado, symlink/traversal rechazados, staging + fsync + rename + rollback. Cierra AAT-B5-01/02/03. |
| 2026-10-12 | B5.W4 | `deaafca2` | `refactor/revalidate.rs` + `b5_w4_post_apply_revalidation.rs`: `verdict_refused` por defecto (AAT-B5-04), evidence-only, no ejecuta comandos arbitrarios. |
| 2026-10-12 | release | `7960031` | bump 0.101.14 -> 0.101.15; golden CBOR regenerado. |
| 2026-10-12 | close | `3e9fddc3` | `docs(odd): close B5 — release receipts + veredicto + tag SHAs`. Ver ROADMAP §10. |
| 2026-10-12 | tag | `v0.101.15` | anotado + pushed + GitHub Release con 3 tarballs. |

### Corrección de recibo (2026-10-19)

La spec detallada
[`b5-safe-refactoring-authorized-changes.md`](b5-safe-refactoring-authorized-changes.md)
credita las cuatro WU al SHA `0e5708d`. **Ese commit no está en la historia
del proyecto**: `git rev-list --all | grep -c 0e5708df` devuelve 0. Comparte
padre (`17b947bb`) y título con `d6bbb748`, que es el que sí aterrizó; es un
commit hermano descartado. El recibo de esa spec queda pendiente de que el
operador lo corrija — el fichero está bajo protección y no se reescribe sin su
decisión. Los SHA de la tabla de arriba son los verificados con
`git merge-base --is-ancestor <sha> main`.

## Cross-block signals

B5.W3 + B5.W4 son prerrequisito de B6.W3 (lifecycle de cogh debe respetar la misma política de autorización).