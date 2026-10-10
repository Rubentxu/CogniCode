# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda
> fuente de estado: donde este documento y el ROADMAP diverjan, manda el
> ROADMAP y este snapshot se considera obsoleto hasta su próxima
> regeneración. Declarado así tras la auditoría de 2026-09-30, que encontró
> este fichero afirmando un estado (programa no iniciado, PIVOT pendiente)
> que el ROADMAP y el árbol desmentían desde hacía días — exactamente el
> drift que QW-02 define como contradictorio.

> **Snapshot**: 2026-10-10, sobre `integrate/v1015` HEAD
> `443a540fd844189d44ea07a69ce838dd3af7c748`. Regenerado para cerrar el
> drift detectado en la auditoría técnica senior del 2026-10-10: el
> snapshot anterior declaraba SHA `15b5c68c9fde` y "80 commits ahead",
> ambos falsos.
>
> Ratchet `python3 scripts/ci/test_roadmap_version_ratchet.py` debe
> pasar al regenerar este snapshot. Si vuelve a fallar, reintroducir
> drift es reintroducir el problema; el ratchet es la red.

## HEAD y batería (a 2026-10-10)

* **Rama**: `integrate/v1015` (no es `origin/main`).
* **SHA funcional**: `443a540fd844189d44ea07a69ce838dd3af7c748` (T0-3 cerrado).
* **Versión workspace**: `0.101.9` (ver `Cargo.toml [workspace.package].version`).
* **Divergencia** vs `origin/main`: 7 commits ahead, 0 behind
  (5 commits de `assurance-evidence/v1` producer + 2 commits de cleanup
  Tier 0 del goal `v1015-consolidation-and-promote`).
* **Tags recientes**: `v0.101.0` .. `v0.101.9` (10 cortes patch consumidos).
* **Batería workspace** (medida sobre este SHA):
  * `cargo fmt --all --check` → exit 0.
  * `cargo test -p cognicode-core --lib` → **2252 passed, 2 ignored** (106 s).
  * `cargo test -p cognicode-core --test assurance_export_roundtrip` → 14/0.
  * `cargo test -p cognicode-core --lib change_signature_strategy` → 13/0.
  * `cargo test -p cognicode-core --lib rmcp_adapter` → 21/0 (era 31 con 10
    `#[ignore]` antes del cleanup T0-3).
  * `cargo build -p cognicode-core --tests` → 0 errors, 1 warning
    (graph-wasm profile warning, no introducido por T0-N).
  * `cargo doc --workspace --no-deps` → exit 0, **0 warnings** (deuda
    residual de M0.8 cerrada por M0.11 el 2026-09-27; reintroducirla
    rompe `m011_rustdoc_gate`).
  * `python3 scripts/ci/test_assurance_export_golden.py` → exit 0
    (golden SHA `45fa782147e2fc25ba6cc7e564ccaf326aae8ecaaef7ae37104133769405ee35`
    sigue pineado; producer is deterministic).
* **Working tree**: dirty (9 archivos no rastreados en `odd/tasks/` —
  audit exploration notes + goal doc `v1015-consolidation-and-promote.md`).
* **Pre-existing clippy issue (no de T0-N)**: `cargo clippy
  --workspace --all-targets -- -D warnings` falla en
  `crates/cognicode-core/tests/assurance_export_roundtrip.rs:408`
  (`clippy::expect_fun_call` sobre `.expect(&format!(...))`).
  El lint `expect_fun_call` está activo en rustc 1.96; el código es
  pre-existente a la serie T0-N. Workaround en WU futuro (no bloquea
  T0-N: este error se arrastró desde la integración de la rama
  `assurance-evidence`).

## Capacidades certificadas (Post-PRF)

* **C7** (firmada por el operador `Ruben <rubentxu@cognicode.dev>` el
  2026-09-24T22:41:33Z UTC sobre `v0.98.1`) — production-ready contractual.
  Evidencia: `docs/prf/F7-C7-EXPEDIENTE.md`.
* **C8** (Post-PRF General Availability, v0.99.0) — **FIRMADA OPERATIVA
  2026-09-26T10:14:47Z** sobre SHA `3954b8b7`. Sin tag anotado, sin
  release GitHub. Recertificación C8-R queda abierta como **CR-01**.
  Ver `docs/roadmap/certifications/C8-POST-PRF-GA.md` §11 y expediente
  `docs/prf/ADMISSION-EXPEDIENTE-F8-C8-OPERATIVO-v0.99.0.md`.

## Goal activo: v1015-consolidation-and-promote

El 2026-10-10, tras la auditoría técnica senior, el operador autoriza
el goal documentado en `odd/tasks/v1015-consolidation-and-promote.md`
(Tier 0 + Tier 1 + promote + tag `v0.101.10`).

| WU | Descripción | Estado | Commit |
|----|-------------|--------|--------|
| T0-1 | println! → eprintln! en `cognicode-mcp/src/mcp_client.rs:140` | **REJECTED** — falso positivo del audit (`mcp_client.rs` es el bin CLI `mcp-client`, no el server; el `println!` es salida intencional del CLI, no log) | n/a |
| T0-2 | eliminar `assert!(true)` + `_suppress_unused_*` | **DONE 2026-10-10** | `a77ae90` |
| T0-3 | eliminar 10 `unimplemented!()` en `rmcp_adapter.rs` | **DONE 2026-10-10** | `443a540` |
| T0-4 | regenerar `CURRENT.md` con SHA real | **DONE 2026-10-10** (este commit) | _pendiente_ |
| T0-5 | disparar merge-gate sobre el SHA actual | OPEN | _pendiente_ |
| T1-1..T1-5 | connascence cleanup (4 enums + mover presenter) | OPEN | _pendiente_ |
| TF-1..TF-4 | promote + bump + tag `v0.101.10` | OPEN | _pendiente_ |

