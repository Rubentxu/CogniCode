# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda
> fuente de estado: donde este documento y el ROADMAP diverjan, manda el
> ROADMAP y este snapshot se considera obsoleto hasta su próxima
> regeneración. Declarado así tras la auditoría de 2026-09-30, que encontró
> este fichero afirmando un estado (programa no iniciado, PIVOT pendiente)
> que el ROADMAP y el árbol desmentían desde hacía días — exactamente el
> drift que QW-02 define como contradictorio.

> **Snapshot**: 2026-10-14, sobre `main` HEAD
> `d22267d3290a9da96626cacad87852b2f0709182` (CD-3 cerrada: el gate completo
> `test-full.sh` da **rc=0**
> por primera vez; `check_known_failures.py` exportaba su propio error como
> `TMPDIR`, lo que fabricaba 16 falsos "NEW REGRESSIONS"; los wrappers del gate
> ahora usan `--tmpdir-root`, que hasta entonces no usaba nadie; CD-3: el stage
> del selector no podia fallar; CD-2: el veredicto de merge se publicaba para un
> commit que nadie probo; CD-1: `test-fast` no podia ejecutar ni una suite;
> cierre del bloque de blockers de superficie anunciada + unidad de claims de
> lenguaje + correccion de los mapeos de nodo de Scala);
> **70 commits ahead de `origin/main`**, sin pushear).
> Regenerado de nuevo tras detectar que el ratchet
> `scripts/ci/test_roadmap_version_ratchet.py` habia vuelto a **FAIL**: el
> SHA declarado (`97145a9c`) quedo a **6 commits** de HEAD, fuera de su
> ventana de 5. Ver `JOURNAL.md` N+113 y N+114.
>
> Version workspace: **`0.101.16`** (`Cargo.toml`), que ya tiene tag
> `v0.101.16` en `origin`. El ultimo tag publicado es `v0.101.16`.

## HEAD y bateria (MEDIDO sobre `d22267d3`, 2026-10-14)

### CD-3 cerrada — el gate completo en rc=0 (`6311217f`, `d22267d3`)

**`test-full.sh` PASS rc=0 (6/6)**, verificado con el `TMPDIR` real de la
maquina (`/home/rubentxu/.jcode/scratch`, que cruza el symlink
`/home -> /var/home`) y **sin exportar nada a mano**:

| Paso | Resultado |
|---|---|
| `fmt --check` | **OK** (antes: FAIL, 100 diffs) |
| `clippy -D warnings` | **OK** (antes: FAIL, 7 errores) |
| `test --workspace` | **OK** (antes: FAIL, 41) — **6607 passed / 0 failed** |
| `test --workspace --doc` | **OK** |
| `known_failures (core/lib)` | **OK — 0 entries** (antes: FAIL, 16 falsos) |
| ratchet roadmap | **OK** |

Tres correcciones de encuadre respecto a lo que este snapshot afirmaba:

* Los **41 fallos** de `--workspace` **no** eran feature unification. El
  arbol era identico; la causa era un binario de test obsoleto en un
  `CARGO_TARGET_DIR` reutilizado, con rutas horneadas a un harness temporal
  ya borrado. Con target dir nuevo: 3134/0.
* Los **16 "NEW REGRESSIONS"** los fabricaba `check_known_failures.py`
  comprobando el campo equivocado de una tupla: exportaba su **mensaje de
  error** como `TMPDIR`, y ese mensaje acababa tambien como **nombre de ruta**
  dentro del repo, lo que ademas bloqueaba el merge-gate por el guard de
  arbol sucio de CD-2.
* El flag `--tmpdir-root` **ya existia** y no lo usaba nadie. Los wrappers
  resuelven ahora una raiz sin symlink por su cuenta.

Nota operativa: `rc=137` (SIGKILL) visto en una corrida intermedia era
contencion de recursos por corridas concurrentes, no un defecto; la corrida
limpia dio rc=0.

### CD-3 — el stage del selector no podia fallar (`97145a9c`)

**Correccion de encuadre, primero.** El plan de CD-3 decia "conectar la
seleccion real de suites al pipeline de merge". Hacer eso habria **debilitado**
el gate: `merge-gate.pipeline.kts` corre sus **73 stages de forma
incondicional** y no hay ninguna condicion de diff que los salve, que es lo
correcto para un gate de merge. El nombre de la unidad era el error. El test
nuevo **fija que el gate siga incondicional**, para que el "arreglo" obvious no
llegue nunca.

El hallazgo real es mas pequeño: `select-suites-script` calculaba `SELECT_OUTPUT`
y lo imprimia, sin leerlo ni validarlo. Medido: el selector sale rc=0 con un path
valido **y con uno que no existe**, asi que el stage no podia detectar un
selector roto, un typo en el path fijo ni una seleccion malformada. Ademas su
comentario afirmaba que la decision era "consumable downstream"; medido, **no la
consume nadie**.

