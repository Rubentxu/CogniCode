# B0 — Release Trust v0.101.17 — CLOSED

> **Fecha de cierre**: 2026-10-16 (CEST)
> **Estado**: **DONE**
> **Tag**: `v0.101.17` (anotado, en `origin`)
> **Release commit**: `36c5608b5587aba9bbe9a373743d43be848a1db4`
> **Pipeline-fix commits (post-build)**: `2bbcb767`, `9367e571`
> **GitHub Release**: https://github.com/Rubentxu/CogniCode/releases/tag/v0.101.17
> **Branch**: `release/v0.101.17` en `9367e571`

## Valor entregado

Una release cuyo SHA de etiqueta, SHA de build, SHA del `source_commit`
del manifest, lista de assets y binarios publicados son **una sola identidad
verificable** — no un conjunto de claims que se contradicen entre sí.
La pipeline del release-candidate quedó en estado de que el recibo que
emite coincide con el SHA que la preflight validó y los binarios que
produjo.

## Prerrequisitos (todos verificados)

- v0.101.10 publicado con 3 de los 8 assets (binarios, sin bundle ni
  inventory ni portable skill bundles).
- 9 patches v0.101.0..v0.101.9 consumidos sin tocar la release lane.
- B1 (Contract & Release Trust) CLOSED contractualmente con v0.101.11.
- `scripts/ci/release-install-smoke.sh` durable y en CI.

## Bugs reales del release-candidate pipeline detectados y corregidos

Tres bugs de la pipeline quedaron sin manifest durante los RCs v5..v8.
Esta sesión los identificó y los arregló en commits limpios sobre
`release/v0.101.17`:

1. **`source_commit` no se ataba al `RELEASE_SHA`**.
   El preflight (línea 195) y la tag-coherence (línea 212) leían el SHA
   de `RELEASE_SHA`, pero la `generate` del manifest usaba `git rev-parse
   HEAD`, desacoplando el SHA del recibo del SHA que la preflight
   validaba. v8 se lanzó contra `36c5608b` y dos commits docs-only
   cayeron durante el run (`2b1686a4`, `fe1def48`); el manifest
   hubiera grabado `fe1def48` (un B4 plan) en vez de `36c5608b`. Fix:
   `source=$(git rev-parse "${RELEASE_SHA:-HEAD}")`. Commit `06ad21bf`
   (amendado en `2bbcb767`).
2. **`TIER1_TRIPLES_OVERRIDE` se aplicaba al comando equivocado**.
   Línea 841: `TIER1_TRIPLES_OVERRIDE="..." $cd && bash ...`. La
   asignación prefija `cd` (y solo `cd`); `bash` corre sin la env, el
   script cae al default Tier 1 (x86_64 + aarch64), la lane aarch64
   no existe (scope x86_64-only per TF-5b) y muere con "no lane
   claimed aarch64". Fix: mover la asignación a la invocación de
   `bash`. Commit `9f441376` (amendado en `2bbcb767`).
3. **`cognicode-release generate` requería `--platform` explícito en
   scope x86_64-only**. Línea 922: el default es Tier 1; pasaba lo
   mismo que en el caso anterior pero en la generación del manifest.
   Fix: `--platform "${targets.joinToString(",")}"`. Commit `9367e571`.

El test durable `test_release_candidate_asset_set.py` se reforzó para
que cuando `RELEASE_SHA` está en el entorno, afirme
`inventory.source_commit == RELEASE_SHA`. Backward-compatible: sin
`RELEASE_SHA`, el predicado original (alcanzable desde HEAD) se
preserva. Verificado con dos casos sintéticos — falla con el caso
incorrecto, pasa con el match.

## Acceptance criteria — VERIFIED

