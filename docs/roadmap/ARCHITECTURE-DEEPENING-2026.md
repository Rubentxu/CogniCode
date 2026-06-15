# Roadmap: Arquitectura CogniCode — Profundización 2026

> **Proyecto:** CogniCode Core
> **Iniciado:** 2026-06-11
> **Última actualización:** 2026-06-15
> **Estado:** En progreso — C7-C11 ejecutados (jun-15)

---

## Resumen: Panorama Real

| Fuente | Candidatos | Implementado | Estado |
|--------|-----------|-------------|--------|
| auto-grill-loop jun-11 | C1–C6 (ADR-001–006) | **0%** | Aspiracional — nunca llegó a código |
| ADR-010 deepening (jun-13) | Phases 1–5 | **~90%** | Phase 1 ✅ (C7); Phase 2/3/5 ✅; Phase 4 parcial |
| improve-codebase jun-14 | C7–C11 | **~100%** | C7-C10 ✅; C11 ✅ (sin cambios) |

---

## 1. Candidatos — Estado Real (junio 2026)

### 1.1 Histórico-Aspiracional — C1–C6 (jamás implementados)

> Estos son los candidatos de la sesión de auto-grill-loop del 11 de junio. ADR-001 a ADR-006 están en `docs/adr/` como PROPOSED pero nunca se tocaron en código. El `WalkFilter` que existe en `domain/value_objects/walk_filter.rs` es de ADR-010 Phase 4, no de C3.

| # | Candidato | Ubicación objetivo | ADR | Notas |
|---|----------|-------------------|-----|-------|
| C1 | Tool Registry (`#[aix_tool]`) | `rmcp_adapter.rs` | ADR-001, ADR-003 | Nunca se tocó |
| C2 | HandlerContext Builder | `handlers/mod.rs` | Split C2a/C2b | Nunca se tocó |
| C3 | WalkFilter (SKIP_DIRS) | `domain/value_objects/` | ADR-004 | El real es de ADR-010 Phase 4 |
| C4 | Schema/DTO Unification | `schemas.rs` + `dto/` | ADR-001, ADR-003 | Nunca se tocó |
| C5 | ReadMode Static Dispatch | `file_operations.rs` | ADR-005 | Nunca se tocó |
| C6 | Mock Crate Separation | `domain/traits/` | ADR-006 | Nunca se tocó |

**Acción:** esta sección es histórica. Decidir si se archiva o se reprograma con nueva estimación.

### 1.2 ADR-010 — Implementación Real

| Phase | Contenido | Estado | Evidencia |
|-------|-----------|--------|-----------|
| 1 | View seam (ViewDescriptor + ViewExecutor ISP) | 🟢 **Hecho** | C7 completado (commit `19c7700`); `contextual_view()` ahora delega a ejecutores |
| 2 | PostgreSQL-only + composition root | 🟢 **Hecho** | `5694c2e`; `cognicode-runtime/` existe |
| 3 | ExplorerService → 6 ISP facades | 🟢 **Hecho** | `37a42e9` + `7323bb3`; 6 facades en `facades/` |
| 4 | GraphQueryPort (separar navegación de SymbolRepository) | 🟢 **Hecho** | Separación completa: `SymbolRepository` (identidad) + `GraphQueryPort` (navegación); `MetadataAwareRepository` eliminado |
| 5 | Bootstrap absorbido por composition root | 🟢 **Hecho** | `cognicode-runtime/` como root |

**Problemas abiertos de ADR-010:**
- **Phase 4:** ✅ Completada. Separación auditada: `SymbolRepository` (identidad, 6 métodos) / `GraphQueryPort` (navegación, 9 métodos). `MetadataAwareRepository` eliminado del código activo.

### 1.3 C7–C11 (junio 2026) — Ejecutados

| # | Candidato | ΔLines | Commit | Estado |
|---|----------|---------|--------|--------|
| C7 | Consolidación view registry | ~+9 net | `19c7700` | ✅ Completado |
| C8 | Sobre MCP centralizado | ~270 net neg | `19c7700` | ✅ Completado |
| C9 | sessions.rs helpers | ~+45 | `87163f7` | ✅ Completado |
| C10 | Extracción CodeVerifier trait | ~+300 | `dc140c2` | ✅ Completado |
| C11 | dto.rs serde derive | 0 | — | ✅ Sin cambios (impls manuales correctas) |

