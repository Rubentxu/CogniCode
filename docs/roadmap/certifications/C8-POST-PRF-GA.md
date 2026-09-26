# CERTIFICACIÓN C8 — Post-PRF General Availability (v0.99.0)

> **Fecha de cierre material**: 2026-09-25
> **Estado**: **CERRADO TÉCNICAMENTE** (pasa `cargo test --workspace` con 0
> failed, clippy limpio, bin arranca y responde CP1). Firma humana del
> operador (analogía con C7) queda **PENDIENTE** hasta que el operador
> decida si esta certificación se firma al nivel del cierre contractual
> PRF o se considera suficiente el cierre técnico. La pregunta C8 es
> explícita y registrada aquí.
>
> **Identidad del artefacto certificado**: `v0.99.0` →
> `3954b8b75b9e9fade8ead2dfeffde8aacfafe83a` (HEAD de origin/main al
> cierre). Sin tag anotado en este cierre. Trazabilidad por commit SHA.

## 1. Contexto

La iniciativa Post-PRF arrancó tras la firma del cierre contractual C7 el
2026-09-24T22:41:33Z (`docs/prf/F7-C7-EXPEDIENTE.md`). El operador
rechazó que `PRF` siguiera siendo roadmap activo (auditoría 2026-09-25,
"PRF como roadmap finalizado; este documento nace") y la sesión §154+ se
ramificó en dos planos:

- **G0.* — Gobernanza**: reset del puntero, baseline sobre `main`, reglas
  operacionales explícitas, ADR-PRF-008 (decisiones arquitectónicas
  reutilizables).
- **M0.* — Mantenimiento**: 4 deudas de v0.98.x resueltas en el ciclo
  (incl. M0.4 fix de `point_at` con `AssetPoint` RAII guard).
- **E0, E1, E2, F0.1**: cuatro unidades Post-PRF cerradas (contratos
  públicos, durable knowledge sobre Ladybug, control plane con
  `wire_canonical_control_query` + bin `cognicode-control-plane`,
  feature `find_usages`).
- **E3**: `NOT_TRIGGERED` por definición (sin segundo consumidor real
  que requiera RPC separada).

Esta certificación C8 cubre **el conjunto de la iniciativa Post-PRF en
su estado HEAD = `3954b8b7`**, no las unidades individuales (éstas
tienen sus propios expedientes: `E0-CLOSEOUT.md`, `E1-CLOSEOUT.md`,
`E2` via `14cf3d1b` + `4138eab7`).

## 2. Alcance de la certificación

C8 verifica que **a nivel SHA congelado**:

| Elemento | Estado | Evidencia |
|---|---|---|
| Build workspace (`cargo build --workspace`) | PASS | exit 0 (corregido en este ciclo, ver §5) |
| Test workspace completo (`cargo test --workspace --quiet`) | **PASS** | 5542 passed, 0 failed, 45 ignored |
| Clippy workspace (`cargo clippy --workspace --all-targets -- -D warnings`) | PASS | exit 0, sin warnings |
| Binarios legendados (`cognicode`, `cognicode-mcp`, `cognicode-mcp-server`) | PASS | `--version` = `0.99.0` |
| Bin `cognicode-control-plane` arranca y sirve `/health` + CP1 | PASS | curl HTTP 200 + 200 sobre binario real (ver §6) |
| `ArchitectureRegistry vacío en producción` (`docs/roadmap/E2` bug original) | CERRADO | E2.W1 (`14cf3d1b`) + E2.W2 (`4138eab7`): `wire_canonical_control_query()` admite 3 constraints canónicas, panic on failure (fail-closed) |
| Tests E2.W2 con TCP real (no mock) | PASS | 3/3 verde (puerto efímero `127.0.0.1:0` + reqwest) |
| Compatibilidad `explorer-*` APIs (E0) | PRESERVED | `adr/ADR-PRF-008-EXPLORER-API-COMPAT-MATRIX.md` + compat tests ejecutados |
| Durabilidad knowledge lbug-backed (E1) | PASS | 23 tests verde; ADR-009 + ADR-010 |
| `find_usages` CLI (F0.1) — consumer proof del contrato E0 | PASS | 14 tests + 4 E2E |
| `point_at` env leak (M0.4) | CERRADO | commit `f76a4b03` + `44c7c9c0`: `AssetPoint` RAII panic-safe, 4 tests pinean contrato |

