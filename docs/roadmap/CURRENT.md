# CURRENT — Puntero operativo post-PRF (reemplaza a `docs/prf/CURRENT.md`)

> **Estado**: puntero activo. Sustituye a `docs/prf/CURRENT.md` (snapshot pre-C8,
> 50 commits stale, congelado como histórico). Esta es la fuente de verdad
> operativa para CogniCode post-PRF.

## HEAD y batería (a 2026-09-26)

* **HEAD funcional**: ver `git rev-parse HEAD` (este doc se versiona junto al workspace, no a sí mismo).
* **Commits del agente sobre `origin/main`**: ver `git log --author=jcode-bot --oneline | wc -l`.
* **Tests workspace**: `cargo test --workspace` →
  **passed=5565 failed=0 ignored=37**.
* **Clippy**: `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
* **Working tree**: clean.
* **Versión binario**: `cognicode 0.99.1` (bump SEMVER patch por M0.5).

## Capacidades certificadas (Post-PRF)

* **C7** (firmada por el operador `Ruben <rubentxu@cognicode.dev>` el
  2026-09-24T22:41:33Z UTC sobre `v0.98.1`) — production-ready contractual.
  Evidencia: `docs/prf/F7-C7-EXPEDIENTE.md`.
* **C8** (Post-PRF General Availability, v0.99.0) — **PASS localmente sobre
  SHA `3954b8b7`** (dosier original). **PENDIENTE firma humana del
  operador**. Ver `docs/roadmap/certifications/C8-POST-PRF-GA.md` y su
  **addendum §8** que documenta los 21 commits posteriores sin
  regresiones (5557/0/45 vs 5542/0/45 del C8 base).

## Estado del programa e91 (saga MCP/graph)

Todos los work units e91 cerrados:

| ID | Descripción | Estado | Commits clave |
|----|-------------|--------|---------------|
| e91.W1 | `iterations_used` + `converged` reales en `graph_communities` | CLOSED | `6f40a08b`, `42a1ddcf` |
| e91.W2 | Caracterización PageRank warm (no bottleneck) | CLOSED | `8b4bbe85`, `6b2738f3` |
| e91.W3 | Cache de PageRank — no viable | CLOSED (non-viability) | entry 17 |
| e91.W4 | Paralelizar god_nodes — no viable | CLOSED (derived W2) | entry 19 |
| e91.W5 | Memoize surprising_connections — no viable | CLOSED (derived W2) | entry 19 |
| e91.W6 | Metadata envelope en 7 sibling handlers | CLOSED | `c1618e84`, `df8002f5` |

## Decisiones pendientes (operator-gated)

| ID | Pendiente | Estado |
|----|-----------|--------|
| **C8 firma** | Operador firma `v0.99.0` Post-PRF GA sobre `3954b8b7` (con addendum §8-§10 documentando delta) | **FIRMADO OPERATIVO 2026-09-26T10:14:47Z** sobre `3954b8b7`. Ver `docs/prf/ADMISSION-EXPEDIENTE-F8-C8-OPERATIVO-v0.99.0.md` y `docs/roadmap/certifications/C8-POST-PRF-GA.md` §11. Recertificación C8-R → CR-01. |
| e91.W4/W5 reapertura | Si caracterización directa demuestra >10% del budget | NO TRIGGERED |
| **M0.6 fix tree-sitter** | Operador elige entre bumpear `tree-sitter = "0.25"` (afecta 18 parsers, riesgo de regresiones API), downgrade a fork comunitario, o marcar PHP/Swift como `Language::Unsupported`. Bug bloqueante para usuarios PHP/Swift (4 tests `#[ignore]` pinean `LanguageError { version: 15 }`). | **BLOCKED 2026-09-26** |
| **PIVOT programa** | Operador autoriza arranque del nuevo programa production-ready (QW-01..07 + CR-01..09 + ST-01..05, 21 acciones, 34-52 días-persona). Paquete ya versionado en `docs/roadmap/production-ready/`. | **PENDIENTE** |

