# Roadmap: Arquitectura CogniCode — Profundización 2026

> **Proyecto:** CogniCode Core  
> **Iniciado:** 2026-06-11  
> **Estado:** Planning  
> **Fuente:** auto-grill-loop (14 preguntas, 2 passes, coverage 100%)  
> **ADRs:** ADR-001 a ADR-006  

---

## 1. Resumen Ejecutivo

Este roadmap documenta las decisiones arquitectónicas para profundizar 6 candidatos de deuda técnica en `cognicode-core`. El trabajo se ejecuta en **5 waves** con gates CI automatizados, minimizando blast radius y habilitando paralelismo donde los candidatos son independientes.

**Impacto estimado:** ~1,450 Δlines distribuidas en 5 waves.  
**Riesgo:** Bajo-Medio. Las waves 1 y 2 son aditivas. Las waves 4-5 tienen rollback path documentado.  
**Bloqueo crítico:** Wave 3 (C4 Schema/DTO) gating Wave 5 (C1 Tool Registry).

---

## 2. Candidatos — Estado Actual

| # | Candidato | Ubicación | Tamaño | Estado | ADR |
|---|----------|-----------|--------|--------|-----|
| C1 | Tool Registry (`#[aix_tool]`) | `rmcp_adapter.rs` | ~2205 l. | 🔴 Propuesto | ADR-001 (boundary), macro en ADR-003 |
| C2 | HandlerContext Builder | `handlers/mod.rs` | ~600 l. | 🟡 Propuesto | Split en C2a/C2b |
| C3 | WalkFilter (SKIP_DIRS) | `domain/value_objects/` | 9 duplicados | 🟢 Propuesto | ADR-004 |
| C4 | Schema/DTO Unification | `schemas.rs` + `dto/` | ~5400 l. | 🔴 Propuesto | ADR-001, ADR-003 |
| C5 | ReadMode Static Dispatch | `file_operations.rs` | ~3226 l. | 🟢 Propuesto | ADR-005 |
| C6 | Mock Crate Separation | `domain/traits/` | ~370 l. | 🟢 Propuesto | ADR-006 |

**Leyenda estado:**
- 🔴 Propuesto — necesita implementación
- 🟡 En progreso
- 🟢 Completado
- ⚠️ Bloqueado

---

## 3. Plan de Ejecución — 5 Waves

```
═══════════════════════════════════════════════════════════════════════════════
                        ROADMAP: Arquitectura CogniCode
═══════════════════════════════════════════════════════════════════════════════

  Wave 1               Wave 2         Wave 3           Wave 4       Wave 5
┌──────────────┐   ┌───────────┐  ┌────────────┐  ┌───────────┐  ┌───────────┐
│ C3 + C5 + C6 │ → │  C2a      │ → │   C4       │ → │   C2b     │ → │   C1      │
│ (PARALELO)   │   │  Builder  │  │ Unification│  │  Deletion │  │ Tool Reg. │
└──────────────┘   └───────────┘  └────────────┘  └───────────┘  └───────────┘
  ~500 Δlines        ~150 Δlines   ~500 Δlines     ~50 Δlines    ~200 Δlines

  └─ C3: WalkFilter     └─ C2a: HandlerContext Builder (aditivo)
  └─ C5: ReadMode          Requisito: ninguno
  └─ C6: Mock crate     Gate CI: coexistence tests + deprecated count = 0
  Gate CI: bench <5%
                      Gate CI: C2a en prod + wrappers deprecated eliminados

  ⚠ GATE: C4 debe completarse antes de C1 (o 3 precondiciones)

═══════════════════════════════════════════════════════════════════════════════
```

### Wave 1 — C3 + C5 + C6 (Paralelo, ~500 Δlines)

**并行 execution.** Los 3 candidatos tocan archivos completamente disjuntos. Zero merge conflict risk.

#### C3: WalkFilter Value Object
- **Archivos:** 5 archivos con SKIP_DIRS duplicados
- **ΔLines:** ~150
- **Entregable:** `domain/value_objects/walk_filter.rs`
  - `WalkDecision` enum: `Include | Skip | Prune`
  - `WalkFilter` struct con builder: `.with_security_blocklist()` + `.with_performance_skips()`
  - Función: `fn(&Path) -> WalkDecision`
- **Gate CI:** Test suite passing + bench <5% regression

