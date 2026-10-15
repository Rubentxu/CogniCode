# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda
> fuente de estado: donde este documento y el ROADMAP diverjan, manda el
> ROADMAP y este snapshot se considera obsoleto hasta su próxima
> regeneración. Declarado así tras la auditoría de 2026-09-30, que encontró
> este fichero afirmando un estado (programa no iniciado, PIVOT pendiente)
> que el ROADMAP y el árbol desmentían desde hacía días — exactamente el
> drift que QW-02 define como contradictorio.

> **Snapshot**: 2026-10-15, sobre `release/v0.101.17`
> `3d8906c267550a3ba4c5a3fca1e765ddc1082792` (base: PR #348 mergeado en `main`
> como `88f6d02a`; este trabajo corrige el gate sobre ella). Los seis
> gates reparados que se listaban sobre
> `0fcfb724b09fc99769c30bdc606bf1b0fd0991b6` siguen en el árbol, **pero el
> veredicto que se les atribuyó era falso**: la etapa `tests` del merge gate
> ejecutaba 4 de sus 39 `cargo test` y devolvía `success`. MEDIDO
> 2026-10-15 — ver "Batería" más abajo, donde esas cifras quedan
> marcadas como no válidas. `v0.101.17` está **suspendida**: la lane
> `release-candidate` falló en `clean-clone` por esta causa.
> Regenerado tras **BLOQUE 5**: el workspace está en `0.101.17` per
> `Cargo.toml`, y el bump 0.101.16 → 0.101.17 todavía **no está
> publicado** — el tag remoto sigue en `v0.101.16`, que es la release
> que hoy publica 3 de los 8 assets. La release viva es `v0.101.16`.
>
> Próxima release objetivo: `v0.101.17` (re-publicar el set completo de
> 8 assets) per
> [`docs/roadmap/2026-Q4-CLI-MCP-ROADMAP.md`](2026-Q4-CLI-MCP-ROADMAP.md).
>
> Ratchet `python3 scripts/ci/test_roadmap_version_ratchet.py` debe
> pasar al regenerar este snapshot. Si vuelve a fallar, reintroducir
> drift es reintroducir el problema; el ratchet es la red.

## HEAD y batería (a 2026-10-15)

