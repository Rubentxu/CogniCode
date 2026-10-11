# Feature: SDDK Ledger Reconciliation (2026-09-28)

> **Goal:** Alinear el ledger SDDK (`p-c1fac1fea05615c6`) con la realidad del repositorio, cerrando formalmente los ciclos cuyo código está mergeado en `origin/main` pero quedaron sin transición SDDK.

## Estado al inicio

| Ciclo | Estado SDDK | Fase | Artefactos existentes | Trabajo real mergeado |
|---|---|---|---|---|
| `cp2-public-contract-hardening` (A-full) | OPEN | explore | 0 | A-009, A-010, A-011, CP2-DEBT-04, A-012 vía PRs #300-#305 |
| `m011-rustdoc-intra-doc-links` (A-lite) | OPEN | build | 3 (exploration, specification, design) | M0.11 mergeado en PR #299 (commits `f0dd2213`, `20a2aa60`) |
| `cp1-oss-foundation` (A-lite) | RELEASE_PENDING | release | 3 (exploration, specification, design) | CP1 mergeado en PR #299 (commit `95595af9`), espera decisión operador |

**Decisión:**
- `cp2-public-contract-hardening`: cerrar formalmente (explore → archive).
- `m011-rustdoc-intra-doc-links`: cerrar formalmente (build → archive).
- `cp1-oss-foundation`: NO cerrar unilateralmente — sigue `RELEASE_PENDING` esperando operador. Documentar su estado actual en el JOURNAL.

## Tasks

Ver `todo` projection (gestionado por el orchestrator).

## Work units

### WU-1: Ciclo `cp2-public-contract-hardening` (A-full)
- 7 artefactos a generar (exploration, specification, design, implementation-receipt, merge-receipt, release-receipt, archive-manifest)
- 5 transiciones (explore → specify → build → verify → release → archive)
- 4 work items en estado `done` (A-009, A-010, A-011, CP2-DEBT-04) + 1 `active` (A-012)

### WU-2: Ciclo `m011-rustdoc-intra-doc-links` (A-lite)
- 4 artefactos a generar (implementation-receipt, merge-receipt, release-receipt, archive-manifest)
- 4 transiciones (build → verify → release → archive)

### WU-3: Estado de `cp1-oss-foundation`
- Sin código nuevo
- Documentar en JOURNAL y en artefactos locales el estado actual (RELEASE_PENDING, esperando operador)

### WU-4: Cierre
- Validar ledger final
- Entry N+30 en JOURNAL.md
- mem_session_summary

## Out of scope

- Código de producto nuevo.
- Cualquier decisión sobre SemVer bump (es del operador).
- Crear un nuevo WU (A-013, etc.) — esto es housekeeping, no feature work.

## Riesgos

- Generar artefactos SDDK sin haber estado presente cuando se hicieron las transiciones puede omitir detalles (rationale de gates, evidence refs exactas). Mitigación: cada artefacto declara su evidencia observada en HEAD (`d1cd28aa`) y los PRs asociados.
- El ciclo `cp1-oss-foundation` puede quedar en estado inconsistente si no se cierra. Decisión: dejarlo explícitamente en RELEASE_PENDING con nota en JOURNAL.