#### C5: ReadMode Static Dispatch
- **Archivos:** 2 (dto/file_ops.rs + file_ops_handlers.rs)
- **ΔLines:** ~100
- **Entregable:** Refactor de dispatch a enum estático
  - Eliminar trait objects si existen
  - `match` exhaustivo en los 4 ReadMode variants
- **Gate CI:** 4 modos compilan + tests pasan

#### C6: Mock Crate Separation
- **Archivos:** ~15 (domain/traits/*.rs)
- **ΔLines:** ~250
- **Entregable:** `crates/cognicode-core-mock/`
  - Cargo.toml con version lockstep
  - `src/lib.rs` con re-exports + mocks
  - Tests de integración contra domain traits
- **Gate CI:** Mock crate compila + integration tests passing

---

### Wave 2 — C2a: HandlerContext Builder (~150 Δlines)

**Cambio aditivo puro.** Sin riesgo de blast radius.

- **Archivo:** `handlers/mod.rs`
- **ΔLines:** ~150
- **Entregable:** `HandlerContext::builder()`
  - Builder pattern con todos los campos actuales
  - `#[deprecated]` thin wrappers sobre constructors existentes
  - Coexistence: código viejo y nuevo funcionan en paralelo
- **Gate CI:** `#[deprecated]` wrapper count = 0 al final de la wave (verificado por linter)

---

### Wave 3 — C4: Schema/DTO Unification (~500 Δlines) ⚠️ GATE for C1

**La wave más crítica.** Define la frontera MCP/domain para todo el trabajo futuro.

- **Archivos:** 5+ (`schemas.rs`, `application/dto/*.rs`, `dto_mapping.rs`)
- **ΔLines:** ~500
- **Entregable:**
  - Macro `#[newtype]` en `cognicode-macros`
  - Los 24 pares schema/DTO como newtypes
  - `schemas.rs` sin imports de `application::dto`
  - `dto_mapping.rs` eliminado (código muerto)
  - BuildGraphInput movido a `schemas.rs` (53 call sites actualizados)
- **Gate CI:**
  - Trybuild macro tests pasando
  - `grep -r "use crate::application::dto" schemas.rs` → cero matches
  - Serde roundtrip tests para los 24 tipos

---

### Wave 4 — C2b: ContextGraphStore Deletion (~50 Δlines)

**Post-requisito:** C2a (Builder) debe estar en producción y los deprecated wrappers deben tener count=0.

- **Archivo:** `handlers/mod.rs`
- **ΔLines:** ~50 (eliminación)
- **Entregable:**
  - `ContextGraphStore` eliminado
  - Todos los 15 call sites migrados a `Arc<dyn GraphStore>`
  - `Box<dyn GraphStore>` → `Arc<dyn GraphStore>` en toda la base
- **Gate CI:** `dead_code` lint clean (ningún warning de código muerto)

---

### Wave 5 — C1: Tool Registry `#[aix_tool]` (~200 Δlines) ⚠️ GATED by C3

**Última wave.** La de mayor impacto en developer workflow.

**Precondiciones (alternativa a esperar C4):**
1. `BuildGraphInput` movido a `schemas.rs`
2. Audit de leakage de DTOs en return types de handlers
3. `schemas.rs` sin imports de `application::dto`

Si las 3 precondiciones se cumplen, C1 puede comenzar antes de C4.

- **Archivos:** `rmcp_adapter.rs` + `cognicode-macros/src/lib.rs`
- **ΔLines:** ~200 + macro
- **Entregable:**
  - Macro attribute `#[aix_tool]` en `cognicode-macros`
  - Registro de herramientas refactorizado para usar la macro
  - ~65+ herramientas registradas via macro
- **Gate CI:**
  - Integration test suite passing
  - Tool count match (mismo número de herramientas antes y después)
  - Benchmark no regressa

---

## 4. CI Gates por Wave

| Wave | Gate | Tool | Pass Criteria |
|------|------|------|---------------|
| 1 | Test suite | `cargo test` | 100% passing |
| 1 | Bench regression | `cargo bench` | <5% regression vs baseline |
| 1 | Mock crate compiles | `cargo build -p cognicode-core-mock` | Zero errors |
| 2 | Coexistence tests | integration test | Both old + new API work |
| 2 | Deprecated count | custom linter | Count = 0 (after migration) |
| 3 | Trybuild tests | `cargo test` (trybuild) | All snapshots passing |
| 3 | DTO boundary | `grep` | Zero `application::dto` imports in schemas |
| 3 | Serde roundtrip | unit tests | All 24 types roundtrip |
| 4 | Dead code | `cargo clippy -- -W dead-code` | Zero warnings |
| 5 | Integration | `cargo test --test '*integration*'` | 100% passing |
| 5 | Tool count | count check | Same count pre/post |

---

## 5. ADRs Vinculados

| ADR | Candidato | Wave | Estado |
|-----|----------|------|--------|
| ADR-001 | Schema/DTO Boundary | 3, 5 | PROPOSED |
| ADR-002 | 5-Wave Execution Order | Todas | PROPOSED |
| ADR-003 | Newtype Macro | 3 | PROPOSED |
| ADR-004 | WalkFilter | 1 | PROPOSED |
| ADR-005 | ReadMode Static Dispatch | 1 | PROPOSED |
| ADR-006 | Mock Crate Separation | 1 | PROPOSED |

---

## 6. Dependencias

```
C4 ──────────────────────► C1
     (gating, salvo 3 precondiciones)

C2a ──► C2b
  (C2b requiere C2a en prod)

C3 ──┬──► Wave 1 (paralelo, sin dependencias entre sí)
C5 ──┘
C6 ──┘
```

---

## 7. Riesgos y Mitigaciones

| Riesgo | Severidad | Probabilidad | Mitigación |
|--------|-----------|-------------|------------|
| C1 bloqueado indefinidamente por C4 | Media | Baja | 3-precondition escape hatch |
| C4 rompe serde roundtrip | **Alta** | Baja | trybuild tests + roundtrip tests obligatorios |
| Merge conflicts en `handlers/mod.rs` | Media | Baja | C2a (Wave 2) y C2b (Wave 4) no se solapan |
| Mock crate version skew | Baja | Media | Lockstep versioning en CI |
| Benchmark regression >5% | Media | Baja | Gate CI hard-stop |

---

## 8. Criteria de Éxito

- [ ] Los 6 candidatos implementados sin romper tests existentes
- [ ] Bench regression <5% en todas las waves
- [ ] Cero imports de `application::dto` en `schemas.rs` post-Wave 3
- [ ] Los 24 tipos schema/DTO compilan con la macro `#[newtype]`
- [ ] `ContextGraphStore` eliminado y todos los call sites migrados
- [ ] Tool registry usa `#[aix_tool]` macro con count unchanged
- [ ] ADR-001 a ADR-006 promoted a ACCEPTED

---

## 9. Timeline Sugerido

```
Junio 2026
├── Semana 1: Wave 1 — C3 (WalkFilter) + C5 (ReadMode) + C6 (Mock crate)
├── Semana 2: Wave 2 — C2a (HandlerContext Builder)
├── Semana 3: Wave 3 — C4 (Schema/DTO Unification) ← CRÍTICO
├── Semana 4: Wave 4 — C2b (ContextGraphStore Deletion)
│
Julio 2026
└── Semana 5: Wave 5 — C1 (Tool Registry)
```

---

## 10. Artefactos del Proceso

| Artefacto | Ubicación |
|-----------|-----------|
| Auto-grill report | `docs/grill/2026-06-11-architecture-deepening.report.md` |
| ADR-001 Schema/DTO Boundary | `docs/adr/ADR-001-schema-dto-boundary.md` |
| ADR-002 5-Wave Execution | `docs/adr/ADR-002-5-wave-execution-order.md` |
| ADR-003 Newtype Macro | `docs/adr/ADR-003-newtype-macro.md` |
| ADR-004 WalkFilter | `docs/adr/ADR-004-walk-filter.md` |
| ADR-005 ReadMode | `docs/adr/ADR-005-readmode-static-dispatch.md` |
| ADR-006 Mock Crate | `docs/adr/ADR-006-mock-crate-separation.md` |
| Drafts (superseded) | `docs/adr/drafts/DRAFT-*.md` (verificar tras promoción) |
| Architecture review HTML | `/tmp/architecture-review-cognicode-2026-06-11.html` |

---

*Documento generado via auto-grill-loop. Para contexto completo, ver el report en `docs/grill/2026-06-11-architecture-deepening.report.md`.*
