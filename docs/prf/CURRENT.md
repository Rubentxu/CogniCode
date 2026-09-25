# CURRENT — Puntero operativo PRF

## Estado (2026-09-25 post-§154.H corrección honesta, sesión 7 operator-gated → cutover G0)
- **HEAD funcional**: `4d988409` (PR-CI workflow shipped, §154 V48)
- **Commits ahead of origin**: 0
- **Releases vigentes**: `v0.98.1` (production-ready contractual, firmada C7 2026-09-24T22:41:33Z), antecedente `v0.98.0`.
- **C7 firma**: PASS contractualmente por el operador `Ruben <rubentxu@cognicode.dev>` el 2026-09-24T22:41:33Z UTC con la cadena "firmo".
- **Tests verdes sobre HEAD actual**:
  - `cargo test -p cognicode-core --lib` → **2188 passed, 0 failed, 27 ignored**.
  - `cargo test -p cognicode-core --test prf_sec_07_adversarial_campaign` → **8/8 verde**.
  - `cargo test -p cognicode-core --lib prf_mcp_05` → **4/4 verde**.
  - **`cargo test -p cognicode-core --test prf_h06_adversarial_e2e` → 15/15 verde** (§153).
- **CI governance (estado REAL a 2026-09-25)**:
  - **PR-CI** (`pr-ci.yml`) — workflow shipped; **gate NO exigido al merge** (main sin branch protection ni required checks; `gh api .../branches/main/protection` → 404).
  - **CI local** (`ci.yml`) — LOCAL-ONLY con `act` / `just ci-local`.
  - **Release gate** (`release.yml` + `release-validate.yml`) — solo en tags.
- **Decisión operador 2026-09-25 (revisión de §154)**: PRF ya no debe seguir siendo el roadmap de desarrollo. La siguiente unidad es **G0 — Post-PRF Governance Cutover** (nuevo roadmap bajo `docs/roadmap/ROADMAP.md`), NO más self-rolls PRF.

## Decisiones operator-gated pendientes (NO cambian con el push)

| ID | Pendiente | Estado al 2026-09-25 |
|---|---|---|
| **P0.3 enforcement** | Branch protection en `main` + required status checks del PR-CI | PENDING (§154.H corrección honesta). El workflow existe pero `main` está sin protección. Cierre real: `G0.1` del nuevo roadmap. |
| **P0.4** | 0.97.x retirement | Operator-gated — ahora bajo `E0.W2` (política de soporte, no borrado de artefactos). |
| **ISSUE-1 cogh** | Bug `cogh rollback --to <same>` | Ahora bajo `M0.1` (mantenimiento `v0.98.x`). |

Los antiguos P0.1 (RELEASE-CANDIDATE freshen) y P0.5 (firma C7) están cerrados por §149/§150 y §151 respectivamente. P0.2 (H06 adversarial E2E) cerrado por §153. P0.3 workflow cerrado por §154; solo queda el enforcement.

## Bloqueos abiertos

**P0.3 enforcement**. C7 sigue firme. El binario release v0.98.1 sigue siendo production-ready contractual.

## Próximo trabajo ejecutable en AUTO

El orden fijado por la revisión del operador 2026-09-25:

1. **`G0.1` enforcement real del PR-CI** — branch protection en main, required checks, PR de prueba con fallo intencionado para verificar bloqueo.
2. **`G0.2` cutover de gobernanza** — crear `docs/roadmap/ROADMAP.md`, reorientar `AGENTS.md`, congelar PRF como histórico.
3. **`G0.3` revalidar/archivar e90** (perf cold-cache).
4. **`G0.4` issues históricos #234/#235** contra v0.98.1.
5. **`M0.1` fix `cogh rollback --to <same>`** → v0.98.2.
6. **`E0..E2`** ver `docs/prf/POST-PRF-EVOLUTION.md`.

## Referencias (sesión 5, ciclo B1→B4→B5 + sesión 6/7)

**Sesión 6 (2026-09-25)** — cierre operator-gated H06 + PR-CI:
- `ddfa0cd8` §153 V47 PRF-H06 adversarial E2E suite (15/15 verde)
- `b4fa4ae3` §153 self-roll (STATE/CURRENT/JOURNAL/MATRIX)
- `4d988409` §154 V48 workflow PR-CI
- `f05c503b` §154 self-roll

**Sesión 7 (2026-09-25)** — revisión operador, corrección honesta §154.H, cutover G0:
- §154.H — P0.3 distinguido en workflow (CLOSED) y enforcement (PENDING).

## SHAs a verificar (estado 2026-09-25)

- HEAD local/origin: `f05c503b4fb1dde6c9aa5fc93ab1cf133e5aaeb4`
- Tag v0.98.1: `a21fccda` → commit `e4ab6c8e8d06b598ce55880d965d785c6710c777`
- Tag v0.98.0: `d99d3911` → commit `8505ad85`
- Commit reciente con código: `4d988409` (§154 V48 — `.github/workflows/pr-ci.yml`, 116 LOC)
- Branch protection en `main`: **NO ACTIVA** (404 desde API GitHub)
