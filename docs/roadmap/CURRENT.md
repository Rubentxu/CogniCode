# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda
> fuente de estado: donde este documento y el ROADMAP diverjan, manda el
> ROADMAP y este snapshot se considera obsoleto hasta su próxima
> regeneración. Declarado así tras la auditoría de 2026-09-30, que encontró
> este fichero afirmando un estado (programa no iniciado, PIVOT pendiente)
> que el ROADMAP y el árbol desmentían desde hacía días — exactamente el
> drift que QW-02 define como contradictorio.

> **Snapshot**: 2026-10-09, sobre `integrate/v1015` HEAD
> `61656ea877ab74bae3cf28735ea63ba8ff3762be`. Regenerar antes de citar.
>
> El ratchet `python3 scripts/ci/test_roadmap_version_ratchet.py` ya disparó
> una vez (2026-10-09, justo despues de anadirlo): los 6 commits de la sesion
> actualizaron HEAD a `a728032031e9` sin regenerar este snapshot, y el
> contrato devolvio `FAIL: CURRENT.md declara SHA 15b5c68c9fde, HEAD es
> a728032031e9...`. El ratchet cumple su funcion.

## HEAD y batería (a 2026-10-09)

* **Rama**: `integrate/v1015` (no es `origin/main`).
* **SHA funcional**: `15b5c68c9fde05c021b542afc762e6fcd1463735` (recibo N+102).
* **Versión workspace**: `0.101.9` (ver `Cargo.toml [workspace.package].version`).
* **Divergencia** vs `origin/main`: 80 commits ahead, 0 behind.
* **Tags recientes**: `v0.101.0` .. `v0.101.9` (10 cortes patch consumidos).
* **Batería workspace** (medida sobre este SHA):
  * `cargo fmt --all --check` → exit 0.
  * `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
  * `cargo test -p cognicode-core --lib` → **2252 passed, 0 failed, 12 ignored** (≈46 s).
  * `cargo test --workspace` → ≈5650 passed, 0 failed.
  * `cargo doc --workspace --no-deps` → exit 0 con **3 warnings** `ambiguous link` en `cognicode-core` (deuda residual heredada de M0.8).
* **Working tree**: dirty (cambios locales en `AGENTS.md`, `odd/`, `scripts/test-fast.sh`, `scripts/test-full.sh`).

## Capacidades certificadas (Post-PRF)

* **C7** (firmada por el operador `Ruben <rubentxu@cognicode.dev>` el
  2026-09-24T22:41:33Z UTC sobre `v0.98.1`) — production-ready contractual.
  Evidencia: `docs/prf/F7-C7-EXPEDIENTE.md`.
* **C8** (Post-PRF General Availability, v0.99.0) — **FIRMADA OPERATIVA
  2026-09-26T10:14:47Z** sobre SHA `3954b8b7`. Sin tag anotado, sin
  release GitHub. Recertificación C8-R queda abierta como **CR-01**.
  Ver `docs/roadmap/certifications/C8-POST-PRF-GA.md` §11 y expediente
  `docs/prf/ADMISSION-EXPEDIENTE-F8-C8-OPERATIVO-v0.99.0.md`.

## Estado del programa e91 (saga MCP/graph)

Todos los work units e91 cerrados a 2026-09-26:

| ID | Descripción | Estado | Commits clave |
|----|-------------|--------|---------------|
| e91.W1 | `iterations_used` + `converged` reales en `graph_communities` | CLOSED | `6f40a08b`, `42a1ddcf` |
| e91.W2 | Caracterización PageRank warm (no bottleneck) | CLOSED | `8b4bbe85`, `6b2738f3` |
| e91.W3 | Cache de PageRank — no viable | CLOSED (non-viability) | entry 17 |
| e91.W4 | Paralelizar god_nodes — no viable | CLOSED (derived W2) | entry 19 |
| e91.W5 | Memoize surprising_connections — no viable | CLOSED (derived W2) | entry 19 |
| e91.W6 | Metadata envelope en 7 sibling handlers | CLOSED | `c1618e84`, `df8002f5` |
| e91.W7 | Regression budget gate contractual (Tier-2 ≤30 s) | CLOSED | entry post-W6 |
| e91.W8 | Per-stage profile breakdown (5 s cap) | CLOSED | entry post-W7 |
| e91.W9 | `feedback_arc_set` O(N²) → O(N) | CLOSED | entry post-W8 |

## Decisiones pendientes (operator-gated)

| ID | Pendiente | Estado |
|----|-----------|--------|
| **C8 firma contractual** | Recertificación C8-R desde clean clone (CR-01) | OPEN — bloquea el cierre del programa production-ready al nivel contractual (no operativo). |
| **v0.101.x release gating** | Operador decide bump v0.102.0 (minor) o continuar patch | NO TRIGGERED — no hay demanda de breaking change. |
| **Mantenimiento #[ignore]** | Auditoría dirigida periódica (lesson 70/79/80) | Activo en background; no bloquea release. |

## Programa production-ready (Post-PIVOT, en ejecución)

Tras el pivot del 2026-09-26 el programa arrancó y ha consumido QW-N
parcialmente en la serie v0.101.x:

| Outcome | Estado a 2026-10-09 | Evidencia |
|---------|---------------------|-----------|
| **PR-G1** (governance reproducible) | **CLOSED** | Cutover completo a PipelineK: `b651774a` (merge authority), `d3426966` (cero workflows en `.github/`). PR-G2 desbloqueado. |
| **PR-G2** (C8-R recertificación) | **IN PROGRESS** | CR-01 OPEN. Sin bloqueos técnicos identificados; depende de firma humana. |
| **PR-PERF** (e91 + budget) | **CLOSED W1..W9** | G5 scorecard pendiente (>3 ejecuciones consecutivas sobre fixture multi-repo en sandbox). |
| **PR-ARCH** (fitness + verticales) | **IN PROGRESS** | CR-06 CLOSED con 5 constraints pineados; verticales `control-plane` y `graph-algos` parcialmente remediados. |
| **PR-SEC** (supply-chain) | **IN PROGRESS** | 46 pines SHA (QW-05); protobuf advisory y migración OTel pendientes. `cargo-deny licenses` con allow-list activo. |
| **PR-DEVEX** (CI + coverage) | **CLOSED enforcement side** | Selector determinista, coverage gate, preflight contractual. Cortes v0.101.0..9 consumieron el refactor. |
| **PR-DEPTH** (deep modules ST-01..05) | **PARTIAL** | ST-N ejecutados sobre casos concretos (provenance, perf budget, skills surface, LSP ratchet); sin cierre formal. |

**Cambios estructurales observables desde v0.99.2 → v0.101.9**:

- **Orquestación**: `.github/workflows/` eliminado por completo (cutover
  a PipelineK `*.pipeline.kts`). El invariante "cero workflows" lo
  pinea `test_no_actions_workflows.py`.
- **Release**: pipeline `release-candidate` con provenance activado por
  clave; 13 contratos sobre generación, 7 mutaciones vistas caer.
- **Skills**: 5 skills publicadas en `skills/`; `test_skill_surface_claims.py`
  pinea que las afirmaciones sobre la superficie MCP (e.g. "73-tool server")
  no pueden mentir sin que el contrato lo detecte.
- **LSP**: 4 tests de integración que daban verde sin LSP ahora pinen
  la propiedad real; nuevo ratchet sobre `process::exit`.
- **Receiving pattern**: recibos `docs(roadmap):` N+85 .. N+102 (22 entries)
  documentan cada delta con su gate medido.

## Bloqueos abiertos

* **C8 firma humana contractual**: OPEN. La firma operativa no es la firma
  contractual; el operador debe firmar C8-R (CR-01) sobre un clean clone.
* **v0.101.x**: el último corte `v0.101.9` está publicado pero no promovido
  a `main` (la rama `integrate/v1015` está 80 commits ahead). Decisión del
  operador sobre cuándo promover.
* **Doctest drift residual**: 3 warnings `ambiguous link` en
  `cognicode-core` (deuda arrastrada de M0.8). Bajo riesgo (solo docs).

## Próximo trabajo ejecutable en AUTO

1. **Promover `integrate/v1015` → `origin/main`** — decisión operador.
2. **Cerrar CR-01 (C8-R)** — necesita clean clone + firma humana.
3. **Continuar ST-N (PR-DEPTH)** — work items identificables: deep-module
   refactors sobre parsers multimodales, simplificación de EvidenceStore.
4. **Auditoría #[ignore]** — repetir la búsqueda de tests con motivo
   "Flaky" o incompatibilidad de versión (lesson 70). Ya dio frutos en
   M0.5/M0.6/M0.10; patrón replicable.
5. **Estrangular doctest drift** — 3 warnings residuales (M0.8).

---

*Mantenedor: agente principal en modo AUTO. Regenerado 2026-10-09
desde snapshot 2026-09-30 (obsoleto) sobre SHA `15b5c68c`.*
*Diferencia: +80 commits, +10 tags patch, +1 rama de orquestación
nueva (PipelineK), +1 programa production-ready ejecutado, +0 FIXME,
+17 TODO manejables.*