El riesgo no era una brecha de seguridad —el gate no depende de la seleccion, asi
que un selector roto no puede reducir la cobertura— sino un gate que **parece**
mas estrecho de lo que es.

* `scripts/ci/test_merge_gate_selection_truth.py`: 5 comprobaciones sobre la
  estructura del pipeline.
  * RED **2 checks** contra HEAD, **0** despues. Verificado restaurando
    `git show HEAD:merge-gate.pipeline.kts`.
  * GREEN **5/5**; suite de contratos **8/8**.
  * Dos comprobaciones del propio test eran incapaces de fallar: buscaban la
    frase retirada en el codigo cuando vivia en un comentario, y buscaban
    `SELECT_OUTPUT` en un stage donde `echo "$SELECT_OUTPUT"` la contiene. Un
    test que no falla antes del arreglo no es un test.

**Gates CD-3**: `test_merge_gate_selection_truth.py` PASS 5/5 · suite de
contratos 8/8 · ratchet OK · **`test-full.sh` NOT_RUN**.

### CD-2 — el veredicto de merge se publicaba para un commit sin probar (`b5b8ae03`)

`scripts/ci/publish-merge-gate.sh` es el puente que publica el contexto
`merge-gate` que `main` exige con `strict: true`. Resolvia el SHA y lo
publicaba, pero **la autoridad de merge nunca recibia ese SHA**: sin checkout,
sin argumento, y `merge-gate.pipeline.kts` no lo menciona. Medido con el puente
real y `pipelinek`/`gh` de mentira:

```
published sha = 367813eb...   (el que se nombra al publicar)
tested   head = 7cfe749e...   (el que la autoridad probo)
```

Con `strict: true`, ese `success` desbloquea el merge, y no hace falta
untruear nada: basta apuntar a un commit que ya se sabe bueno mientras el arbol
que se prueba es otro.

Corregido con tres guards: el argumento tiene que ser un commit del repositorio,
tiene que ser **HEAD**, y el arbol tiene que estar limpio. Se **rechaza** en
vez de hacer checkout: la autoridad prueba el arbol en el que corre, asi que la
unica forma de que el veredicto describa el commit nombrado es no correr para
ningun otro.

* `scripts/ci/test_merge_gate_sha_authority.py`: 5 comprobaciones que ejecutan el
  puente y afirman sobre el **SHA publicado** y **que arbol se probo**, no
  sobre el texto. Un test que greppara `git checkout` pasaria con la linea
  presente pero sin usarse.
  * RED **3/4**. El cuarto daba verde por un defecto del propio test (la URL
    llega como `statuses/<sha>` sin barra final), lo que tapaba el hallazgo.
  * GREEN **5/5** (6.2 s).
  * Mutation **3 de 4**. `drop_verify_commit` **sobrevive**: el check de HEAD
    rechaza el SHA invalido igualmente por debajo. Se conserva y se declara que
    no tiene cobertura independiente.

* Regresion de integracion: `test_publish_merge_gate_status.py` corria el puente
  en el arbol real del desarrollador y, con el guard de arbol sucio, fallaba
  siempre que hubiera trabajo sin commitear. Ahora usa un checkout limpio.

**Gates CD-2**: `test_merge_gate_sha_authority.py` PASS 5/5 ·
`test_publish_merge_gate_status.py` PASS · ratchet OK ·
**`test-full.sh` NOT_RUN**.

### CD-1 — `test-fast` no ejecutaba ninguna suite (`26e04905`)

`scripts/test-fast.sh` era un **falso verde de forma total**: no ejecutaba ni
una sola suite desde que el selector paso a emitir JSON. El `sed` que extraia
`suites` conservaba las comillas, de modo que el token iterado era
`["ladybug","core"]` y el `case` de `run_suite` no casaba con nada. El abort por
`set -u` sobre `roadmap_rc` sin inicializar enmascaraba el defecto y devolvia un
rc que parecia el de un fallo de tests. Medido:

```
$ bash scripts/test-fast.sh --paths crates/cognicode-core/src/lib.rs
selector -> strategy=changed suites=["ladybug","core"]
scripts/test-fast.sh: linea 151: roadmap_rc: variable sin asignar
# 0.2 s, cero tests ejecutados
```

Todo lo que `fast` "verificaba" en las iteraciones anteriores era el ratchet del
roadmap. Ningun cambio de codigo de las ultimas unidades pudo ser detectado por
`fast`, porque `fast` no ejecutaba tests.

