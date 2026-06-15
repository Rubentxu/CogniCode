# Roadmap: Arquitectura CogniCode — Profundización 2026

> **Proyecto:** CogniCode Core
> **Iniciado:** 2026-06-11
> **Última actualización:** 2026-06-15
> **Estado:** C7-C11 + ADR-010 100%. C1-C6 auditados (jun-15). Brechas gtoolkit documentadas en ADR-016.

---

## Resumen: Estado Real (jun-15)

| Fuente | Candidatos | Estado |
|--------|-----------|--------|
| auto-grill-loop jun-11 | C1–C6 (ADR-001–006) | Auditados — C1, C2, C5, C6 ya implementados; C3 consolidado; C4 → ADR-015 (deuda documentada) |
| ADR-010 deepening (jun-13) | Phases 1–5 | 100% completado y aceptado |
| improve-codebase jun-14 | C7–C11 | 100% completado |

---

## 1. Decisión de C1–C6 (jun-15)

Decisión tomada tras investigación del estado real del código. Cada candidato se evaluó por utilidad, complejidad y riesgo.

### 1.1 C1 — Tool Registry (`#[aix_tool]`)

| | |
|---|---|
| **Decisión** | **ARCHIVADO** |
| **Razón** | El macro `#[cognicode_macros::aix_tool]` existe y se aplica en 65 sitios. El "Tool Registry dinámico" del ADR-001 no se materializó, pero la macro cubre el caso de uso real (anotación estática en handlers). Refactorizar a un registry centralizado es 1-2 semanas con riesgo de regresión alto. |
| **Acción** | Ninguna. |

### 1.2 C2 — HandlerContext Builder

| | |
|---|---|
| **Decisión** | **ARCHIVADO** |
| **Razón** | `HandlerContext` y `HandlerContextBuilder` YA EXISTEN en `handlers/mod.rs:321,524`. Builder pattern completo con 16 campos y métodos `with_*`. ADR-002 decía "Wave 2: C2 Builder" — esa fase ya está implementada. |
| **Acción** | Ninguna. |

### 1.3 C3 — WalkFilter (consolidar SKIP_DIRS)

| | |
|---|---|
| **Decisión** | **COMPLETADO** |
| **Razón** | WalkFilter existía pero no se usaba. Había 5 copias de SKIP_DIRS con divergencias (5-15 entradas cada una). Riesgo de seguridad real (alguien agrega un dir al blocklist en uno y no en los otros). |
| **Acción** | Commit `5f47dd2` (jun-15) — agregada `WalkFilter::matches_any_component()`, reemplazadas las 5 copias (`analysis_service.rs` x2, `workspace_session.rs` x2, `semantic_search.rs`, `lightweight_index.rs`, `handlers/mod.rs`). |

### 1.4 C4 — Schema/DTO Boundary

| | |
|---|---|
| **Decisión** | **DEFERRED — ver ADR-015** |
| **Razón** | `schemas.rs` líneas 11-20 re-exporta 10 DTOs. Eliminar la duplicación es 1-2 semanas de refactor sin beneficiario concreto. Tests de round-trip cubren el riesgo real. |
| **Acción** | ADR-015 creado (jun-15) documenta la deuda explícitamente. Reabrir C4 si surge un caso de divergencia wire vs DTO. |

### 1.5 C5 — ReadMode Static Dispatch

| | |
|---|---|
| **Decisión** | **ARCHIVADO** |
| **Razón** | `ReadMode` ya es un enum cerrado con 4 variantes. Dispatch es `match` exhaustivo en `file_operations.rs:214`. ADR-005 dice exactamente esto y ya está hecho. |
| **Acción** | Ninguna. |

### 1.6 C6 — Mock Crate Separation

| | |
|---|---|
| **Decisión** | **ARCHIVADO** |
| **Razón** | `cognicode-core-mock` existe, v0.5.0 con lockstep versioning. Mocks escritos a mano (cumple ADR-006). Dependencia `mockall` eliminada (jun-15) por ser código muerto. |
| **Acción** | Commit `5f47dd2` — removida `mockall` de `Cargo.toml`. |

---

## 2. ADR-010 — Detalle de Fases (100% completado)