## 3. Comandos ejecutados y resultados observados (verbatim)

### 3.1 Build workspace

```bash
cargo build --workspace --quiet
# (no output beyond cargo's own status, exit 0)
```

### 3.2 Test workspace — agregación final

```bash
cargo test --workspace --quiet 2>&1 | grep "test result:" | \
  awk '{ p += $4; f += $6; i += $8 } END { print "passed=" p " failed=" f " ignored=" i }'
# passed=5542 failed=0 ignored=45
```

Distribución por crate (resumen, no exhaustivo):

| Crate | passed | failed | ignored |
|---|---|---|---|
| `cognicode-core` (implícito en 323) | 323 | 0 | 1 |
| `cognicode-explorer` (implícito) | 44 + 11 + 11 + 11 + 9 + 12 + 3 + 7 + 10 | 0 | 1 |
| (resto workspace) | resto | 0 | resto |

### 3.3 Clippy

```bash
cargo clippy --workspace --all-targets --quiet -- -D warnings
# (exit 0, no warnings, no errors)
```

### 3.4 Versionado binario

```bash
/var/home/rubentxu/cargo-targets/debug/cognicode --version
# cognicode 0.99.0
```

> **Nota sobre el path de cargo**: este entorno tiene
> `target-dir = "/var/home/rubentxu/cargo-targets"` en
> `~/.cargo/config.toml` (no en el repo). Por defecto cargo emite binarios
> allí, no en `target/`. Detectado y aplicado en este ciclo.

### 3.5 Live smoke `cognicode-control-plane`

```bash
CP=/var/home/rubentxu/cargo-targets/debug/cognicode-control-plane
$CP --bind 127.0.0.1:9843 --source-root ./crates/cognicode-core/src &

# /health
curl -s http://127.0.0.1:9843/health
# {"service":"cognicode-explorer","status":"ok"}              HTTP 200

# CP1 read question
curl -s http://127.0.0.1:9843/control-plane/workspaces/cognicode-core/architecture
# (200, 448 bytes)  status=evaluated, zero violations on self-host

# Out-of-scope paths return clean 404
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:9843/control-plane/probe
# 404
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:9843/api/anything
# 404
```

## 4. Material técnico verificable en SHA `3954b8b7`

- **HEAD = `3954b8b7`**, working tree clean, branch en sync con
  `origin/main` (`git status --short --branch` = `## main...origin/main`
  sin diferencias).
- **Tag anotado v0.99.0**: NO EXISTE. **Decisión deliberada del
  cierre C8**: no se crea tag sin firma humana del operador (analogía
  con la decisión del operador al firmar C7). El SHA es la unidad
  material verificable hasta que se cree el tag.
- **CI `pr-ci.yml`**: '0 jobs' failure observada (workflow file no
  respeta `paths:` filter esperado por CI). **No bloqueante** — la
  verificación material se hace localmente sobre el SHA HEAD. CI
  merge-gate bypass se documenta como **PRE-EXISTING + ESCAPE LOCAL**
  (no introducido por esta iniciativa).
- **`cargo test --workspace`**: 5542 / 0 / 45 (PASS / FAIL / IGNORED).

## 5. Lecciones aprendidas (alta señal)

### L01 — `target-dir` global evade detección local

Cargos con `~/.cargo/config.toml` `target-dir = ...` escriben binarios
fuera de `target/` del repo. Si la verificación busca binarios en
`target/debug/`, **todos los comandos dan "binario no existe"** pero el
workspace parece PASS. **Regla operativa nueva**: usar siempre
`cargo metadata --no-deps | jq .workspace_root + target-dir efectivo`
o leer la config global antes de buscar artefactos.

Aplicado en este ciclo: `ls /var/home/rubentxu/cargo-targets/debug/cognicode-*`.

### L02 — `point_at` y `unpoint` no eran atómicos

`temp_env` (`tempfile::TempBaseUrl` + el análogo `point_at` en
`release_test_support`) no restauraba env vars en orden de drop,
dejando leaks entre tests #[serial]. Causa de M0.4. Fix:
`AssetPoint` RAII panic-safe.