## Programa production-ready (Post-PIVOT, no iniciado)

Tras el pivot del 2026-09-26, el siguiente programa de estabilización
queda versionado y listo para arrancar (no se ejecuta hasta decisión
del operador). Detalle completo en
`docs/roadmap/production-ready/EXECUTIVE-SUMMARY.md` y `EXECUTION-PLAN.md`.

* **Outcomes**: PR-G1 (governance), PR-G2 (C8-R), PR-PERF (e91 G5),
  PR-ARCH (boundary), PR-SEC (protobuf+Actions), PR-DEVEX (CI+coverage),
  PR-DEPTH (deep modules).
* **Fase 1 — Quick Wins**: QW-01..07 (3-5 días-persona).
* **Fase 2 — Critical**: CR-01..09 (13-20 días-persona).
* **Fase 3 — Strategic**: ST-01..05 (18-27 días-persona).
* **Bloqueos heredados al programa** (no resueltos por el pivot):
  C8 firma humana, M0.6 fix tree-sitter.
* **Decisiones tomadas en pivot**:
  * NO firmar C8 unilateralmente.
  * NO bumpear tree-sitter unilateralmente.
  * SÍ versionar el paquete (stewardship de bajo riesgo).
  * NO fusionar `ROADMAP-ADDENDUM.md` con este `CURRENT.md` aquí;
    queda como QW-01 del nuevo programa.

## Bloqueos abiertos

* **C8 firma humana**: **FIRMADO OPERATIVO 2026-09-26T10:14:47Z** sobre SHA `3954b8b7`. Ver dosier `docs/roadmap/certifications/C8-POST-PRF-GA.md` §11 y expediente `docs/prf/ADMISSION-EXPEDIENTE-F8-C8-OPERATIVO-v0.99.0.md`. Recertificación C8-R queda abierta como **CR-01** dentro del programa production-ready.
* **M0.6 fix**: PHP y Swift rotos en producción por incompatibilidad tree-sitter parser (version 15) vs runtime (version 14). 4 tests `#[ignore]` pinean el bug; el fix requiere bumpear `tree-sitter` a 0.25 o equivalente (alcance mayor, no automatizable unilateralmente). Ver `docs/roadmap/MAINTENANCE.md` M0.6 con 3 opciones de fix. **No bloqueante para CR-01.**

## Próximo trabajo ejecutable en AUTO

El backlog automatizable está probablemente vacío tras el cierre de M0.5.
M0.6 es BLOCKED esperando decisión del operador (no automatizable).
Opciones:

1. **Refinamientos sobre C8** — el operador puede pedir más evidencia
   antes de firmar (campaña adversarial Post-PRF, UAT cross-crate E2E).
2. **Nuevas work units** — el operador puede autorizar trabajo nuevo
   (no hay nada en `docs/roadmap/ROADMAP.md` que esté desbloqueado y
   sin acción pendiente).
3. **Mantenimiento** — `docs/roadmap/MAINTENANCE.md` lista M0.* cerrados
   y posibles nuevas auditorías (e.g. otros `#[ignore]` con flake pendiente).
4. **Auditoría dirigida** — repetir la búsqueda de tests `#[ignore]` con
   motivo "Flaky" o de incompatibilidad de versión para detectar otros
   bugs latentes (lesson 70). Ya dio frutos en M0.5 (8 tests
   re-habilitados) y M0.6 (PHP/Swift pineados).

---

*Mantenedor: agente principal en modo AUTO. Actualizado 2026-09-26
tras firma OPERATIVA de C8 (categoría OPERATIVO, no contractual;
recertificación C8-R abierta como CR-01).
Próxima actualización: tras arranque de CR-01, decisión sobre M0.6,
o firma C8-R al nivel contractual.*