| Phase | Contenido | Estado | Evidencia |
|-------|-----------|--------|-----------|
| 1 | View seam (ViewDescriptor + ViewExecutor ISP) | ✅ | C7 (commit `19c7700`); `contextual_view()` delega a ejecutores |
| 2 | PostgreSQL-only + composition root | ✅ | `5694c2e`; `cognicode-runtime/` existe |
| 3 | ExplorerService → 6 ISP facades | ✅ | `37a42e9` + `7323bb3` |
| 4 | GraphQueryPort (separar navegación) | ✅ | `SymbolRepository` (identidad) + `GraphQueryPort` (navegación); `MetadataAwareRepository` eliminado |
| 5 | Bootstrap absorbido por composition root | ✅ | `cognicode-runtime/` como root |

---

## 3. C7–C11 (junio 2026) — Ejecutados

| # | Candidato | ΔLines | Commit | Estado |
|---|----------|---------|--------|--------|
| C7 | Consolidación view registry | ~+9 net | `19c7700` | ✅ |
| C8 | Sobre MCP centralizado | ~270 net neg | `19c7700` | ✅ |
| C9 | sessions.rs helpers | ~+45 | `87163f7` | ✅ |
| C10 | Extracción CodeVerifier trait | ~+300 | `dc140c2` | ✅ |
| C11 | dto.rs serde derive | 0 | — | ✅ Sin cambios (impls manuales correctas) |

---

## 3.1 Brechas con gtoolkit (jun-15) → ADR-016

Auditoría del estado real contra el modelo de gtoolkit. El vocabulario es compartido pero la implementación diverge en 3 áreas.

| Aspecto | gtoolkit | CogniCode Explorer | Estado |
|---------|----------|-------------------|--------|
| Navegación entre objetos | Pane stack (GtPager) | Column-based (Miller) | ⚠️ Brecha — ADR-016 Fase 2 |
| Historial de exploración | GtPager navigation history | `HistoryEntry` de Ask (no de navegación) | ⚠️ Brecha — ADR-016 Fase 3 |
| Persistencia semántica | Sí, primera clase | Solo sesiones efímeras con TTL | ⚠️ Brecha — ADR-016 Fases 3-4 |
| Moldable views runtime | Sí, maduro | Sí, v1 (sin remote renderers) | ✅ Parcialmente alineado |
| Mocks | Hand-written | Hand-written | ✅ Alineado |
| Spotter | Sí | Sí | ✅ Alineado |
| Lepiter (notebook) | Sí | Diseñado, no v1 | Diferido |

**Decisión (ADR-016):** cerrar las 3 brechas con pane-stack opt-in + `ExplorationEvent` + sharing por URL. Estimación: 3-4 semanas, ~1050-1500 LOC, default = column-based (no regresión para vertical slice tracing).

**Detalle por brecha:**

1. **Navegación column-based** — `apps/explorer-ui/src/state/context.ts` modela state como `columns: ExplorationColumn[]` (lineal) + `activeObjectId: string | null` (un foco). `SELECT_OBJECT` "collapses trailing columns" → comportamiento de reemplazo, no apilamiento. Para drill-down funciona; para exploración amplia (comparar implementaciones) hay que navegar来回 entre columnas.

2. **Historial de exploración** — `crates/cognicode-explorer/src/session/state.rs:28-36` define `HistoryEntry { question, answer_summary, pattern_id, ts }`. Es historial de **preguntas del Ask**, no de navegación entre objetos. La acción `ADD_EXPLORATION` en el frontend está definida pero no se usa en código real (solo en tests/fixtures).

3. **Persistencia semántica** — `crates/cognicode-explorer/src/facades/persistence.rs:26` usa `Mutex<HashMap>` en RAM. `ExplorationPath { columns, objects, lens, created_at }` es UNA lista, no modela pane-stack. No hay sharing por URL ni restore desde link.

**Por qué importa:** `CONTEXT.md` describe pane-stack, persistencia, sharing — el código implementa otra cosa. Un contributor que lea `CONTEXT.md` espera GtPager; encuentra Miller. Costo de onboarding para usuarios familiarizados con gtoolkit.

**Por qué opt-in (no default):** el flujo vertical slice tracing (drill-down) funciona bien con column. Forzar pane-stack a todos los usuarios es regresión de UX para el caso primario. El `NavigationAdapter` (Fase 1) permite coexistencia.

---

## 4. ADRs — Estado Real (jun-15)