Corregido sin tocar el producto: parseo real del JSON con `python3` (ya
dependencia del script), rc del selector ya no descartado, suite desconocida
`return 1` en vez de `return 0`, `roadmap_rc` inicializado, y `suites: []` cae al
subset conservador **con aviso** en lugar de `exit 0` (se mantiene la regla 2 de
AGENTS.md: el fallback avisa, no falla).

* `scripts/ci/test_fast_truthfulness.py`: 7 comprobaciones que **ejecutan** el
  wrapper con selector y `cargo` de mentira. Los tests previos del wrapper
  greppeaban texto y habrian dado verde sobre este bug.
  * RED **7/7**, cada uno por su causa propia (1 m 09 s).
  * GREEN **7/7** (33 s).
  * Mutation **4/5 mutantes mueren**. El quinto no lo mata ningun test, asi que
    se **elimino** la guarda que protegia en vez de dejar codigo muerto.

**Hallazgo lateral, preexistente y NO resuelto**: `cargo test -p cognicode-core
--lib` pasa **2352/0**, pero `cargo test --workspace --lib` corre **41 tests mas
que fallan**. Es *feature unification*: `cognicode-cli` habilita
`cognicode-core/evidence-cli-ladybug`, de modo que los dos comandos prueban
conjuntos distintos de tests. Verificado con el codigo de `HEAD` y el mio, y con
`CARGO_TARGET_DIR` nuevo y reutilizado: el resultado es identico. Queda abierto
para CD-2.

**Gates CD-1**: `test_fast_truthfulness.py` PASS (7/7) ·
`test_fast_full_wrappers.py` PASS · `check_known_failures.py` con
`--tmpdir-root` real PASS (0 entradas) · **`test-full.sh` NOT_RUN**.

Correcciones a las afirmaciones del snapshot anterior (2026-10-11), que
eran **falsas** y se dejaron pasar 112 commits:

| Afirmacion previa | Realidad medida |
|---|---|
| `cargo fmt --all --check` → exit 0 | **100 ficheros** con deriva de fmt → **arreglado** en `b6c7d613` |
| version workspace `0.101.10` | **`0.101.16`** |
| `next release v0.101.11` | ya publicados hasta **`v0.101.16`** |
| "`docs/roadmap/*` esta excluido por `.gitignore`" | los 4 ficheros **si estan versionados** |
| "40 commits ahead" | **57** (hoy **70**) |

* **Bateria workspace** (medida en este SHA, target dir por defecto):
  * `cargo test --workspace` → **6607 passed / 0 failed** (exit 0).
  * `cargo test --workspace --doc` → **OK**.
  * `cargo fmt --all --check` → **OK** (arreglado en `b6c7d613`; antes FAIL
    con 100 ficheros de deriva previa).
  * `cargo clippy --workspace --all-targets -- -D warnings` → **OK**
    (arreglado en `b6c7d613`; antes FAIL con 7 errores, que aparecian por
    capas: core enmascaraba a cli, que enmascaraba a los targets de test).
  * `scripts/ci/test_roadmap_version_ratchet.py` → **ver arriba, FAIL →
    regenerado**.

## Deuda de fmt y clippy: declarada, no tocada

Los **100** ficheros de fmt y los **11** de clippy son deuda **preexistente**
en `main`, no introducida por el bloque de blockers. Medido: la interseccion
entre los ficheros con aviso de clippy y los ficheros que el bloque toco es
**vacia**. Se dejan intactos a proposito — mezclarlos con correcciones de
blocker haria la revision mas cara y el rollback mas dificil, que es
justamente lo contrario de lo que un commit atomico pide.

## Estado de las filas del snapshot anterior

| Fila del snapshot 2026-10-11 | Estado medido 2026-10-14 |
|---|---|
| T0-1..T0-3 (dead code, `unimplemented!`) | CLOSED, conservado |
| T0-5 disparar merge-gate | OPEN, sin ejecutar en este bloque |
| T1-1..T1-5 connascence cleanup | OPEN, **sin tocar por este bloque** |
| TF-1..TF-4 promote + tag v0.101.10 | **OBSOLETO**: ya publicados hasta v0.101.16 |
| SDDK cycle creation | sigue BLOQUEADO por `ledger.sqlite` vacio |

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

*Mantenedor: agente principal en modo AUTO. Regenerado 2026-10-14 desde el
HEAD real `4cde1257`, reemplazando el snapshot 2026-10-11 sobre `6af11bf8` que
llevaba **112 commits** de deriva y cinco afirmaciones verificadas como falsas
(fmt limpio, version 0.101.10, `docs/roadmap/*` sin versionar, 40 commits
ahead, proximo release v0.101.11). Motivo del hallazgo: el ratchet
`test_roadmap_version_ratchet.py` estaba en FAIL desde `1c664066` y **ningun**
gate lo invoca — un ratchet sin red. Ver `JOURNAL.md` N+106.*