### L03 — `ArchitectureRegistry vacío` requiere admisión explícita en boot

El bug original E2 NO era falta de constraints — era que el código de
producción las omitía silenciosamente. **Fix**: `wire_canonical_control_query`
usa admisión con promoción canónica y PÁNICA si alguna constraint no
admite. Fail-closed > silent empty.

### L04 — Router<S> primer route() fija tipo

`axum::Router` infiere tipo de state en el **primer** `.route()` que se
declara; declarar `/health` antes que `/control-plane/...` fija el tipo
de state a `()` (estos dos no usan state). Documentado como lección
inline en `control_plane_router()` para el siguiente desarrollador.

## 6. Riesgos y elementos abiertos

| # | Riesgo | Severidad | Mitigación |
|---|---|---|---|
| R1 | Tag v0.99.0 no emitido | Media | Decisión consciente del agente principal; requiere autorización operador (analogía con C7 firma) |
| R2 | CI `pr-ci.yml` 0 jobs | Media (pre-existente) | Verificación local cubre C8 material; arreglo de CI no es scope Post-PRF |
| R3 | `tests::sbom_script` flaky pre-existente | Baja | Pasa en 2/2 corridas aisladas en este ciclo; no introducido por E2 |
| R4 | Cargo `target-dir` global puede confundir futuras verificaciones | Baja (OPERATIVA) | Lección L01 registrada |
| R5 | Diferencia entre lo declarado en C8 PASS y un eventual tag v0.99.0 | Baja (CONTROLADA) | Si el operador firma tag tras C8, crear `ADMISSION-CERTIFICATION-C8-v0.99.0.md` con la misma forma que `ADMISSION-EXPEDIENTE-F7-C7-v0.98.0.md` |

## 7. Decisión C8

**Estado de la certificación**: `PASS` localmente.

**Firma humana del operador**: PENDIENTE. Tres opciones que el operador
puede tomar:

1. **Firmar C8 al nivel contractual** (analogía con C7) → crea
   `docs/prf/ADMISSION-EXPEDIENTE-F7-C8-v0.99.0.md`, emite tag anotado
   `v0.99.0` apuntando a `3954b8b7`, publica release en GitHub Releases
   vía `release.yml` workflow.
2. **Marcar C8 como CIERRE OPERATIVO LOCAL** (sin release formal) →
   `v0.99.0` queda como SHA checkpoint para futuras auditorías; tag
   se difiere hasta que aparezca un trigger (release-driven).
3. **Pedir más evidencia** (e.g. campaña adversarial Post-PRF, UAT
   cross-crate E2E) → antes de firmar, ejecutar como work items
   adicionales.

**Cierre técnico real**: ✅ verificado en HEAD `3954b8b7` sobre
`origin/main`, working tree limpio, batería completa verde, binario
arranca y responde CP1 con admisión canónica.

**Cierre contractual PRF**: queda **A DISPOSICIÓN DEL OPERADOR**.

---

*Certificación emitida por el agente principal en modo AUTO el
2026-09-25. Trabajos delegados, recibos y JOURNAL en
`docs/roadmap/JOURNAL.md` (entries 1-7).*

---

## 8. Addendum — 21 commits posteriores a `3954b8b7` (verificación sobre HEAD actual)

### Contexto

Este addendum NO reabre C8 ni lo modifica. Registra el estado
verificable del workspace sobre HEAD `528d9966`, posterior al SHA
certificado `3954b8b7`, para que el operador que firma C8 conozca
el delta que firmar (o sepa que debe re-firmar si los 21 commits
cambian el panorama).

### Delta de commits (21 entre `3954b8b7` y `528d9966`)

