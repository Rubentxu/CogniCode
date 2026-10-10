# Feature: QW-03 — `.gitignore` guard testable y CI-integrated

> **Goal:** Cerrar formalmente QW-03 (`.gitignore` demasiado amplio → test contractual + merge-gate step), triangular el detector contra mutaciones que el guard puede no estar viendo, y cerrar el ciclo SDDK `production-ready-q3-2026`.

## Estado al inicio

- **Código:** El guard `scripts/ci/check-bin-tracking.sh` y su test contractual `crates/cognicode-cli/tests/qw03_bin_tracking_guard.rs` ya existen y pasan verde (7/7 tests OK, 2026-09-28). Los 3 tests específicos de QW-03 cubren:
  - Real repo (GREEN baseline)
  - Planted untracked bin (RED → guard falla)
  - Planted + tracked (GREEN, recovery)
- **CI:** Dos steps en `.github/workflows/pr-ci.yml` `merge-gate` (L3.W2 + L3.W2.2).
- **Commits:** `d781e846` (bin-tracking guard becomes testable) + `47085b0d` (integrate into pr-ci merge-gate) ya mergeados a `main` vía `arch/cr-06-application-fitness-functions`.
- **ROADMAP.md §8** declara QW-03 como `CLOSED 2026-09-26`.
- **Work item SDDK:** `7187a0ee-f86e-4435-9d16-dadca53fc76b` QW-03 → estado `"draft"`.
- **Ciclo SDDK:** `p-c1fac1fea05615c6/production-ready-q3-2026` → OPEN, fase `explore`, 0 artefactos, sin lease.

## Disciplina: Strict TDD

1. **RED explícito** — el test contractual `qw03_bin_tracking_guard_fails_when_a_bin_source_is_untracked` ya pina el RED (planted untracked → guard falla). Verificado verde.
2. **GREEN explícito** — el guard `scripts/ci/check-bin-tracking.sh` existe y retorna exit 0 sobre HEAD limpio. Verificado.
3. **TRIANGULATE** — añadir mutaciones que el guard NO detecta aún:
   - **T1 — Silent gitignore**: tracked pero `git check-ignore` retorna 0 (drift por ignore silencioso que el `git add` obvia). El script ya lo verifica (`git check-ignore`) pero el test no lo pina.
   - **T2 — New crate with bins, NOT listed in CRATES array**: si alguien añade un `[[bin]]` en un crate que el guard no inspecciona, el guard pasa verde silenciosamente.
   - **T3 — Path custom (no src/main.rs, no src/bin/<name>.rs)**: el default fallback puede ser erróneo si el path no es uno de los esperados.
4. **REFACTOR** — si las mutaciones revelan defectos, arreglarlos con tests primero.

## Tasks

Ver `todo` projection.

## Work units

- **WU-1: TRIANGULATE T1 (silent gitignore)** — añadir test que planta un bin tracked PERO gitignored y verifica que el guard falla.
- **WU-2: TRIANGULATE T2 (new crate no listado)** — añadir test que planta un bin en un crate nuevo que NO está en el array `CRATES` del guard y verifica que el guard falla. Esto requiere que el guard descubra crates automáticamente (no hardcoded).
- **WU-3: TRIANGULATE T3 (path custom)** — verificar que el default fallback funciona o extender el guard para aceptar paths custom.
- **WU-4: REFACTOR** — aplicar fixes si T2 revela que `CRATES` debe ser auto-descubierto.
- **WU-5: CI gate promotion** — verificar que el step ya corre en merge-gate (sí) y que el fallo aborta el merge.
- **WU-6: Cierre formal del ciclo SDDK** — artefactos + transiciones + archive.

## Out of scope

- Renombrar o mover `scripts/ci/check-bin-tracking.sh` a otro path.
- Cambiar la política L1.2 (steps en merge-gate, no required checks separados).
- Promover el guard a un required check separado (esa es una decisión separada de governance).

## Riesgos

- Si T2 revela que `CRATES` es hardcoded y el guard pasa silenciosamente cuando alguien añade un crate con bins, hay que arreglar el guard (descubrir crates vía `cargo metadata --no-deps` o similar). Mitigación: hacer el fix antes de cerrar.
- Si T3 revela un path custom malformado, hay que extender el guard. Mitigación: aceptar paths explícitos en `[[bin]] path = ...` y validar que existen.

## Recibo (al cierre)

| Fecha | Estado | Evidencia |
|---|---|---|
| 2026-10-10 | **DONE en código** | `cargo test -p cognicode-cli --test qw03_bin_tracking_guard` → 15/15 verde. Guard `scripts/ci/check-bin-tracking.sh` + test contractual pinean la propiedad: real repo (GREEN baseline), planted untracked bin (RED → guard falla), planted + tracked (GREEN, recovery), y los triangulates WU-1..WU-4 ya materializados (silent gitignore, new crate no listado, path custom, refactor si aplica). Wired en `merge-gate.pipeline.kts:cli-qw03-bin-tracking-guard`. Goal doc de "7/7 tests OK, 2026-09-28" subestimó el estado actual: hoy son 15/15 con las triangulaciones completas. Doc-debt cleanup. |