| Criterio | Resultado |
|---|---|
| Tag anotado `v0.101.17` → `36c5608b` | DONE |
| 8 assets en `release/` | DONE (3 component tarballs, 2 portable skill bundles, bundle manifest, release inventory, SHA256SUMS) |
| `release-inventory-0.101.17.json` con `source_commit = 36c5608b...` | DONE |
| `test_release_candidate_asset_set.py` con `RELEASE_SHA` | exit 0, 3/3 artifacts, source_commit matches |
| `test_release_lane_wiring.py` | exit 0, 4/4 |
| `test_release_candidate_layout.py` | exit 0 |
| `test_release_truth_convergence.py` | exit 0 |
| `test-harness/.cycles/B0-3f833bf/B0-UAT-01.sh` (8 assets) | exit 0 |
| `test-harness/.cycles/B0-3f833bf/B0-AAT-01.sh` (2 mutations) | exit 0 |
| `test-harness/.cycles/B0-3f833bf/B0-E2E-01.sh` (install + --version/--help) | exit 0 |
| `test-harness/.cycles/B0-3f833bf/B0-FIX-01.sh` (TIER1 fix + backward compat) | exit 0 |
| `bash scripts/ci/release-install-smoke.sh 0.101.17 release/` (local) | exit 0 (install/doctor/CLI/MCP/update/reshim/uninstall) |
| Push branch `release/v0.101.17` → `9367e571` | DONE |
| Push tag `v0.101.17` → `origin` | DONE |
| GitHub Release `v0.101.17` con 8 assets | DONE |
| SHA256SUMS check sobre los assets descargados de GitHub | exit 0 (7/7 matches) |
| Post-publish install-smoke contra assets de GitHub | exit 0 |

## Efímeros retirados

`test-harness/.cycles/B0-3f833bf/` (B0-UAT-01, B0-AAT-01, B0-E2E-01,
B0-FIX-01) se elimina en este commit. Las cuatro propiedades que
cubrían están pineadas en durables:

- Asset set (8 expected) → `scripts/ci/test_release_candidate_asset_set.py`
  + `sandbox/scripts/test_release_asset_set.sh` (durable, corre por tag)
- Mutaciones in-process → `scripts/ci/test_release_lane_wiring.py`
  + durable mutations regression
- Install + version/help → `scripts/ci/release-install-smoke.sh`
- TIER1 fix → `sandbox/scripts/test_release_asset_set.sh` corre por tag

## Out of scope (B0 no es responsable)

- Push a `origin/main`. El branch `release/v0.101.17` se queda con la
  base `36c5608b`; el merge a `main` es trabajo del bloque B1
  (siguiente) o de un PR específico. El operador decide.
- Certificación C8 (Post-PRF GA) — orthogonal, sigue en estado
  CERRADO TÉCNICAMENTE / PENDING firma humana.

## Riesgos (cerrados al cierre)

- Pipeline bug de `source_commit` parcheado, con test durable reforzado.
- Pipeline bug de `TIER1` parcheado, con test efímero B0-FIX-01 que
  reproduce el bug original y confirma el fix + backward compat.
- Pipeline bug de `--platform` parcheado, con test durable
  `cognicode-release plan` que se puede usar para regresión.
- Binarios publicados: 7/7 SHA256SUMS verificados al cierre.
- `cognicode --version` y `cogh --version` confirmados idénticos a
  la versión declarada en el tag y el manifest.

## Próximo bloque desbloqueado

B1 (Contract & Release Trust) ya está CLOSED contractualmente
(`odd/tasks/b1-contract-release-trust.md`, commit
`63286bca` con v0.101.11). El siguiente bloque no certificado en
el árbol Post-PRF es B2 (Shared Analysis & Capability Contracts),
visible en `docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`. Su
desbloqueo depende de que la release v0.101.17 sea consumida por
un PR contra `origin/main`, decisión del operador.

## Recibo de cierre (append-only)

| Fecha | SHA | Notas |
|---|---|---|
| 2026-10-16 | `36c5608b` | Binarios construidos desde este commit; release SHA del manifest y del tag |
| 2026-10-16 | `2bbcb767` | `fix(release-candidate): scope TIER1 var to bash, capture source_commit from RELEASE_SHA` (patches 1+2 amendados) |
| 2026-10-16 | `9367e571` | `fix(release-candidate): scope \`generate\` to lane's platforms` (patch 3) |
| 2026-10-16 | `f5921cae` | Tag `v0.101.17` push OK a `origin`; SHA-256 tag object `ea6487e1...` |
| 2026-10-16 | `5b4d2172` | (este commit) cierre de bloque B0 |
