# roadmap-coherence-ratchet — Sincronizar ROADMAP con la rama activa

## Goal

Cerrar el drift entre `docs/roadmap/ROADMAP.md` (congelado en v0.99.2, 2026-09-27) y la rama `integrate/v1015` (v0.101.9, 80 commits ahead, 9 releases patch consumidos). El propio ROADMAP §6 prohíbe "dos fuentes de verdad" — la situación actual incumple ese anti-patrón.

## Problem

- **ROADMAP §2** lista el estado hasta v0.99.2 (released 2026-09-27).
- **Integrate branch HEAD** está en v0.101.9 (`Cargo.toml [workspace.package].version`).
- **80 commits ahead** de `origin/main`, mayormente `docs(roadmap)` (22 recibos), `release:` (9 cortes), `fix(release)` (8), `fix(ci)` (6), `fix(candidate)` (5).
- **`CURRENT.md`** snapshot explícitamente obsoleto (lo declara en su cabecera: "regenerar antes de citar"). Cita v0.99.1 como binario y 5651 tests como cifra del snapshot 2026-09-27.
- Sin gate automático: un PR puede divergir workspace-version de ROADMAP sin que nada se entere.

## Why

El operador pide ejecutar las 3 recomendaciones del informe previo
(generar CURRENT, gate de coherencia, reescribir §8 como histórico).

## Scope

### R1 — Regenerar `docs/roadmap/CURRENT.md`
- Sustituir snapshot obsoleto por uno basado en HEAD real `15b5c68c`.
- Actualizar cifra de tests al estado medido (2252 en `core/lib`, ~5650 workspace).
- Versión binario v0.101.9.
- Outcomes production-ready: marcar QW-N ejecutados, dejar ST-N parciales.

### R2 — Gate de coherencia version ↔ ROADMAP
- Crear `scripts/ci/test_roadmap_version_ratchet.py` (siguiendo patrón de `test_skill_surface_claims.py`).
- Contrato: pinea que `Cargo.toml [workspace.package].version` aparece como entrada reciente en ROADMAP o JOURNAL reciente (≤14 días de drift aceptable).
- 4 mutaciones vistas caer (workspace version ahead, ROADMAP stale, sin entradas de versión, ventana temporal rota).

### R3 — Reestructurar `docs/roadmap/ROADMAP.md`
- Marcar §8 "Programa production-ready (2026-09-26)" como **HISTÓRICO** con sello de cierre 2026-10-09, sin reescribir historia.
- Añadir §9 "Serie v0.101.x" como agenda viva: outcomes abiertos, work items en curso, decisiones pendientes.
- Mantener §1..§7 (principios, criterios, anti-patrones, referencias) intactos.

## Constraints

- **No reescribir historia**. ROADMAP §2..§8 reflejan lo que se decidió en su fecha; los nuevos apartados solo **anotan delta** o **declaran histórico**.
- **No reabrir PRF ni C# firmadas**.
- **No fusionar** `CURRENT.md` con `ROADMAP.md` (anti-patrón §6).
- **No introducir otro CI workflow** — el repo no tiene `.github/workflows/` desde #340. El gate se añade al catálogo `scripts/ci/run-all-contracts.sh` si existe, o como ejecutor manual pineado por el operador.
- Idioma neutro en artefactos (inglés por convención de docs/roadmap heredada; CURRENT.md mantiene español por su cabecera).

## Acceptance criteria

1. `docs/roadmap/CURRENT.md` declara explícitamente el SHA `15b5c68c`, versión v0.101.9, tests workspace al día.
2. `python3 scripts/ci/test_roadmap_version_ratchet.py` pasa verde en limpio, falla en 4 mutaciones.
3. `docs/roadmap/ROADMAP.md` tiene §9 nuevo y §8 marcado con sello histórico.
4. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` siguen exit 0 (no se toca Rust).
5. `git diff --stat` solo toca: `docs/roadmap/CURRENT.md`, `docs/roadmap/ROADMAP.md`, `scripts/ci/test_roadmap_version_ratchet.py`.

## Tasks

- [x] R1.1 — Regenerar `docs/roadmap/CURRENT.md`
- [x] R1.2 — Validar formato (frontmatter, secciones, sin contradicciones con ROADMAP §2)
- [x] R2.1 — Crear `scripts/ci/test_roadmap_version_ratchet.py` con 4 mutaciones
- [x] R2.2 — Validar: verde en limpio, rojo en cada mutación (4 verificadas, control verde)
- [x] R3.1 — Sellar §8 ROADMAP como HISTÓRICO
- [x] R3.2 — Añadir §9 "Serie v0.101.x" con agenda viva
- [x] R3.3 — Validar coherencia: ninguna afirmación de §9 contradice §1..§7

## Out of scope

- Reescribir `docs/prf/` (es histórico por contrato).
- Cambiar `Cargo.toml [workspace.package].version` (es decisión del operador).
- Mover ROC `e1dd8169` ni `4ee9ca61` (ya merged).
- Bumps SemVer (decisión operador).
- Fusionar PRs del roadmap al `main`.

## Evidence

| Check | Resultado |
|---|---|
| `python3 scripts/ci/test_roadmap_version_ratchet.py` (HEAD limpio) | exit 0: `workspace=0.101.9, head=15b5c68c9fde` |
| M2 mutación: SHA incorrecto en CURRENT | exit 1: `CURRENT.md declara SHA 000000000000, HEAD es 15b5c68c9fde` |
| M3 mutación: snapshot stale (2020-01-01) | exit 1: `lleva 2473 dias sin regenerarse` |
| M4 mutación: workspace version huérfana (0.999.99) | exit 1: `no aparece en ROADMAP.md ni en JOURNAL.md` |
| Triple mutación combinada | exit 1: 3 assertions rojas simultáneas |
| `cargo fmt --check` / `cargo clippy -D warnings` | sin cambios Rust, no aplica |
| `git diff --stat docs/ scripts/ci/test_roadmap_version_ratchet.py Cargo.toml` | `docs/roadmap/CURRENT.md 154 cambios`, `docs/roadmap/ROADMAP.md +83/-0`, `Cargo.toml` intacto |