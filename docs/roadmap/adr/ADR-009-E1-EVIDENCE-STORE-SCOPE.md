# ADR-009 — E1 Scope: EvidenceStore durable sobre LadybugDB

**Status:** PROPOSED · **Date:** 2026-09-25 · **Cycle:** Post-E0, pre-E1.W1

**Contexto.** E0 cerrado (ver `docs/roadmap/E0-CLOSEOUT.md`). Próximo bloque
Post-PRF es E1 (Durable Knowledge sobre Ladybug). El roadmap dice:

> E1 · Durable Knowledge sobre Ladybug (ADR + FactStore + SnapshotStore
> + EvidenceStore kernel + wiring) — PENDING

El "ADR" sobre EvidenceStore es prerequisito antes de código, porque el
histórico PRF advirtió (ADR-PRF-003 §STATE, FINAL-STATE §31):

> 4. **Dos nombres iguales en dominios distintos no se fusionan.**
>    `EvidenceStore` en `domain::evidence_kernel::ports` ≠
>    `domain::ports::evidence_store`. Un ADR previo a la fusión.

Hoy el estado es:

- `domain::ports::evidence_store::EvidenceStore` (trait) — read-only
  (list + search). Ya existe. Stub `LadybugStore::list_evidence` /
  `search_evidence` en `crates/cognicode-ladybug/src/evidence_store.rs`.
- `domain::evidence_kernel::ports::EvidenceStore` — **NO EXISTE en el
  repo actual** (búsqueda exhaustiva en `crates/` confirma un solo
  `EvidenceStore`, el de `domain::ports`). El conflicto de nombres
  que ADR-PRF-003 temía **ya no aplica**; el namespace quedó limpio.

Esta ADR resuelve:

1. **No hay ADR de fusión pendiente** — el conflicto original era entre
   LSI hypothetical namespace y el namespace actual; el LSI namespace
   nunca llegó a materializarse en este repo. La advertencia de
   FINAL-STATE §31 es **histórica** y se da por cerrada sin acción.
2. **Scope de E1** se acota a: completar el stub `LadybugEvidenceStore`
   con DDL + impl real, pineado por tests de comportamiento.
3. **Primer consumer** (justificación E1 ≠ build especulativo): exponer
   la lista de evidencias de un workspace vía CLI/MCP tool, validando
   que un operador puede recuperar la historia de una investigación.

---

## Decisión 1 — No-ADR de fusión

**Decisión:** no escribir ADR de fusión `EvidenceStore` porque no hay
dos `EvidenceStore` que fusionar. El segundo (`evidence_kernel::ports`)
nunca se materializó; el único `EvidenceStore` real es
`domain::ports::evidence_store`.

**Justificación:** ADR-PRF-003 §STATE advertía sobre un namespace LSI
hipotético que en este repo no existe. Mantener el `EvidenceStore` actual
como autoridad única es consistente con el principio PRF "un dato es
canónico, derivado o efímero" (ADR-PRF-003 §Decisión propuesta). Si en
el futuro LSI materializa un segundo `EvidenceStore` en otro namespace,
se reabre este ADR con diff concreto.

**Acción:** cerrar la referencia en `FINAL-STATE.md` §31 con nota
"resuelto por no-aplicabilidad" en la siguiente entrada del JOURNAL.

## Decisión 2 — Scope de E1: un solo store

**Decisión:** E1 implementa **solo** el stub `LadybugEvidenceStore`
(ya declarado en `cognicode-ladybug::evidence_store`). NO introduce
`FactStore`, `SnapshotStore`, ni `EvidenceStore kernel` (estos
términos del roadmap vienen del paquete LSI histórico, no del repo
actual).

**Justificación:**

