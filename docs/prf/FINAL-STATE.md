# FINAL-STATE — Cierre del programa PRF (2026-09-25)

> **Este archivo es el ÚLTIMO puntero de PRF. NO se añaden más self-rolls.**
> Cierra el ciclo de vida de PRF como programa. Su sucesor es `docs/roadmap/`.

## 1. Estado final

| Concepto | Estado |
|---|---|
| **Programa PRF** | **CERRADO contractualmente**. C7 firmado por el operador el 2026-09-24T22:41:33Z UTC. |
| **Release firmada** | `v0.98.1` (tag anotado `a21fccda` → commit `e4ab6c8e`). Sigue siendo Latest en GitHub Releases. |
| **HEAD actual del repo** | `f05c503b4fb1dde6c9aa5fc93ab1cf133e5aaeb4` (más commits `51a650b8` §154.H, `07f989c9` G0.1 si se ha mergeado). |
| **Documentación PRF** | `docs/prf/` — evidencia histórica. READ-ONLY. Sin más self-rolls. |
| **Sucesor** | `docs/roadmap/ROADMAP.md` (Post-PRF, único roadmap activo) + `docs/roadmap/MAINTENANCE.md` (backlog de parches v0.98.x). |

## 2. Lo que PRF dejó hecho (resumen ejecutivo)

- 2188 tests lib + 8 PRF-SEC-07 lib + 4 PRF-MCP-05 + 15 PRF-H06 E2E = 2215 verdes.
- 20 tools MCP authority=`read` cuando se invoca `--read-only`.
- Cifrado de secretos por scope, aislamiento de procesos por authority, sin telemetría.
- Adversarial E2E contra binario real (15 vectores + capabilities).
- Workflow PR-CI shipped con `merge-gate` y branch protection activa en `main` (strict:true, contexts:[merge-gate]).
- 5 excepciones contractuales E-C7-001..E-C7-005 documentadas en `F7-C7-EXPEDIENTE.md`.
- 12 assets publicados en la release v0.98.1 con SHA256 reproducible.

## 3. Lecciones aprendidas (para no repetir)

1. **No cerrar "X está hecho" cuando solo una parte lo está.** §154.H fue una corrección honesta de este patrón. La regla: si una unidad tiene más de una parte (workflow + enforcement, código + tests, spec + impl), cada parte es una fila separada en la matriz.
2. **El gate no es "el workflow existe" sino "el gate se exige".** Branch protection sin required checks = ilusión de seguridad.
3. **No acumular fmt-drift.** Si lo haces, un día un PR bueno falla por fmt y parece que el PR está mal.
4. **Dos nombres iguales en dominios distintos no se fusionan.** `EvidenceStore` en `domain::evidence_kernel::ports` ≠ `domain::ports::evidence_store`. Un ADR previo a la fusión.
5. **Roadmap cerrado ≠ roadmap muerto.** C7 firma el estado del binario en una fecha, no la evolución del proyecto.

## 4. Lo que NO es responsabilidad de este documento

- Decisiones de roadmap Post-PRF → `docs/roadmap/ROADMAP.md`.
- Mantenimiento v0.98.x → `docs/roadmap/MAINTENANCE.md`.
- Reactivación de PRF → no procede. C7 está firmado.
- Modificación de docs/prf/STATE.md, JOURNAL.md, RECONCILIATION-MATRIX.md, etc. → congelados. Solo lectura para nuevas auditorías.

## 5. Cómo se firma este FINAL-STATE

Por la naturaleza de este documento (cierre de programa), **no requiere firma nueva**: ya está cubierto por C7 sobre v0.98.1 y por la firma implícita de §154.H.G0.2 (corte a nuevo roadmap).

## 6. Referencias cruzadas

- `docs/prf/F7-C7-EXPEDIENTE.md` — firma contractual
- `docs/prf/STATE.md` — estado al cierre
- `docs/prf/RECONCILIATION-MATRIX.md` — requisitos vs evidencia
- `docs/prf/CURRENT.md` — puntero (con la corrección honesta §154.H)
- `docs/prf/POST-PRF-EVOLUTION.md` — notas de transición (si existe)
- `docs/roadmap/ROADMAP.md` — sucesor activo
- `docs/roadmap/MAINTENANCE.md` — backlog de parches