| ADR | Fuente | Candidato | Estado ADR | Notas |
|-----|--------|-----------|------------|-------|
| ADR-001 | jun-11 | C1 Tool Registry | ARCHIVED | Macro `#[aix_tool]` cubre el caso |
| ADR-002 | jun-11 | C2 HandlerContext | ARCHIVED | Builder ya existe |
| ADR-003 | jun-11 | C3 WalkFilter (macro) | ARCHIVED | Macro `#[newtype]` existe, lista para C4 |
| ADR-004 | jun-11 | C3 WalkFilter (value object) | ARCHIVED | Consolidación hecha en jun-15 |
| ADR-005 | jun-11 | C5 ReadMode | ARCHIVED | Ya implementado |
| ADR-006 | jun-11 | C6 Mock Crate | ARCHIVED | Ya implementado |
| ADR-007 | jun-12 | No-WASM browser | ACCEPTED | |
| ADR-008 | jun-12 | Moldable View Runtime | ACCEPTED | |
| ADR-009 | jun-12 | Hybrid Explorer Navigation | ACCEPTED | |
| ADR-010 | jun-13 | Deepening Roadmap | ACCEPTED | 100% |
| ADR-015 | jun-15 | C4 Schema/DTO deuda | ACCEPTED (deuda) | Documenta violación aceptada |
| ADR-016 | jun-15 | Alineación con gtoolkit | PROPOSED | Pane-stack + ExplorationEvent + sharing. 3-4 semanas. Ver §3.1 |

**ADR-011 a ADR-014 NO EXISTEN como archivos en `docs/adr/`.** El roadmap anterior los referenciaba como "PROPOSED" pero no fueron creados formalmente. Las decisiones correspondientes (C8 MCP Envelope, C9 SessionHandler, C10 Rust Verifier, C11 dto Serde) viven en sus commits. No se crean retroactivamente — los commits son la documentación.

---

## 5. Criterios de Éxito — Completados

### ADR-010 ✅
- [x] Phase 1: C7 implementado
- [x] Phase 2-5: PostgreSQL-only, facades, GraphQueryPort, composition root
- [x] ADR-010 → ACCEPTED

### C7–C11 ✅
- [x] C7, C8, C9, C10, C11 (todos los commits referenciados)

### C1–C6 (jun-15)
- [x] C1, C2, C3, C5, C6 → ARCHIVADO
- [x] C3 consolidación hecha
- [x] C4 → ADR-015 con deuda documentada
- [x] ADR-001-006 actualizados a ARCHIVED en tabla

### Alineación con gtoolkit (jun-15) — ADR-016 PROPOSED
- [ ] Fase 1: `NavigationAdapter` interface + refactor `ColumnNavigation` (3-4 días)
- [ ] Fase 2: Pane-stack end-to-end con viewport handling (1-2 semanas)
- [ ] Fase 3: `ExplorationEvent` + persistencia semántica (1 semana)
- [ ] Fase 4: Sharing por URL + restore (3-5 días)

---

## 6. Riesgos Cerrados (jun-15)

| Riesgo | Estado |
|--------|--------|
| ADR-010 Phase 4 no auditada | Cerrado — separación SymbolRepository/GraphQueryPort completa |
| C1-C6 confunde contributors | Cerrado — tabla actualizada con estado real |
| SKIP_DIRS duplicado en 5 sitios | Cerrado — WalkFilter consolidado |
| `mockall` dependencia muerta | Cerrado — removido de Cargo.toml |
| Brechas con gtoolkit no documentadas | Cerrado — ADR-016 + tabla §3.1 |

---

## 7. Riesgos Abiertos

| Riesgo | Severidad | Mitigación |
|--------|-----------|------------|
| Schema/DTO violación (10 re-exports) | Baja | ADR-015, tests de round-trip |
| 22 tests `#[ignore]` (flaky verification + CI pre-existing) | Baja | Sin acción inmediata — son ruidosos pero no bloquean |
| Brecha de navegación con gtoolkit | Media | ADR-016, pane-stack opt-in |
| Brecha de persistencia semántica | Media | ADR-016, `ExplorationEvent` + sharing |
| `CONTEXT.md` describe visión no implementada | Media | ADR-016 documenta el gap explícitamente |

---

## 8. Artefactos

| Artefacto | Ubicación |
|-----------|-----------|
| Auto-grill report (jun-11) | `docs/grill/2026-06-11-architecture-deepening.report.md` |
| ADR-001-006 (archivo histórico) | `docs/adr/ADR-00X-*.md` (PROPOSED — no se borraron por valor histórico) |
| ADR-007-010 | `docs/adr/ADR-00X-*.md` (ACCEPTED) |
| ADR-015 (deuda schema/DTO) | `docs/adr/ADR-015-schema-dto-debt.md` |
| ADR-016 (alineación gtoolkit) | `docs/adr/ADR-016-gtoolkit-alignment.md` |

---

*Documento actualizado el 2026-06-15: C1-C6 auditados y decididos. C3 consolidado. ADR-015 creado para la única deuda restante.*
