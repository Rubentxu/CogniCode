# E1-CLOSEOUT — Durable Knowledge sobre Ladybug

**Estado**: **CLOSED 2026-09-25**
**Sustituye a**: (cierre original sin writeup formal; este es el writeup definitivo)
**ADR rector**: [`docs/roadmap/adr/ADR-009-E1-EVIDENCE-STORE-SCOPE.md`](ADR-009-E1-EVIDENCE-STORE-SCOPE.md) (scope), [`docs/roadmap/adr/ADR-010-KNOWLEDGE-EVIDENCE-NAMESPACE.md`](ADR-010-KNOWLEDGE-EVIDENCE-NAMESPACE.md) (namespace split)
**Commits cubiertos**:
- `7611a589` — `feat(ladybug): LadybugEvidenceStore impl real (E1.W1 / ADR-009)`
- `425b0fb3` — `feat(runtime): wire EvidenceStore port from bootstrap_ladybug (E1.W2)`
- `b7026475` — `feat(core,cli,mcp): cognicode evidence list|search + MCP tools (E1.W3)`
- `ffb85b6b` — `docs(adr): ADR-010 KnowledgeEvidence namespace split (E1 cierre)`

---

## 1. Lo que E1 prometía (definición del scope)

ADR-009 (2026-09-25) cerró la decisión de scope de E1:

> Implementar `LadybugEvidenceStore` (lectura) como **única** materialización del puerto `EvidenceStore` en LadybugDB. NO nuevos puertos especulativos (`FactStore`, `SnapshotStore`, `EvidenceStore kernel`) sin consumidor real.

Esto alineó E1 con la regla PRF de "no código especulativo" y dejó dos WorkUnits explícitos:

| WU | Descripción | Estado |
|---|---|---|
| **E1.W1** | `LadybugEvidenceStore` impl real (sustituye stub) | ✅ commit `7611a589` |
| **E1.W2** | Wiring en `cognicode-runtime` (port propagation + SearchService) | ✅ commit `425b0fb3` |
| **E1.W3** | CLI `cognicode evidence list|search` + MCP `list_evidence|search_evidence` + tests equivalencia | ✅ commit `b7026475` |

El tercer WU (E1.W4: writer port) queda **fuera del scope E1** — ADR-009 lo dejó explícito: el escritor necesita un consumidor real antes de implementarse, no se construye especulativamente.

## 2. Lo que se entregó (delta medible)

### 2.1 Backend (`cognicode-ladybug`)

- `init_evidence_schema()` (DDL idempotente, single-line para evitar el bug lbug 0.19 de multi-line no-op).
- `evidence_ddls()` produce `CREATE NODE TABLE IF NOT EXISTS KnowledgeEvidence(...)` con 8 columnas.
- `LadybugEvidenceStore` con `list_evidence(workspace, kind)` y `search_evidence(workspace, query, limit)`.
- 12 tests inline `#[serial]` (lbug 0.19 no thread-safe a nivel de connection): pinean nombre de tabla, shape de columnas, filtrado por kind, búsqueda case-insensitive, schema idempotente.

### 2.2 Runtime (`cognicode-runtime`)

- `Runtime.evidence_store: Option<Arc<dyn EvidenceStore>>` añadido.
- `RuntimePorts.evidence_store` añadido.
- `bootstrap_ladybug` propaga `Some(store.clone())`; `bootstrap` y `bootstrap_with_backend` propagan `None`.
- `into_api_state` invoca `SearchServiceImpl::new(...).with_evidence_store(self.evidence_store.clone())` — el único consumer que importa.
- 2 tests `evidence_store_wiring_smoke.rs` (multi-thread tokio) verde.

### 2.3 CLI (`cognicode-cli`)

- Feature opt-in `ladybug` (default OFF) que activa la dep `cognicode-ladybug`.
- Subcomando `cognicode evidence {list|search}` con `--workspace`, `--kind`, `--query`, `--limit`, `--db-path`, `--format text|json`.
- Adaptador `LadybugEvidenceBackend` (factory + register).
- Patrón hexagonal: el core define el puerto (`EvidenceBackend` trait + factory registry); el cli provee el adaptador.
- 9 tests `evidence_cli_mcp_equivalence.rs` verde (CLI JSON shape contract).

### 2.4 MCP (`cognicode-mcp`)

- 2 tools nuevos: `list_evidence` y `search_evidence` (gated `feature = "evidence-cli-ladybug"`).
- Mismo `EvidenceBackend` registry — la factory del CLI (o del runtime) sirve a ambos lados.
- Misma función `render_evidence_rows_json` para CLI y MCP (fuente única de la verdad del JSON shape).

### 2.5 ADR-010 (namespace split)

- Cierra el colgajo que E1.W1 dejó explícito en `init_schema.rs` líneas 65-68: `the future ADR-010 will document the namespace split`.
- Tabla backing del `EvidenceStore` se llama `KnowledgeEvidence` (NO `Evidence`) porque el `RunLineageStore` ya posee una tabla `Evidence` con esquema incompatible (provenance con `valid_from/to` vs snapshot con `confidence`).
- Cumple la lección FINAL-STATE §31: dos nombres iguales en dominios distintos no se fusionan.

## 3. Lo que NO se entregó (y por qué)

### 3.1 Writer port del `EvidenceStore` (E1.W4)