SDDK cycle creation: **BLOQUEADO** por infra local
(`~/.local/share/sddk/data/ledger.sqlite` vacío, 0 bytes; `sddk cycle start`
falla con `Invalid parameter name: cycle_leases`). El goal vive como
artefacto versionado en `odd/tasks/` y se materializa como cycle cuando
el storage se repare (operator-gated, no bug de CogniCode).

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
| **SDDK storage repair** | `~/.local/share/sddk/data/ledger.sqlite` vacío (0 bytes) | OPEN — operator-gated, no bug de CogniCode. |

## Programa production-ready (Post-PIVOT, en ejecución)

Tras el pivot del 2026-09-26 el programa arrancó y ha consumido QW-N
parcialmente en la serie v0.101.x:

| Outcome | Estado a 2026-10-10 | Evidencia |
|---------|---------------------|-----------|
| **PR-G1** (governance reproducible) | **CLOSED** | Cutover completo a PipelineK: `b651774a` (merge authority), `d3426966` (cero workflows en `.github/`). PR-G2 desbloqueado. |
| **PR-G2** (C8-R recertificación) | **IN PROGRESS** | CR-01 OPEN. Sin bloqueos técnicos identificados; depende de firma humana. |
| **PR-PERF** (e91 + budget) | **CLOSED W1..W9** | G5 scorecard pendiente (>3 ejecuciones consecutivas sobre fixture multi-repo en sandbox). |
| **PR-ARCH** (fitness + verticales) | **IN PROGRESS** | CR-06 CLOSED con 5 constraints pineados; verticales `cognicode-control-plane` y `graph-algos` parcialmente remediados. |
| **PR-SEC** (supply-chain) | **IN PROGRESS** | 46 pines SHA (QW-05); protobuf advisory y migración OTel **CLOSED** vía CR-07 (commit migrando `opentelemetry` 0.27 → 0.29.1 + bump `prometheus` 0.13 → 0.14 que resuelve RUSTSEC-2024-0437). Contrato `cr07_protobuf_advisory_closed.rs` 3/3 verde. Pendiente: otros advisory scanning futuros. |
| **PR-DEVEX** (CI + coverage) | **CLOSED enforcement side** | Selector determinista, coverage gate, preflight contractual. Cortes v0.101.0..9 consumieron el refactor. |
| **PR-DEPTH** (deep modules ST-01..05) | **PARTIAL** | ST-N ejecutados sobre casos concretos (provenance, perf budget, skills surface, LSP ratchet); sin cierre formal. **Tier 1 del goal v1015-consolidation-and-promote ataca parte de esto** (consolidar 4 enums `SymbolKind`, mover presenter CLI). |

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
- **Doctest drift**: **CERRADO 2026-09-27** vía M0.11 (ciclo
  `m011-rustdoc-intra-doc-links`, gate `m011_rustdoc_gate` 2/2 verde).
  `cargo doc --workspace --no-deps` ahora sale con 0 warnings.
- **Producer `assurance-evidence/v1`**: 5 commits en `integrate/v1015`
  (267f206f + 7ce0d2fc + 1c02d367 + 580c696b + 3109f7f2). Pin del
  golden SHA `45fa7821...`. Consumido por `pipelinek-assurance` (R1
  v0.2.0-alpha.1).
- **Cleanup Tier 0 (goal v1015-consolidation-and-promote)**: 2 commits
  (`a77ae90` T0-2, `443a540` T0-3) eliminan dead code y 10
  `unimplemented!()` tests en `rmcp_adapter.rs`.

## Bloqueos abiertos

* **C8 firma humana contractual**: OPEN. La firma operativa no es la firma
  contractual; el operador debe firmar C8-R (CR-01) sobre un clean clone.
* **v0.101.x → main**: el último corte `v0.101.9` está publicado pero no
  promovido a `main` (la rama `integrate/v1015` está 7 commits ahead).
  PR #345 OPEN sobre `integrate/v1015 → main`, `mergeStateStatus: BLOCKED`
  por infra pipelinek (no código).
* **SDDK storage**: `~/.local/share/sddk/data/ledger.sqlite` vacío.
  Cycle creation falla; goal materializado como artefacto versionado.

## Próximo trabajo ejecutable

1. **T0-5** (goal v1015-consolidation-and-promote) — disparar merge-gate
   sobre `443a540` (push o rebase vacío) y verificar que el producer
   pasa el gate.
2. **Tier 1 del goal** (T1-1..T1-5) — consolidación de 4 enums
   `SymbolKind`, 3 `RiskLevel`, 2 `VerificationStatus`, 2 `ConfidenceTier`,
   y mover presenter de `cognicode-core::commands.rs` a `cognicode-cli`.
3. **Tier final** (TF-1..TF-4) — promote a `main` + bump `v0.101.10` +
   tag anotado.
4. **CR-01 (C8-R)** — necesita clean clone + firma humana.
5. **Auditoría #[ignore]** — repetir la búsqueda de tests con motivo
   "Flaky" o incompatibilidad de versión (lesson 70). Ya dio frutos en
   M0.5/M0.6/M0.10; patrón replicable.

---

*Mantenedor: agente principal en modo AUTO. Regenerado 2026-10-10
desde snapshot stale (`15b5c68c9fde`, 80 commits ahead falso) sobre
SHA `443a540f` (post T0-2 + T0-3).*
*Diferencia vs snapshot anterior: −2 commits stale de drift, +7 commits
reales ahead, −10 ignored tests en `rmcp_adapter`, +1 goal doc en
`odd/tasks/`, +1 pre-existing clippy issue documentado (no introducido).*