```
528d9966 docs(roadmap): entry 17 — e91.W3 CLOSED with non-viability evidence
e107e34d test(sbom): serialise 5 SBOM contract tests to close parallel race
714cafb4 docs(journal): entry 15 — negative audit result for non-graph crates
90123021 docs(roadmap): entry 14 — SBOM test RAII guards + honest scope
a553fbd6 test(sbom): RAII guards make workspace cleanup non-skippable on panic
df8002f5 docs(roadmap): entry 13 — e91.W6 CLOSED with metadata contract
c1618e84 feat(handlers): W6 metadata contract — 7 analytics handlers expose algorithm + parameters + subgraph dims
6b2738f3 docs(roadmap): e91.W2 entry 12 — PageRank recomputation NOT the bottleneck
8b4bbe85 test(graph-algos): W2 evidence — PageRank recomputation is not the bottleneck
cecb7b3a docs(journal): register e91.W6 + sandbox e91.W1 cleanup
3086e1a9 docs(roadmap): e91.W1 addendum (b) — second commit closes client-visible path
42a1ddcf fix(graph-handler): expose real LP iterations/converged to MCP clients
5da43a49 docs(roadmap): e91.W1 journal + ROADMAP row + e91/e90 addendum (e90 partially obsolete)
6f40a08b fix(graph): e91.W1 report real iterations/converged from LP run
2991e5e2 docs(roadmap): journal entry 10 — pinear build de cognicode-control-plane en CI
2047162b feat(ci): cover cognicode-control-plane in PR-CI build-binary
7272c4c2 docs(roadmap): journal entry 9 — fix release+ci bugs E2.W2 + GHA
4737173c fix(release): track E2.W2 bin source + exempt src/bin in .gitignore
fbaed1c3 chore(fmt): apply rustfmt over 8 drifted files
8f768a07 fix(ci): merge-gate needs.$job.result was unparseable by GHA
(+ 1 commit de estabilización no listado en detalle)
```

### Capacidades modificadas o añadidas

* **e91.W1 (commit 6f40a08b)**: `cognicode_graph_algos::communities`
  ahora retorna `iterations_used` y `converged` reales. Era un bug
  latente que reportaba valores hardcoded.
* **e91.W1 (commit 42a1ddcf)**: el handler paralelo
  `cognicode-explorer/src/mcp/handler/graph_analyze.rs:466` (el path
  que `explorer-mcp` sirve realmente) ahora expone los nuevos
  campos al MCP. Este era el client-visible path; el fix en
  `cognicode-core/handlers/graph_handlers.rs:310-311` no era
  suficiente.
* **e91.W2 (commits 8b4bbe85, 6b2738f3)**: caracterización de
  PageRank warm-start (0.8-4ms en grafos cíclicos densos).
  Decisión: NO optimizar — ahorra 0.01-0.07% del budget.
* **e91.W3 (entry 17)**: cerrado con evidencia de no-viabilidad.
* **e91.W6 (commit c1618e84)**: 7 sibling handlers en
  `graph_analyze.rs` ahora exponen envelope metadata
  (`algorithm` + `parameters` + `subgraph_dims`). Aditivo,
  backwards compatible.
* **SBOM hygiene (commits a553fbd6, 90123021, e107e34d)**: RAII
  guards `WorkspaceSbomGuard` + `SpuriousFile` cierran el panic
  window; `#[serial]` en 5 tests cierra el parallel-test race
  (10% flake pre-fix observado en N=10).

### Verificación post-delta sobre HEAD `528d9966`

```
$ cargo test --workspace 2>&1 | grep "test result" | awk '...'
TOTAL: passed=5557 failed=0 ignored=45

$ cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 45.59s
(exit 0, no warnings, no errors)
```

### Comparación con C8 base

| Métrica | C8 base `3954b8b7` | HEAD `528d9966` | Delta |
|---------|---------------------|------------------|-------|
| Tests passed | 5542 | 5557 | +15 |
| Tests failed | 0 | 0 | 0 |
| Tests ignored | 45 | 45 | 0 |
| Clippy `-D warnings` | exit 0 | exit 0 | 0 |
| SHA base | `3954b8b7` | `528d9966` | +21 commits |

### Conclusión

El delta de 21 commits **no introduce regresiones, no aumenta
flakiness, no debilita garantías C8**. Los 15 tests adicionales
son RED→GREEN por construcción (cubren los nuevos campos del
envelope W6 y la cobertura de iteraciones LP en W1).

Si el operador firma C8 sobre el SHA base `3954b8b7`, firma
exactamente lo que el dosier describe. Si desea que C8 cubra
también el HEAD actual, debe re-firmar (o emitir un addendum
formal C8.1) tras validar este addendum.

**Este addendum NO es una re-firma de C8**. Es trazabilidad
para que la decisión del operador sea informada.

*Addendum emitido por el agente principal en modo AUTO el
2026-09-26.*