* **Rama**: `release/v0.101.17` creada desde `origin/main` = `88f6d02a`
  (PR #348 mergeado el 2026-10-15; #346 y #347 siguen abiertos y `BLOCKED`,
  contenido dentro de #348).
* **SHA**: `3d8906c267550a3ba4c5a3fca1e765ddc1082792`. **No hay SHA funcional**:
  no existe ningún commit de esta rama con un veredicto de gate atribuible a él.
* **Versión workspace**: `0.101.17` (bump en el árbol, **sin publicar**;
  el tag remoto más reciente sigue siendo `v0.101.16`).

* **Los tres gates obligatorios, sin adornos.** Ninguno está `PASS`.

  | Gate | Estado | Motivo |
  |---|---|---|
  | `merge-gate` | **FAIL** | dos corridas; ambas con el veredicto contaminado entre checkouts |
  | `integration` | **NOT_RUN** | arrancada y abortada; fuera del alcance mínimo que fijó el operador |
  | `certification` | **NOT_RUN** | no ejecutada |

  El operador decidió publicar `v0.101.17` sin el merge gate en verde, con
  motivo declarado: el bloqueo es de entorno, no de código. El agente registra
  esa decisión y **no la firma**.

* **RETIRADO — «Gate en verde» sobre `fb0e9494`.** Este snapshot afirmaba, hasta
  el 2026-10-15, que el gate había corrido en 606 s con 71 etapas declaradas, 71
  ejecutadas y 0 fallos. **Se retira.** No porque las cifras fueran falsas —el
  recuento se cruzó contra los nombres de etapa que el runtime imprimió— sino
  porque **no son atribuibles a este árbol**: seis checkouts de este repo
  compilan al mismo target dir (`/var/home/rubentxu/cargo-targets/`), cargo
  decide frescura por mtime, y un mtime no dice de qué checkout salió un
  binario. Las corridas 6 y 7 fallaron exactamente así, en las dos
  direcciones: un test paniqueó en la línea 900 de un fichero de 861 nombrando
  un test que no existe en este árbol, y otro comparó su `source_commit`
  (`d5082848…`) contra `git rev-parse HEAD` (`d73e7b08…`), que es el HEAD de
  `worktree-test-harness`.

  Las corridas 7 y 8 se marcan **no atribuibles**, no «verdes». Un gate que
  puede ejecutar el binario de cualquiera de los otros cinco checkouts no
  decide nada: ni rojo ni verde.

  La etapa `build-isolation` (`3d8906c2`) se antepone ahora a todo lo demás y
  **se niega** cuando un checkout hermano de este repo está compilando,
  nombrándolo. Falla, no avisa: el aviso caería en la misma pantalla que el
  resultado. Y es una detección puntual, no una garantía — entre el preflight y
  la última etapa otro checkout puede empezar a compilar. El aislamiento de
  verdad es `CARGO_TARGET_DIR` por corrida.

* **Flake abierto, sin reproducir**: `cr07_metrics_exposition_contract`
  devolvió `shell exited with code 101` en la quinta corrida del gate, siendo la
  etapa 32 de 67. En solitario no se ha reproducido: 3/3 y después 20/20. Pasó en
  las corridas 6, 7 y 8, **ninguna atribuible**. **No está reproducido, así que
  no está arreglado**, y un verde no atribuible no es ni la mitad de la prueba.

* **Estado del gate**: diez corridas el 2026-10-15. Las seis primeras, cada una
  con una avería distinta y real, y cada una encontrada por el gate y no por una
  revisión — cuatro de ellas de este mismo trabajo. La 7 y la 8 salieron
  verdes. La 9 y la 10 salieron rojas. **Ninguna de las cuatro es atribuible a
  este árbol.**

  | Corrida | Resultado | Causa |
  |---|---|---|
  | 1 | FAIL | `prf_h06_adversarial_e2e` nombrado bajo el crate que ya no lo contenía |
  | 2 | FAIL | `build-binary` estaba *después* del primer consumidor |
  | 3 | FAIL | `fmt` — dos líneas mías sin `cargo fmt`, en 13 s |
  | 4 | FAIL | `contracts` — este mismo ratchet, por 4 commits de distancia |
  | 5 | FAIL | `cr07-metrics-exposition`, flake sin reproducir (ver arriba) |
  | 6 | FAIL | `mcp-prf-sec-02-read-only-uat`, UAT roja desde `e11016ff` |
  | 7 | "verde", 606 s | **no atribuible** |
  | 8 | "verde", 677 s | **no atribuible** |
  | 9 | FAIL | test paniqueando en la línea 900 de un fichero de 861 — binario de otro checkout |
  | 10 | FAIL | `source_commit (d5082848…) must match git rev-parse HEAD (d73e7b08…)` — `d73e7b08` es el HEAD de `worktree-test-harness` |

  La primera medición del gate —687 s en 8 etapas— era de un gate que ejecutaba
  4 de sus 39 tests. Las cifras de abajo son las que se midieron después de
  arreglarlo, y **también están bajo la misma reserva**: son tiempos reales de
  una máquina real, pero de una máquina donde seis checkouts compiten por el
  mismo directorio.

* **La sexta corrección era una UAT roja de meses, no una avería del gate.**
  `prf_sec_02_read_only_uat` afirmaba que el modo por defecto marca
  «exactamente las tres herramientas mutantes» nombrándolas una a una: esa
  lista es una segunda fuente de verdad del campo `authority` de
  `product/tools.json`. Cuando B5.W3 (`e11016ff`) declaró `safe_refactor_apply`
  como mutante —con razón, escribe el refactor— la lista no se tocó y el test
  quedó rojo. Nadie lo vio porque era una de las **35 pruebas que el gate
  colapsado no ejecutaba**: salió al restaurar las etapas. El oráculo pasa a
  ser el catálogo, leído con el mismo cargador que el runtime
  (`load_tool_catalog_active`), y el test pasa a comprobar en negro, sobre un
  proceso MCP real y con la paginación de `tools/list` de por medio, que el flag
  que llega por el cable coincide con lo que el catálogo declara.

* **Batería — MEDIDAS, y ya no son las de antes.** Las cifras de este
  snapshot se retiraron el 2026-10-15 porque se midieron sobre el gate
  colapsado de 73 a 8 etapas, en el que la etapa `tests` ejecutaba **4 de sus
  39 `cargo test`** y devolvía `success`. Causa: un `exit $rc` heredado de un
  sub-bloque termina el proceso, no el sub-bloque; pegado en medio de una etapa
  de 39 órdenes se come las 35 siguientes. Y PipelineK corre el cuerpo con
  `bash script.sh` **sin opciones de shell**, así que sin `-e` el veredicto de
  una etapa de muchas órdenes es el de la última.

  Coste real medido el 2026-10-15, orden por orden:
  * 37 invocaciones con selector de target → **692 s**, de las cuales
    `cargo test -p cognicode-core --lib` son **477 s** en solitario.
  * Los cuatro builds de perfil release, en un target dir **vacío**:
    `cognicode-mcp` 358 s, `cognicode-control-plane` 123 s,
    `cognicode-cli` 174 s, `cognicode` 38 s → **693 s**. En un runner limpio
    es ese el precio de que las cuatro suites UAT de release puedan correr.

  * **Coste por etapa: 124 ms.** Dos pipelines de sonda con trabajo idéntico
    (30 órdenes `true`), mejor de 5: 1 etapa 5827 ms, 30 etapas 9432 ms.
    Restaurar las 73 etapas cuesta **8,9 s**. El colapso de etapas no era el
    problema que parecía, y su precio —un gate que miente— sí lo era.

  * **El presupuesto de 15 min que se aprobó con este gate ya no es
    alcanzable**, y la cifra que lo sostenía (687 s) era la de un gate que no
    ejecutaba su batería. El orden de magnitud real, frío, es de decenas de
    minutos. Volver a decidir el presupuesto con el número viejo sería repetir
    el error.

  Historial de lo que se afirmó entonces, para no reutilizarlo:
  * `toolchain` → success (0,2 s).
  * `repo-contract` → success (0,1 s).
  * `fmt` → success (4,5 s).
  * `clippy` (`-D warnings`) → success (83,8 s).
  * `rustdoc-gate` → success (434,0 s = 7,2 min) — ya no está en el merge
    gate; vive en la lane de release-candidate.
  * `contracts` → sin medir a fondo.

  **Nota sobre el presupuesto**: el gate ya no cabe en 15 min, y la razón
  no es la segmentación de etapas —medida a 124 ms por etapa— sino el trabajo
  que hace: la batería de tests y los cuatro builds de release. `rustdoc-gate`
  ya no forma parte del merge gate; vive en la lane de release-candidate.
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