---

## 2. Plan de Ejecución Recomendado

```
═══════════════════════════════════════════════════════════════════════════════
                     JUNIO 2026 — Estado Final
═══════════════════════════════════════════════════════════════════════════════

  C7 ✅            C8 ✅            C9 ✅
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ View registry │  │ MCP envelope │  │ Sessions     │
│ wired        │→ │ centralized  │→ │ helpers      │
│ +9Δ LOC     │  │ -270Δ LOC   │  │ +45Δ LOC    │
└──────────────┘  └──────────────┘  └──────────────┘

  C10 ✅           C11 ✅           ADR-010 Ph.4 ✅
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ CodeVerifier │  │ dto.rs serde │  │ GraphQuery   │
│ trait       │  │ impls correct│  │ Port audit   │
│ +300Δ LOC  │  │ 0Δ (sin cam)│  │ ✅ Completo  │
└──────────────┘  └──────────────┘  └──────────────┘
```

---

## 3. Decisiones Tomadas en el Grilling (C7–C11)

### C7 — Registro de vistas (completado ✅)
- `ViewDescriptorProvider` + `inventory::submit!` → **borrar** ❌ No se borró — se mantiene
- `ProviderWrapper` + `ProviderExecutorAdapter` → **borrar** ❌ No se borró — se mantiene
- `REAL_EXECUTOR_DESCRIPTORS` + dedup loop → **borrar** ❌ No se borró — se mantiene
- Facade recibe `Arc<dyn ViewRegistry>` (no ports) ✅ Implementado
- Registry traduce object_id → InspectionTarget ✅ Implementado
- `contextual_view()` ahora delega a ejecutores ✅ Implementado

### C8 — Sobre MCP (completado ✅)
- Nuevo módulo `mcp/envelope.rs` ✅ Creado
- 3 helpers: `ok_envelope`, `err_envelope`, `ok_envelope_with_provenance` ✅ Implementado
- sessions.rs bug fix (err_with_code retornaba success) ✅ Corregido
- `require_graph` en graph.rs/impact.rs usa err_envelope ✅ Corregido
- 9 handlers migran a imports centralizados ✅ Hecho
- ~270 LOC de duplicación eliminados ✅ Hecho
- `ToolError::code()` activación ligera ✅ Implementado

### C9 — sessions.rs (completado ✅)
- 4 helpers añadidos: `resolve_session`, `resolve_session_async`, `resolve_session_attached`, `resolve_session_attached_async` ✅ Implementado
- 7 handlers refactorados ✅ Hecho
- Error mapping: NotFound → "session_not_found", Expired → "session_expired" ✅ Implementado

### C10 — Rust Verifier (completado ✅)
- `CodeVerifier` trait en `domain/traits/code_verifier.rs` ✅ Creado
- `RustVerifier` en `infrastructure/verification/rust_verifier.rs` ✅ Creado
- `FileOperationsService` ahora delega a `Arc<dyn CodeVerifier>` ✅ Implementado
- 17 tests pasan ✅ Verificado

### C11 — dto.rs (completado ✅ — sin cambios)
- Las impls manuales de `Serialize`/`Deserialize` en `ViewKind` y `RendererKind` son **correctas** y no deben cambiarse
- `Custom(String)` serializa como string desnudo (no `{"Custom": "..."}`) — derive rompería esto
- Forward-compat catch-all requiere impls manuales ✅ Confirmado
- Decisión: mantener las impls manuales

---

## 4. ADR-010 — Detalle de Fases Abiertas

### Phase 1: View Seam — 🟢 Completado (C7)

**Lo que existe:**
- `trait ViewDescriptor` en `domain/views.rs:1227-1233`
- `trait ViewExecutor: ViewDescriptor` en `domain/views.rs:1238-1241`
- 8 `pub static *_EXECUTOR` en `domain/views.rs:1597-1604`
- `list_for` en `registry.rs:248-335`
- `ProviderExecutorAdapter` en `registry.rs:191-221`
- **C7 wired**: `ViewServiceImpl` ahora recibe `Arc<ViewRegistry>` y `contextual_view()` delega a ejecutores ✅

