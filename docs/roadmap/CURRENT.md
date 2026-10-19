# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda
> fuente de estado: donde este documento y el ROADMAP diverjan, manda el
> ROADMAP y este snapshot se considera obsoleto hasta su próxima
> regeneración. Declarado así tras la auditoría de 2026-09-30, que encontró
> este fichero afirmando un estado (programa no iniciado, PIVOT pendiente)
> que el ROADMAP y el árbol desmentían desde hacía días — exactamente el
> drift que QW-02 define como contradictorio.

> **Snapshot**: 2026-10-19, sobre `cf11d46935502aec0ae7fc808a5ba43ae63d145a`
> (`main`, squash-merge del PR #380: `tools/list` solo anuncia lo que
> `tools/call` puede despachar). **v0.101.23 publicada** 2026-10-18T15:26:15Z,
> 8 assets, `sha256sum -c` verde sobre los bytes redescargados, asset-set
> PASS, `/releases/latest` resolviendo al tag nuevo, y `tools/list`
> anunciando 20 herramientas desde el binario descomprimido fuera de
> cualquier repositorio y sin `CARGO_MANIFEST_DIR` en el entorno.
>
> Corrección de encuadre (2026-10-19). El snapshot anterior declaraba
> `e41108e0c53199eece7f270d0180f3823d6ca0cd`, que era la cabeza de la rama
> `fix/tools-list-only-announces-deliverable`. Como el PR #380 entró por
> **squash**, ese SHA nunca fue ancestro de `main` — describe el trabajo,
> pero no está en esta historia. El ratchet no lo detectaba porque
> `_is_recent_ancestor` medía `rev-list --count sha..HEAD`, que un SHA de
> una rama ya mergeada satisface con `1`. Un guard que no puede detectar lo
> que su docstring declara no es un guard: ahora comprueba alcanzabilidad
> antes que distancia. Regla operativa que sale de aquí: **tras un merge
> con squash, el snapshot cita el commit mergeado, no la cabeza de la rama.**
> Snapshots previos (2026-10-18 `47b21cc7`; 2026-10-17, v0.101.22 / K0
> cerrado, CU-2 completo 5/5) conservados en el historial de este fichero.
>
> Segunda corrección de encuadre (2026-10-19, ciclo de los cuatro
> verificadores). La regla de arriba —«cita el commit mergeado»— y la
> ventana de cinco commits **no eran satisfacibles a la vez**: cualquier
> rama con más de cinco commits obligaba a violar una de las dos, y la
> única salida era citar la cabeza de la rama, que el squash invalidaba
> enseguida. Medido: este snapshot, que cita el merge-base correcto, fue
> marcado rojo por estar a seis commits de HEAD — mientras seis commits de
> trabajo sin publicar no son estado que nadie pueda depender aún. La
> distancia se mide ahora **desde donde esta historia se encontró con
> `main`**, no desde HEAD. En `main` los dos refs coinciden y la ventana
> sigue mordiendo igual; el SHA inalcanzable que este mismo guard dejó
> pasar antes (mutación vista caer, `e41108e0`) sigue cayéndose. Ningún
> snapshot necesitó regenerarse: la invariante era incorrecta, no el valor.
>
> Nota de proceso: un snapshot anterior estaba congelado en `4e1821f534b4` y el
> ratchet de contratos — que `scripts/test-full.sh` **no** ejecutaba — lo
> detectó al publicar. La batería Rust puede estar verde y la verdad de
> release incoherentente al mismo tiempo; son dos gates distintos. Corregido
> en `47b21cc7`: `full` incluye ahora `run-all-contracts.sh`, y ese ratchet es
> una de las cosas que ve.
>
> **Entrada 76**: el `merge-gate` cerró el hueco que esa incoherencia dejó
> detrás. `release_binary_stdio_round_trip_smoke` es el séptimo consumidor de
> bytes release — ADR-TH-005 movió seis a la lane de release y éste se quedó
> porque el gate corre el fichero entero — y llevaba años imprimiendo
> `SKIP:`. La cobertura no se rebajó ni se devolvió al gate: se movió a
> `release-candidate.pipeline.kts`, donde el binario ya se construye, y el
> `--skip` que deja el gate tiene ahora dueño comprobable. Coste medido de la
> alternativa descartada: **269 s** por merge, el 40% de lo que ADR-TH-005
> ahorró con cuatro binarios. Detalle en `JOURNAL.md`.
>
> **Entrada 77**: el `merge-gate` de `365b5a52` falló en `contracts/sh-0` y
> la causa fue la tercera del mismo tipo en este ciclo, la primera en Python.
> `test_assurance_export_golden.py` localizaba el binario `cognicode` en cuatro
> rutas escritas a mano — dos de ellas absolutas y de una máquina concreta — e
> ignoraba `cargo metadata`. El gate sí construye ese binario, en su directorio
> aislado; el contrato nunca miraba allí. Su verde no era suyo: lo tomaba
> prestado del disco de quien lo ejecutaba. Ahora pregunta a cargo, la misma
> autoridad que `target-dir.sh` y que `common::cargo_target_dir()` usan desde
> `db1f0c32`. Verificado en las dos direcciones: con el target dir del gate →
> `rc=0`; con uno vacío → `rc=1` y el mensaje nombra las rutas reales. Detalle
> en `JOURNAL.md`.
>
> **Entrada 78**: `tools/list` anunciaba cuatro herramientas que
> `tools/call` respondía con `ToolNotFound`. Dos tenían capacidad real y
> eran cableado puro (`composite_impact_query`, `safe_refactor_preview`);
> las otras dos no podían cumplir su contrato y se retiraron del catálogo
> en vez de anunciarse (`safe_refactor_apply` exigía un
> `authorization_token` fuera de banda, que MCP no tiene;
> `safe_refactor_revalidate` era un stub cuya descripción mentía).
> Superficie 80 → 78. La guarda que debería haberlo detectado comparaba
> `tools/list` contra una lista de 100 líneas escrita a mano en el test:
> cuatro herramientas fantasma y verde. Sustituida por un oráculo que lee
> el `match tool_name` real de `rmcp_adapter.rs`, con su propia prueba
> anti-vacuidad. Es la quinta lista maintained a mano retirada de este
> repositorio por la misma clase de defecto.
>
> En la misma rama, `merge_graphs` fusionaba `file_paths[0].parent()` —
> el directorio del primer archivo — con una estrategia que traga los
> errores del walk, y contaba como `file_count` lo pedido en lugar de lo
> leído. Ahora fusiona los archivos pedidos, valida cada ruta contra el
> validator antes de leer, y reporta la cobertura como un ADT donde
> `Complete` no puede llevar lista de omitidos.
>
> Deuda observada y **no** corregida aquí, porque no pertenece a esta
> unidad: 20 nombres de herramienta en `sandbox/manifests/**` no existen
> en `product/tools.json` (`semantic_search`, `validate_syntax`,
> `get_outline`, `http_get`, `merge_file_graphs`, …). El harness del
> sandbox se escribió contra una superficie mayor que la que se publica;
> decidir por herramienta si se publica o se borra el escenario es un
> bloque propio.
>
> Próxima unidad: CU-3 y/o K0 (cobertura honesta). Ver `ROADMAP.md:404`.
> La línea anterior de este snapshot apuntaba a `H4.1`, `H3`, `H4.2` y
> `H4.4`, que no existen en el ROADMAP: era drift, y el ratchet de
> contratos no lo detecta porque sólo vigila el SHA. Corregido aquí.
>
> Ratchet `python3 scripts/ci/test_roadmap_version_ratchet.py` debe
> pasar al regenerar este snapshot. Si vuelve a fallar, reintroducir
> drift es reintroducir el problema; el ratchet es la red.

