# K0 — Release v0.101.22 (Corte línea Kotlin K0–K3)

**Fecha:** 2026-10-17
**Versión:** v0.101.22 (PATCH bump)
**SHA post-merge esperado:** `860ff5c5a1c72bbf55bf2259c00861ac09933a9f`
**Estado del gate de release:** en curso al cierre de este veredicto.
**Unidad de trabajo:** K0 (línea Kotlin subordinada a CU-2),
autoridad
[`docs/roadmap/consumer-contracts/ROADMAP.md`](../../consumer-contracts/ROADMAP.md)
§Línea Kotlin K0–K3.

## Goal

Cerrar K0 de la línea Kotlin (cobertura honesta) en una release
production-grade que el consumidor pueda instalar y verificar. El
producto admite un nuevo estado de walk (`KnownUnsupportedGap`),
retira el anuncio falso de soporte Kotlin, y documenta el subprograma
Kotlin K0–K4 con baseline actualizado a v0.101.21 y K0 marcado
honestamente con su criterio parcial + K0.1 propuesto.

## Cambio

Veredicto K0 funcional:
[`docs/roadmap/production-ready/veredictos/2026-10-17-k0-honest-coverage.md`](2026-10-17-k0-honest-coverage.md)
(caracterización, RED→GREEN, UAT efímero, defectos separados).

Release-candidate payload:

- 8 assets en el bundle (cognicode, cognicode-mcp, cogh + manifests).
- `product/languages.json`: 21 lenguajes, kotlin fuera, kt/kts
  documentados como gap tipado.
- `product/platforms.json`: regenerado con `source_commit` apuntando
  a HEAD de release.
- `product/product-manifest.json`: version + source_commit a
  `4e1821f534b443328f1f4e4299731af09174b1a1` (K0 squash).

## Lo que el consumidor ve

- `cogh --version` → `0.101.22`.
- `cognicode --format json index build <repo_mixto>` reporta
  correctamente las fuentes `.kt`/`.kts` como gap tipado (no
  inflado a `Complete`, no silenciado en `Unsupported`).
- `cognicode --format json graph per-file <fuente.kt>` →
  respuesta explícita de "not supported" con razón
  `KnownUnsupportedGap`, sin falsa completitud.
- `product/languages.json` muestra 21 entradas, sin kotlin.
- `CAPABILITIES-MATRIX.md` (`docs/prf/specs/`) sincronizado: 21
  entradas, kotlin documentado como gap hasta K1.

## Tests pre-tag

- `python3 scripts/ci/test_roadmap_version_ratchet.py` → `OK
  (workspace=0.101.22, head=4e1821f534b4)`.
- `bash scripts/ci/run-all-contracts.sh` → 33 contratos, 0 failed.
- `merge-gate` sobre PR #377 verde en `52a75343`; sobre PR #378
  (bump) en curso al cierre.

## Defectos separados (no resueltos aquí)

1. **PRF f6.w1** (`prf_f6_w1_release_coherence.rs`): test huérfano
   asume `cogh generate`, subcomando retirado del binario actual.
   Pre-existente en main desde `c28f2143` (V32.5). NO está
   enumerado en `merge-gate.pipeline.kts:604–656` (33 --test
   target), por lo que NO bloquea el gate aunque aparezca rojo en
   `cargo test --workspace`. Acción: ticket separado (tooling vs
   test-harness; ADR pendiente).
2. **K0 criterios restantes** (taxonomía 7+ variantes, 
   `CompleteForScope`, diagnóstico CLI/MCP): propuestos como
   **K0.1** (BUGFIX / release intermedio, mismo PATCH SemVer) en
   `docs/roadmap/consumer-contracts/kotlin-evolution/MILESTONES.md`.
3. **K1 tree-sitter-kotlin**: pendiente. Sin K1, K0–K0.1 son
   honestos pero parciales — kotlin queda en `KnownUnsupportedGap`.

## Evidencia esperada al cierre de release

- Tag anotado `v0.101.22` apuntando a `860ff5c5` (o al SHA que el
  squash de PR #378 produzca; los dos posibles: 4-arg squash
  message `release: bump 0.101.21 -> 0.101.22 (corte K0 línea Kotlin)`).
- GitHub Release con 8 assets y `SHA256SUMS`.
- `test_release_asset_set.sh v0.101.22` → PASS.
- `cognicode`/`cognicode-mcp`/`cogh` instalados reportan 0.101.22;
  `--format json index build` sobre un repo mixto muestra el
  nuevo campo de gap tipado.

## Progreso del roadmap

CU-1: 5/5 WUs (v0.101.19). CU-2: 5/5 WUs (v0.101.21). CU-3: 0/4.
CU-4: 0/5. **Línea Kotlin K0–K3: 1/4 WUs (K0 cerrado en v0.101.22;
K0.1 propuesta, K1–K3 pendientes)**. Siguiente unidad: K0.1
(taxonomía por fichero + `CompleteForScope`) o K1
(tree-sitter-kotlin) — la decisión de operator es por cuál abrir
el siguiente ciclo.

## Hitos relacionados

- K0 merge: PR #377 squash `4e1821f5`.
- Bump commit: `860ff5c5a1c72bbf55bf2259c00861ac09933a9f` (squash de
  PR #378 post-gate).
- Veredicto de release: este fichero, una vez squash de PR #378
  produzca un SHA anotado en GitHub.
- `docs/roadmap/CURRENT.md` regenerado para declarar el nuevo HEAD
  post-merge.
- `docs/roadmap/JOURNAL.md` Entry 54 (recibos append-only).
