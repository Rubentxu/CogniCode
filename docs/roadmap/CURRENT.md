# CURRENT — Puntero operativo post-PRF (reemplaza a `docs/prf/CURRENT.md`)

> **Estado**: puntero activo. Sustituye a `docs/prf/CURRENT.md` (snapshot pre-C8,
> 50 commits stale, congelado como histórico). Esta es la fuente de verdad
> operativa para CogniCode post-PRF.

## HEAD y batería (a 2026-09-26)

* **HEAD funcional**: `aed8b7d7` (commit de cierre de e91 saga, G0 cerrado).
* **Commits sobre `origin/main`**: 16.
* **Tests workspace**: `cargo test --workspace` →
  **passed=5557 failed=0 ignored=45**.
* **Clippy**: `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
* **Working tree**: clean.

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
| **C8 firma** | Operador firma `v0.99.0` Post-PRF GA sobre `3954b8b7` (con addendum §8 documentando delta) | PENDIENTE |
| e91.W4/W5 reapertura | Si caracterización directa demuestra >10% del budget | NO TRIGGERED |

## Bloqueos abiertos

* **C8 firma humana**: única acción bloqueante para declarar `v0.99.0`
  production-ready contractual. Ver dosier + addendum §8.

## Próximo trabajo ejecutable en AUTO

No quedan work units automatizables en el backlog post-PRF inmediato. Opciones:

1. **Refinamientos sobre C8** — el operador puede pedir más evidencia
   antes de firmar (campaña adversarial Post-PRF, UAT cross-crate E2E).
2. **Nuevas work units** — el operador puede autorizar trabajo nuevo
   (no hay nada en `docs/roadmap/ROADMAP.md` que esté desbloqueado y
   sin acción pendiente).
3. **Mantenimiento** — `docs/roadmap/MAINTENANCE.md` lista issues
   abiertos sobre v0.98.x; no son bloqueantes de C8.

---

*Mantenedor: agente principal en modo AUTO. Actualizado 2026-09-26
tras cierre de e91.W4/W5. Próxima actualización: tras firma humana
de C8 o nueva work unit autorizada.*
