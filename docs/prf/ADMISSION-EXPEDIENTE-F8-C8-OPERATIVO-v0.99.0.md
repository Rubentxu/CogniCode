# ADMISSION-EXPEDIENTE F8 — C8 Post-PRF General Availability (v0.99.0) — **OPERATIVO**

> **Categoría**: **OPERATIVO** (no contractual).
> **SHA firmado**: `3954b8b75b9e9fade8ead2dfeffde8aacfafe83a`.
> **Versión**: `v0.99.0` (sin tag anotado; tag diferido hasta C8-R).
> **Fecha de firma operativa**: 2026-09-26T10:14:47Z.
> **Firmante**: `Ruben <rubentxu@cognicode.dev>`.
> **Diferencia con C7**: C7 (firmada 2026-09-24T22:41:33Z) es
> CONTRACTUAL sobre `v0.98.1` y cierra el programa PRF. C8 (esta)
> es OPERATIVA sobre `v0.99.0` y cierra el ciclo Post-PRF sin
> compromiso contractual de release.

## 1. Contexto y autoridad

Este expediente recoge la **firma operativa** de la certificación C8
(Post-PRF General Availability) sobre la versión `v0.99.0` del
repositorio CogniCode. Se emite en el contexto del programa
**production-ready stabilization** que arrancó el 2026-09-26 (commit
`3f2de0b9`).

El dosier completo de C8 vive en
`docs/roadmap/certifications/C8-POST-PRF-GA.md` (595 líneas, 11
secciones). Este expediente es el **recibo de firma**, análogo a
`docs/prf/F7-C7-EXPEDIENTE.md` pero con categoría OPERATIVO.

## 2. Decisión firmada

El operador firma C8 al **nivel operativo** (opción 2 de las tres que
ofrecía el dosier §6), eligiendo:

* **C8 = CIERRE OPERATIVO LOCAL** sin release formal.
* **v0.99.0** queda como **SHA checkpoint** (`3954b8b7`) para futuras
  auditorías.
* **Tag anotado `v0.99.0`**: NO se emite.
* **Release GitHub**: NO se publica.
* **Recertificación C8-R**: queda abierta como **CR-01** dentro del
  programa production-ready (outcome PR-G2).

## 3. Cadena de evidencia

| SHA | Descripción | Estado C8 |
|-----|-------------|-----------|
| `3954b8b7` | **C8 base — SHA firmado operativo** | PASS (sin cambios) |
| `528d9966` | §8 addendum — 21 commits | PASS (delta documentado) |
| `ec98c532` | §9 addendum — 4 commits docs-only | PASS (delta documentado) |
| `d2af7ae6` | §10 addendum — 19 commits (1 fix + 18 docs/chore) | PASS (delta documentado) |

Los criterios §2 del dosier C8 original se mantienen sobre HEAD actual
(`d2af7ae6`): build, tests (5565/0/37), clippy (exit 0), binarios
legendados (`cognicode 0.99.1`), control-plane arranca con 3
canonical constraints, ArchitectureRegistry wireado, E0 compat
preserved, E1 durability preserved, F0.1 verde, M0.4 cerrado.

El único commit de código del delta acumulado es `5fad9b40` (fix
M0.5 flake rustc), que **mejora fiabilidad sin cambiar contratos
públicos**.

## 4. Hand-off al programa production-ready (PR-G2)

Con esta firma operativa, **PR-G2 puede arrancar CR-01**:

* **CR-01** ejecutará `scripts/ci/preflight-clean-clone.sh` (QW-04,
  creado en commit `66fd4103`) sobre HEAD actual o un SHA candidato a
  elegir.
* Si preflight PASS, emitirá C8-R siguiendo el runbook
  `docs/roadmap/production-ready/runbooks/C8-RECERTIFICATION-RUNBOOK.md`.
* **PR-G2 = CLOSED** cuando el operador firme C8-R al nivel
  contractual (analogía con C7).

## 5. Estado de bloqueos heredados

Tras esta firma operativa:

| Bloqueo | Estado anterior | Estado nuevo |
|---------|-----------------|---------------|
| **C8 firma humana** | PENDIENTE | **FIRMADO OPERATIVO** (`3954b8b7`) |
| M0.6 PHP/Swift tree-sitter | BLOCKED | **BLOCKED con herencia** (sigue pendiente, no es bloqueante para CR-01) |

## 6. Cómo NO se reabre esta firma operativa

* **NO** se reabre C7 (contractual, sobre `v0.98.1`).
* **NO** se reabre C8 base (`3954b8b7`) para "incluir" commits
  nuevos: cada delta vive en su propio addendum.
* **NO** se reabre C8 operativa para emitir tag o release sin C8-R
  previa.
* **NO** se reabre CR-01 si pasa: queda como evidencia de PR-G2 CLOSED.

## 7. Bloqueos heredados que CR-01 debe respetar

* **M0.6** (PHP/Swift tree-sitter bump): NO es bloqueante para
  CR-01. Puede resolverse como hotfix independiente o dentro de
  CR-07 (OTel 0.27→0.28) si el bump tree-sitter surge como
  subproducto.
* **Bloqueos adicionales detectados durante Fase 1**:
  * `dtolnay/rust-toolchain@stable` (canal mutable, no pineado).
  * `rootful/setup-podman@v4` (repo borrado en GitHub).
  Ambos documentados con comentarios inline en los workflows. NO son
  bloqueantes para CR-01.

## 8. Verificación de la firma operativa

```
$ git cat-file -p 3954b8b7 | head -3
tree 4b8d2bf3...
parent 3a42d95d...
author Ruben <rubentxu@cognicode.dev> ...

$ git tag -l 'v0.99.0*'
(empty — tag diferido)

$ git log 3954b8b7 -1 --format='%H %s'
3954b8b75b9e9fade8ead2dfeffde8aacfafe83a docs(certification): C8 Post-PRF GA — cierre técnico
```

## 9. Comando de recuperación

```bash
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode

# Estado de C8
git cat-file -p 3954b8b7 | head -5
git diff 3954b8b7..HEAD --stat | tail -3

# Validar que la firma operativa no se ha invalidado
cargo test --workspace 2>&1 | grep "test result" | \
  awk '{p+=$4; f+=$6; i+=$8} END {printf "passed=%d failed=%d ignored=%d\n", p, f, i}'
# Esperado: passed=5565 failed=0 ignored=37

cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
# Esperado: exit 0

# C8-R pendiente (CR-01)
cat openspec/changes/2026-09-26-c8-recertification/tasks.md
```

---

*Expediente emitido por el agente principal en modo AUTO el
2026-09-26T10:14:47Z.*

*SHA firmado operativo: `3954b8b75b9e9fade8ead2dfeffde8aacfafe83a`.*

*Categoría: OPERATIVO (no contractual).*

*Próxima acción: CR-01 (recertificación C8-R desde clean clone).*