**No entregado.** ADR-009 lo dejó fuera del scope E1: no hay consumidor real que escriba filas de evidence. Cuando aparezca uno (típicamente el `find_usages` o un futuro `inspect`), se escribirá un WU nuevo con su propio ADR de scope.

### 3.2 `FactStore` y `SnapshotStore`

**No entregado.** ADR-009 explícitamente NO construyó estos puertos especulativamente. Si la futura necesidad del `inspect` los demanda, será un ciclo de trabajo nuevo con su propio ADR.

### 3.3 Persistencia del `evidence-kernel` (feature `multimodal`)

**No entregado.** El feature `evidence-kernel` en `cognicode-core` ya existe (gated `feature = "evidence-kernel"`) pero su persistencia queda pendiente. ADR-010 §6.1 lo documenta como riesgo abierto: si ese feature llega a persistir, NO puede usar `KnowledgeEvidence` (nombre cogido por E1).

### 3.4 Pine de la CLI en CI

**Parcialmente entregado.** El `cognicode-cli` tiene 3 tests pre-existentes fallando (`install::t_l4_install_emits_warning_when_no_skill_bundle_present`, `installer_transaction::commit_writes_journal_next_to_install`, `layout::cmd_rollback_after_live_install`). Estos NO son regresiones del trabajo E1 — son fallos pre-existentes del gating del CLI en CI. Documentados en L3.W1 commit `4d93873c`. Resolución fuera del scope E1.

## 4. UAT pineada por tests

E1 no abrió UAT formal (es un vertical nuevo sin consumer externo). En su lugar, pineamos el contrato con tests automatizados:

| Test | Cubre | Estado |
|---|---|---|
| `cognicode-ladybug::evidence_store` (12 tests inline) | Schema, idempotencia, list/search, kind filter, search vacío | ✅ |
| `cognicode-runtime::evidence_store_wiring_smoke` (2 tests) | Port propagado, idempotencia bootstrap | ✅ |
| `cognicode-cli::evidence_cli_mcp_equivalence` (9 tests) | CLI JSON shape contract, MCP/CLI agreement | ✅ |

Los 23 tests verde que pinean E1 viven en sus crates respectivos (no en `tests/uat/`). Si en el futuro E1 entra en una release candidate, esos tests sirven de UAT-1.

## 5. Métricas de cobertura

- **Líneas nuevas**: ~1.500 (counted: 883 del commit E1.W3 + 134 wiring smoke + DDL + ADR docs + tests).
- **APIs nuevas públicas** (cambios breaking menores): `EvidenceStore::list_evidence`, `EvidenceStore::search_evidence` ya existían como traits; el adapter y la feature gate son aditivos.
- **Pine tests**: 23 tests inline sobre el comportamiento de E1.
- **Pine CI**: `cognicode-ladybug unit tests` step pineado en `merge-gate` desde L3.W1 (`4d93873c`). La integración E2E en CI remoto está bloqueada por el workflow file issue pre-existente (fuera de scope E1).

## 6. Decisiones técnicas notables (resumen)

| Decisión | Razón | ADR |
|---|---|---|
| `KnowledgeEvidence` (no `Evidence`) como tabla Cypher | Conflicto pre-existente con `RunLineageStore.Evidence` | ADR-010 |
| DDL single-line | lbug 0.19 silencia multi-line con `\` continuation como no-op | inline en `init_schema.rs` |
| `Value::Null` como tuple variant `Null(LogicalType)` | API lbug 0.19 | inline en `init_schema.rs` |
| Sin `to_lower`/`to_upper` Cypher | lbug 0.19 no expone | filtro case-insensitive se hace en Rust |
| `#[serial]` en tests de evidence_store | lbug 0.19 connection no es thread-safe | inline en `evidence_store.rs` |
| Factory pattern en CLI (no `Arc<Backend>` directo) | CLI subcommand permite `--db-path` por invocación | inline en `evidence_backend.rs` |
| `render_evidence_rows_json` `pub(crate)` (no `pub`) | CLI y MCP ambos la consumen; no exponer API pública | inline en `commands.rs` |
| `EvidenceBackend` trait separado del `EvidenceStore` dominio | Adapter pattern; el core no conoce lbug | inline en `evidence_backend.rs` |

## 7. Riesgos abiertos

1. **Writer port (E1.W4)**: si un consumer lo demanda, hay que escribir un WU nuevo con su propio ADR de scope y un Cypher `MERGE (n:KnowledgeEvidence {...})` pineado.
2. **`evidence-kernel` persistencia**: ADR-010 §6.1 documenta el riesgo. Resolución: ADR-011+ cuando proceda.
3. **3 tests pre-existentes de CLI fallando**: ortogonal a E1. Backlog de mantenimiento.
4. **CI workflow file issue**: ortogonal a E1. Bloqueante externo desde hace 2+ horas.

## 8. Cierre

E1 está implementado, integrado y verificado a nivel local. Los criterios de cierre del roadmap (código mergeado, tests verde, ADRs documentados) están satisfechos. El bump SemVer (F0.* serie → v0.99.0) está pendiente — se trata en el writeup de SemVer bump, no aquí.

**E1 pasa de PENDING a CLOSED** con este writeup. ROADMAP.md actualizado en el mismo commit.