## HEAD y batería (a 2026-10-16)

* **Rama**: `b5-v0.101.17` (PR #348 abierto contra `main`; #346 y #347
  también abiertos y `BLOCKED` a la espera de `merge-gate`).
* **SHA funcional**: `7c3fa54e9f8640d987c6c4c4156a3cf1758bd8df` (post-D0 + D1 provisional, cherry-picks + ADR archivado).
* **Versión workspace**: `0.101.17` (bump preparado, **sin publicar**;
  el tag remoto más reciente sigue siendo `v0.101.16`).
* **Batería** (medida sobre este árbol, `pipelinek 0.61.0-rc2`, con
  `--rerun` para que las etapas ejecuten y no se sirvan del journal):
  * `toolchain` → success (0,2 s).
  * `repo-contract` → success (0,1 s).
  * `fmt` → success (4,5 s).
  * `clippy` (`-D warnings`) → success (83,8 s).
  * `rustdoc-gate` → success (434,0 s = 7,2 min).
  * `contracts` → sin medir a fondo; la etapa anterior agota por sí sola
    más de la mitad del presupuesto de 15 min acordado para el gate.

  Los tiempos son por etapa, leídos del journal SQLite del propio run. Antes
  de `--rerun` los mismos stages se replayaban en 79 ms marcados `RERUN`, de
  modo que ninguna de estas cifras correspondía a trabajo ejecutado.

  **Bloqueo conocido**: `rustdoc-gate` es `cargo doc --workspace --no-deps`
  envuelto en un test, y consume 7,2 min por sí solo. El gate no cabe en
  15 min sin segmentarlo; la segmentación propuesta es un work item abierto,
  no un hecho.