- El roadmap E1 menciona `FactStore + SnapshotStore + EvidenceStore
  kernel + wiring`. **Estos artefactos no existen en el repo**. Crearlos
  sería **construcción especulativa** sin consumer real, violando
  `docs/prf/specs/SPEC-EXTENSIBILITY.md` y AGENTS.md ("no abrir
  packs/IA autónoma salvo decisión explícita de desvío PRF").
- `EvidenceStore` (existente, read-only) **sí** tiene un consumer
  potencial real: la búsqueda de evidencia histórica durante
  investigations (`crates/cognicode-explorer/src/facades/search.rs:47`
  ya lo inyecta como `Option<Arc<dyn EvidenceStore>>`).
- El consumer `search.rs` actualmente recibe `None` en runtime
  (revisar wiring en `cognicode-explorer/src/runtime/wiring.rs`
  pendiente) — implementar `LadybugEvidenceStore` permite
  reemplazar el `None` con un adapter real.

**Acción:** E1.W1 = implementar `LadybugEvidenceStore` siguiendo el
patrón `NarrativeStore`/`QualityStore` (DDL en `init_schema.rs`,
impl en `evidence_store.rs`, tests en `cognicode-ladybug/tests/`).
E1.W2 = wire `LadybugStore` como `EvidenceStore` provider en
`cognicode-explorer/src/runtime/wiring.rs` y verificar que
`search.rs` ve la lista no-vacía. E1.W3 = consumer CLI/MCP para
exponer `list_evidence` (sigue el patrón de F0.1).

## Decisión 3 — Primer consumer de E1

**Decisión:** el primer consumer externo es un comando CLI
`cognicode evidence list --workspace <ws> [--kind log|trace|...]`.
El consumer interno es `SearchService` en `cognicode-explorer` que
pasa de `Option<Arc<dyn EvidenceStore>>` con `None` a `Some(...)`
con `LadybugStore`.

**Justificación:** el consumer CLI es paralelo a F0.1 (consumer
proof del contrato MCP) — es la **misma receta** que pineó E0.
El consumer interno (`SearchService`) valida que el wiring no es
meramente cosmético.

**Acción:** E1.W3 = CLI subcommand `cognicode evidence list`,
análogo a F0.1 `cognicode find-usages`. Tests de equivalencia
CLI ↔ MCP tool (mismo patrón que L1.4.W3).

## Decisión 4 — NO LadybugDB schema break

**Decisión:** el DDL del nuevo `Evidence` node table es
**aditivo**. NO se renombra ninguna tabla existente. NO se
modifican índices activos.

**Justificación:** la rama `main` está consumiendo `LadybugStore`
en producción (F0..F7, MCP tools, search). Un schema break
rompería back-compat del binario, contradiciendo la política E0
recién pineada (compat matrix 0.97.x). ADR-PRF-006 §GATES
exige migrations aditivas.

**Acción:** `CREATE NODE TABLE IF NOT EXISTS Evidence(...)` con
`IF NOT EXISTS` (idempotente). PK sintético `{}::{}` de
`(workspace_id, evidence_id)`. Sin FK, sin índices únicos más
allá del PK (los índices secundarios se añaden en W2 si el
benchmark lo exige).

---

## Plan operativo (3 work units)

| WU | Descripción | Tipo | Criterio de cierre |
|---|---|---|---|
| **E1.W1** | Implementar `LadybugEvidenceStore::list_evidence` + `search_evidence` siguiendo el patrón `NarrativeStore`. DDL aditivo en `init_evidence_schema`. Tests unit + integration en `cognicode-ladybug/tests/evidence_store_test.rs` (3+ tests: list vacío, list con evidencia, search por substring). | código | `cargo test -p cognicode-ladybug evidence_store` verde |
| **E1.W2** | Wire `LadybugStore` como `EvidenceStore` provider en `cognicode-explorer/src/runtime/wiring.rs`. Eliminar el `None` por defecto. Test e2e: `cargo test -p cognicode-explorer evidence_wiring` confirma que `SearchService` ve evidencia. | código | `cargo test -p cognicode-explorer --test evidence_wiring` verde |
| **E1.W3** | CLI subcommand `cognicode evidence list` (F0.1-style consumer proof del puerto LSI). Adapter thin en `commands.rs::execute_evidence_list`. MCP tool `list_evidence` como contraparte. Tests de equivalencia CLI ↔ MCP. ADR-post-review opcional. | código + docs | 6+ tests verde; UAT binaria manual |

**No-go explícito** para E1.W1-W3: NO LadybugDB schema break, NO
nuevos puertos (`FactStore`/`SnapshotStore`), NO Loop automático de
self-hosting (se documenta como follow-up en ADR-009-appendix si
aparece demanda).

---

## Riesgos identificados

| Riesgo | Mitigación |
|---|---|
| `LadybugEvidenceStore` se implementa pero `wiring.rs` no la inyecta → código muerto | E1.W2 obligatorio antes de cerrar E1; merge-gate pine con test de wiring |
| Tests del adapter LadybugDB dependen de un DB real → flaky | Usar `LadybugStore::new` (raw) en tests, sin `open` (que aplica DDL); el DDL se testea por separado en `init_schema_test.rs` |
| Self-hosting E1 (CogniCode analiza CogniCode) revela problemas del propio parser | E1.W4 opt-in: ejecutar `cognicode evidence list --workspace cognicode-self` y verificar volumen. NO bloqueante. |
| El nombre `EvidenceStore` colisiona con algo externo (e.g. lib nueva) | `git grep -r EvidenceStore` antes de merge; renombrar a `EvidenceStorePort` solo si hay colisión real |

---

## Cierre de la ADR

Esta ADR se cierra cuando E1.W1, E1.W2 y E1.W3 estén mergeados en
`origin/main` con tests verde. El recibo se publica en
`docs/roadmap/JOURNAL.md` y el estado de E1 en el ROADMAP ejecutivo
pasa de PENDING a CLOSED.

Tras E1 cerrado, el siguiente bloque Post-PRF es **E2** (Constraint
→ Evidence → Architecture decision real), ya recogido en el
roadmap como PENDING.