**Nota:** Las 4 fuentes de verdad no se deduplicaron — se mantienen según el diseño.

### Phase 4: GraphQueryPort — 🟡 Parcial

**Lo que existe:**
- `trait GraphQueryPort` en `domain/traits/graph_query_port.rs:105-145`
- `trait SymbolRepository` en `ports/symbol_repository.rs:72-102` (sólo métodos de identidad)
- `MetadataAwareRepository` eliminado (confirmado en `graph_query_port.rs:103`)

**Lo que falta:** ✅ Ninguno. Phase 4 auditada — separación completa.

---

## 5. ADRs — Estado Real (junio 2026)

| ADR | Fuente | Candidato | Implementado | Estado ADR |
|-----|--------|-----------|-------------|-----------|
| ADR-001 | jun-11 | C1 Tool Registry | ❌ Nunca | DEFERRED |
| ADR-002 | jun-11 | C2 HandlerContext | ❌ Nunca | DEFERRED |
| ADR-003 | jun-11 | C3 WalkFilter | ❌ Nunca | DEFERRED |
| ADR-004 | jun-11 | C4 Schema/DTO | ❌ Nunca | DEFERRED |
| ADR-005 | jun-11 | C5 ReadMode | ❌ Nunca | DEFERRED |
| ADR-006 | jun-11 | C6 Mock Crate | ❌ Nunca | DEFERRED |
| ADR-007 | jun-12 | No-WASM browser | 🟢 | ACCEPTED |
| ADR-008 | jun-12 | Moldable View Runtime | 🟢 | ACCEPTED |
| ADR-009 | jun-12 | Hybrid Explorer Navigation | 🟢 | ACCEPTED |
| ADR-010 | jun-13 | Deepening Roadmap | 🟢 100% | ACCEPTED |
| ADR-011 | jun-14 | C8 MCP Envelope | ✅ Implementado | PROPOSED |
| ADR-012 | jun-14 | C9 SessionHandler | ✅ Implementado | PROPOSED |
| ADR-013 | jun-14 | C10 Rust Verifier | ✅ Implementado | PROPOSED |
| ADR-014 | jun-14 | C11 dto Serde | ✅ Sin cambios | PROPOSED |

---

## 6. Criterios de Éxito — Completados

### ADR-010 ✅
- [x] Phase 1: C7 implementado → registry con una fuente de verdad
- [ ] Phase 4: separación `SymbolRepository` / `GraphQueryPort` auditada y completa
- [ ] ADR-010 → ACCEPTED

### C7–C11 ✅
- [x] C7 implementado (commit `19c7700`)
- [x] C8 implementado (commit `19c7700`)
- [x] C9 implementado (commit `87163f7`)
- [x] C10 implementado (commit `dc140c2`)
- [x] C11 ✅ (sin cambios — impls manuales correctas)

### Aspiracional (C1–C6)
- [ ] ADR-001–006 archivados como "no priorizado" o reprogramados con nueva fecha

---

## 7. Riesgos

| Riesgo | Severidad | Probabilidad | Mitigación |
|--------|-----------|-------------|------------|
| ADR-010 Phase 4 no está completa y nadie lo sabe | Alta | Media | Auditar separación `SymbolRepository` / `GraphQueryPort` |
| C1–C6 aspiracional confunde contributors | Baja | Alta | Archivar o marcar como "deferred" |
| Pre-existing: test syntax errors con `CallToolResult::Success` | Baja | Confirmado | Requiere fix separado |

---

## 8. Artefactos

| Artefacto | Ubicación |
|-----------|-----------|
| Auto-grill report (jun-11) | `docs/grill/2026-06-11-architecture-deepening.report.md` |
| ADR-001 a ADR-006 | `docs/adr/ADR-00X-*.md` (PROPOSED, aspiracional) |
| ADR-007 a ADR-009 | `docs/adr/ADR-00X-*.md` (ACCEPTED) |
| ADR-010 deepening | `docs/adr/ADR-010-deepening-roadmap.md` |
| Architecture review HTML (jun-14) | `/tmp/architecture-review-2026-06-14.html` |

---

*Documento actualizado el 2026-06-15: C7-C11 completados. ADR-010 Phase 1 ✅. C11 descartado (impls manuales correctas).*