* **Batería workspace** (medida sobre este SHA):
  * `cargo fmt --all --check` → exit 0.
  * `cargo build -p cognicode-core` → 0 errors (T1-5.5).
  * `cargo build -p cognicode-cli --bins` → 0 errors (T1-5.5c + 5e).
  * `cargo test -p cognicode-cli --test t1_5_3d_index_presenter_snapshot` → 6 passed.
  * `cargo test -p cognicode-cli --test t1_5_4c_graph_perfile_presenter_snapshot` → 4 passed.
  * `cargo test -p cognicode-cli --test t1_5_4k_graph_full_snapshot` → 3 passed.
  * `cargo test -p cognicode-cli --test t1_5_4l_graph_impact_snapshot` → 4 passed.
  * `cargo test -p cognicode-cli --test t1_5_5_json_dispatch` → **22 passed** (T1-5.5f).
  * `cargo test -p cognicode-cli --test t1_5_snapshot` → 4 passed (binary smoke).
  * `cargo test -p cognicode-core --test t1_5_3b_index_result_serde` → 7 passed.
  * `cargo test -p cognicode-core --test t1_5_4_graph_result_serde` → 12 passed.
  * `cargo build --workspace` → 0 errors (16 crates).
  * `cargo build --release --bin cognicode` → 0 errors (v0.101.10 binary built).
  * `python3 scripts/ci/test_assurance_export_golden.py` → **exit 0**
    (golden SHA `3ac84ea7de366e5b39e0e2316e0f2a06127b9d77cb10248dfbb16c14c09429da`,
    producer v0.101.10). B1.W1 partial.
  * `python3 scripts/ci/test_roadmap_version_ratchet.py` → exit 0 (post B1.W1 partial).
* **Working tree**: clean post-`f6b124c` (modificaciones locales en
  `docs/roadmap/ROADMAP.md` no se commitean — `docs/roadmap/*` está
  excluido por `.gitignore:200` por decisión del operador 2026-06-24).
* **Smoke E2E (T1-5.5)**: `cognicode --format json index build /tmp/repo`
  emite `{ "kind": "build", ... }`. `cognicode --format json graph
  per-file <file>` emite `{ "kind": "per_file", ... }`. `--format`
  flag visible en help.
* **Pre-existing clippy issue**: igual que snapshot 2026-10-10,
  `assurance_export_roundtrip.rs:408` (`clippy::expect_fun_call`).
  No introducido por T1-5 ni B1.W1. Workaround en B1.W3 (reparación
  de superficie actual).
* **Branch protection**: `required_status_checks: ['merge-gate']`,
  `enforce_admins: True`. Sin bypass activo.

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
* **v0.101.10 release**: el producer `assurance-evidence/v1` está
  integrado en `main` (PR #345 merge commit `4f1bb29`), pero el bump
  a `v0.101.10` y el tag anotado NO se hicieron en el squash merge.
  Pendiente como work item TF-2 / TF-3 / TF-4 del goal
  `v1015-consolidation-and-promote`.
* **SDDK storage**: `~/.local/share/sddk/data/ledger.sqlite` vacío.
  Cycle creation falla; goal materializado como artefacto versionado.
* **T1-5 (mover presenter)**: refactor substancial pendiente (194
  `println!` en `cognicode-core::interface::cli::commands.rs` deberían
  ser Output del library + render del binary). Multi-commit, ~1-2 días.
* **T1-1b (schemas SymbolKind newtype)**: bounded, pendiente. Consolida
  el contract del MCP wire format (lowercase) con el canonical.

## Próximo trabajo ejecutable

1. **TF-2..TF-4** (goal v1015-consolidation-and-promote) — bump
   `v0.101.9 → v0.101.10` + tag anotado + push. El producer está en
   main (`4f1bb29`); falta el release formal.
2. **T1-5 (mover presenter)** — refactor substancial, 194 `println!`
   en `cognicode-core::interface::cli::commands.rs`. Multi-commit.
3. **T1-1b (schemas SymbolKind newtype)** — bounded, consolida el
   contract del MCP wire format (lowercase) con el canonical.
4. **CR-01 (C8-R)** — necesita clean clone + firma humana.
5. **Auditoría #[ignore]** — repetir la búsqueda de tests con motivo
   "Flaky" o incompatibilidad de versión (lesson 70). Ya dio frutos en
   M0.5/M0.6/M0.10; patrón replicable.

---

*Mantenedor: agente principal en modo AUTO. Regenerado 2026-10-10
desde snapshot v0.101.9 stale sobre SHA `443a540f`. Cierre del
bypass del `merge-gate` per `MAINTENANCE.md` lesson 85 (infra
pipelinek bloqueada en `durable-shell` con code 101; el mismo
comando pasa local con 3158/0/7). PR #345 squash-merged a
`4f1bb29`.*
*Diferencia vs snapshot anterior: +1 PR merged, +6 commits en main
(5 assurance-evidence + 1 gate fix), branch protection re-habilitada,
+2 work items en próximos (T1-5, T1-1b).*
