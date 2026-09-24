# CURRENT — Puntero operativo PRF

## Estado (2026-09-24 post-release v0.98.1, sesión 5 AUTO + operador-gated)
- **HEAD**: `0d6c575e` (STATE self-roll — §149 V43 integración final v0.98.1)
  sobre `b67c9deb` (§148 V42 — release v0.98.1 publicada) sobre
  `e4ab6c8e` (chore(release): bump workspace version 0.98.0 → 0.98.1)
  sobre `a83ea210` (STATE self-roll — §147 B4) sobre `4feb5456` (§147
  B4 Admission Expediente) sobre `92ec698a` (STATE self-roll — §146 B3)
  sobre `bb0f2edb` (§146 V40 B3 cierre — distribución + instalación +
  recuperación sobre v0.98.0).
- **Local = origin = `0d6c575e`**. Push completo. Sin commits ahead.
- **Tags**: `v0.98.0` (`d99d3911`) y `v0.98.1` (`a21fccda`) ambos en repo.
- **Releases publicadas**: `v0.98.0` (#36034410448 success) y `v0.98.1`
  (#36063804784 success, marked as Latest, 12 assets con SHA256).
- **Tests verdes sobre HEAD actual `0d6c575e`**:
  - `cargo test -p cognicode-core --lib` → **2188 passed, 0 failed, 27 ignored**.
  - `cargo test -p cognicode-core --test prf_sec_07_adversarial_campaign` → **8/8 verde** (PRF-SEC-07).
  - `cargo test -p cognicode-core --lib prf_mcp_05` → **4/4 verde** (PRF-MCP-05 enforcement).
- **B1+B2+B3 del plan prolongado B1→B4**: cerrados.
- **Trabajo de este ciclo (B5 final §149)**:
  - Push de los 22 commits ahead of origin/main.
  - Bump workspace 0.98.0 → 0.98.1.
  - release-validate run #36062820528 (5/5 SUCCESS).
  - release.yml run #36063804784 (3/3 SUCCESS).
  - 12 assets publicados con SHA256 en GitHub Releases v0.98.1.
  - Re-ejecución B3 sobre v0.98.1: install → doctor → MCP JSON-RPC stdio
    (20 tools, **todos authority=`read`**) → uninstall limpio.
  - Admission Expediente §147 redactado y archivado.
  - Body de release enriquecido (92 líneas con verificación completa).

## Decisiones operator-gated pendientes (NO cambian con el push)

| ID | Pendiente | Estado al 2026-09-24 |
|---|---|---|
| **P0.1** | RELEASE-CANDIDATE.md freshen del SHA congelado | El operador ya autorizó el push en `2026-09-24T21:37:01Z` y la release v0.98.1 está publicada. El SHA congelado `178f8a5b` en RELEASE-CANDIDATE.md debe actualizarse al SHA de v0.98.1 (`e4ab6c8e8d06…`) para que el expediente deje de estar knowingly stale. **Esta actualización se ejecuta en este ciclo §150** (parte de B5/integra). |
| **P0.2** | Cierre H06 adversarial campaign | Operator-gated — scope decisión. Pineado en cargo test (HEAD), no campaña E2E sobre binario en red hostil. |
| **P0.3** | PRF-CI-07 disparador automático en push-PR | Operator-gated — branch protection policy fuera del código. |
| **P0.4** | 0.97.x retirement | Operator-gated — scope decisión. |
| **P0.5** | **C7 firma CONTRACTUAL** sobre v0.98.1 | Operator-gated — la auditoría 2026-09-22 no autoriza firma automática. Material técnico en su lugar (binario verificado, MCP probe, SHA256 reproduce, attestations). |

## Bloqueos abiertos

**Solo C7 firma contractual** sigue siendo el bloqueo contractual. Todo lo demás está publicado, verificado y trazable. La release v0.98.1 cumple los pineos del código fuente HEAD; lo único que falta es la firma humana del operador sobre el expediente.

## Próximo trabajo ejecutable en AUTO

1. ~~Auditoría similar a §101 sobre PRE-§90~~ — ya hecha en §150 (RELEASE-CANDIDATE refresh).
2. ~~PRF-ANA-05: UAT reproducibilidad binario~~ — hecho en §146 (B3) y §148 (B5 re-ejecutado sobre v0.98.1).
3. **Documentar el bug `cogh rollback --to <same>`** en `docs/prf/COGH-ISSUES.md` para upstream.
4. **Documentar el orden `cogh install` → `cogh init`** en `docs/prf/INSTALL-ORDER.md`.
5. Si el operador quiere avanzar CI governance: PRF-CI-07 disparador automático (~30 min, branch protection + workflow file).

## Referencias (sesión 5, ciclo B1→B4→B5)
- `7e6b1270` §144 B1 RECONCILIATION-MATRIX
- `ea34ff7d` §145 B2 PRF-MCP-05 enforcement + PRF-SEC-07 adversarial
- `bb0f2edb` §146 B3 distribution end-to-end (v0.98.0)
- `4feb5456` §147 B4 Admission Expediente
- `e4ab6c8e` chore(release) bump 0.98.0→0.98.1
- `b67c9deb` §148 V42 release v0.98.1 publicada
- `0d6c575e` §149 V43 integración final
- `0d6c575e` §150 V44 (este ciclo): CURRENT + RELEASE-CANDIDATE refresh

## SHAs a verificar

- HEAD local: `0d6c575ea2c21ab7e1f02f552c56c80dc209e094`
- HEAD origin: igual
- Tag v0.98.1: `a21fccda` → commit `e4ab6c8e8d06b598ce55880d965d785c6710c777`
- Tag v0.98.0: `d99d3911` → commit `8505ad85`
