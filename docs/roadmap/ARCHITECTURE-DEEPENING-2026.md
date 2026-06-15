# Roadmap: Arquitectura CogniCode — Profundización 2026

> **Proyecto:** CogniCode Core
> **Iniciado:** 2026-06-11
> **Estado:** Requiere revisión —混杂了三个来源不同的 roadmap
> **Fuentes:**
> - auto-grill-loop jun-11 → ADR-001 a ADR-006 (C1-C6, aspiracional, nunca implementado)
> - ADR-010 deepening roadmap (jun-13) → parcialmente implementado (~60%)
> - improve-codebase-architecture jun-14 (C7-C11, decisiones tomadas, sin código)

---

## Resumen: Panorama Real

Este documento mezcla **tres hojas de ruta distintas**:

| Fuente | Candidatos | Implementado | Estado |
|--------|-----------|-------------|--------|
| auto-grill-loop jun-11 | C1–C6 (ADR-001–006) | **0%** | Aspiracional — nunca llegó a código |
| ADR-010 deepening (jun-13) | Phases 1–5 | **~60%** | Phase 1 (view seam) 50%; Phase 4 (GraphQueryPort) parcial |
| improve-codebase jun-14 | C7–C11 | **0%** | 5 decisiones tomadas, sin código |

**Problema crítico del documento anterior:** los indicadores 🟢 en C3/C5/C6 decían "Completado" pero eran aspiracionales — significaban "diseñado y listo", no "implementado".

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
| 1 | View seam (ViewDescriptor + ViewExecutor ISP) | 🟡 **Mitad** | traits existen en `domain/views.rs:1227-1604`; `facades/view.rs` tiene hardcoded match + devuelve `FeatureDisabled` |
| 2 | PostgreSQL-only + composition root | 🟢 **Hecho** | `5694c2e`; `cognicode-runtime/` existe |
| 3 | ExplorerService → 6 ISP facades | 🟢 **Hecho** | `37a42e9` + `7323bb3`; 6 facades en `facades/` |
| 4 | GraphQueryPort (separar navegación de SymbolRepository) | 🟡 **Parcial** | `trait GraphQueryPort` existe (`domain/traits/graph_query_port.rs`); `MetadataAwareRepository` eliminado; pero separación no completada |
| 5 | Bootstrap absorbido por composition root | 🟢 **Hecho** | `cognicode-runtime/` como root |

**Problemas abiertos de ADR-010:**
- **Phase 1:** 4 fuentes de verdad en el registro de vistas. `contextual_view()` devuelve `FeatureDisabled`. Esto ES el C7 que grillamos ayer — la continuación directa de `view-seam-consolidation`.
- **Phase 4:** `SymbolRepository` en `ports/symbol_repository.rs` aún no tiene los métodos de navegación completamente separados de `GraphQueryPort`.

### 1.3 C7–C11 (junio 2026) — Decisiones Tomadas, Sin Código

| # | Candidato | ΔLines | Depende | Prioridad |
|---|----------|---------|----------|-----------|
| C7 | Consolidación view registry | ~200 net negative | — | 🔴 Alta |
| C8 | Sobre MCP centralizado | ~150 net negative | — | 🔴 Alta |
| C9 | sessions.rs SessionHandler trait | ~500 net negative | C8 | 🔴 Media |
| C10 | Extracción Rust Verifier | ~500 | — | 🟡 Media |
| C11 | dto.rs serde derive + NamedView | ~380 net negative | — | 🟡 Baja |

---

## 2. Plan de Ejecución Recomendado

```
═══════════════════════════════════════════════════════════════════════════════
                    PRÓXIMAS 3 SEMANAS — CogniCode
═══════════════════════════════════════════════════════════════════════════════

  AHORA              SIGUIENTE            JUNIO FIN
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ C7 (view     │  │ C8 (envelope│  │ C9 (sessions│
│   registry)  │→ │   MCP)      │→ │   trait)     │
│ ~200Δ net    │  │ ~150Δ net   │  │ ~500Δ, req C8│
│ continua     │  │ independiente│  └──────────────┘
│ view-seam    │  └──────────────┘
│ consolidation│
└──────────────┘

PARALELO (junio-julio):
┌──────────────┐  ┌──────────────┐  ┌──────────────────┐
│ ADR-010 Ph.4│  │ C10 (rust    │  │ C11 (dto serde)  │
│ GraphQuery  │  │   verifier)  │  │ ~380Δ net        │
│ Port        │  │ ~500Δ        │  │ mecánico         │
└──────────────┘  └──────────────┘  └──────────────────┘
```

---

## 3. Decisiones Tomadas en el Grilling (C7–C11)

### C7 — Registro de vistas
- `ViewDescriptorProvider` + `inventory::submit!` → **borrar**
- `ProviderWrapper` + `ProviderExecutorAdapter` → **borrar**
- `REAL_EXECUTOR_DESCRIPTORS` + dedup loop → **borrar**
- Facade recibe `Arc<dyn ViewRegistry>` (no ports)
- Registry traduce object_id → InspectionTarget (el que llama pasa ViewContext armado)
- `list_for_with_store` queda en registry — respeta ADR-010
- Registry: `{ spec_store: Option<Arc<dyn ViewSpecStore>> }` (sin estado de ports)

### C8 — Sobre MCP
- Nuevo módulo `mcp/handler/envelope.rs`
- Re-exporta `McpResultEnvelope`, `EnvelopeError`, `ProvenanceMetadata`, `FollowUp` de `explorer.rs`
- 4 helpers: `ok_envelope`, `ok_envelope_prov`, `err_envelope`, `plain_err`
- `McpResultEnvelope` usado por fin

### C9 — sessions.rs
- `make_handler!` macro → **borrar** (declarada, 0 usos)
- `SessionHandler` trait + `handle_dispatch` fn → absorbs 4 impl blocks por handler
- Cada handler: const NAME + typed Args + typed Response + validate + call
- ~1028 LOC → ~520 LOC

### C10 — Rust Verifier
- Cluster 2 de `file_operations.rs` → nuevo `application/services/rust_verifier.rs`
- `trait RustVerifier: Send + Sync` → costura
- `CommandRunnerAdapter` (prod) + `InMemoryCommandRunner` (tests)
- `file_operations.rs` baja a ~1700 LOC

### C11 — dto.rs
- `ViewKind`, `RendererKind`, `HierarchyKind` → `#[derive(Serialize, Deserialize)]` + `#[serde(rename_all = "snake_case", other)]`
- `NamedViewDescriptor` → **borrar** (~170 LOC tests + impl)
- `to_view_spec`, `lens_to_view_kind`, `level_to_inspectable_object_type`, `truncate_description` → **borrar** (~150 LOC)
- `NamedView` se conserva (usado por `postgres_repository.rs`)

---

## 4. ADR-010 — Detalle de Fases Abiertas

### Phase 1: View Seam — 🟡 50% hecho

**Lo que existe:**
- `trait ViewDescriptor` en `domain/views.rs:1227-1233`
- `trait ViewExecutor: ViewDescriptor` en `domain/views.rs:1238-1241`
- 8 `pub static *_EXECUTOR` en `domain/views.rs:1597-1604`
- `list_for` en `registry.rs:248-335` (dedup loop con 4 fuentes)
- `ProviderExecutorAdapter` en `registry.rs:191-221` (devuelve `FeatureDisabled`)

**El problema:** 4 fuentes de verdad + `contextual_view()` no llama a los ejecutores.

**Continuación = C7.**

### Phase 4: GraphQueryPort — 🟡 Parcial

**Lo que existe:**
- `trait GraphQueryPort` en `domain/traits/graph_query_port.rs:105-145`
- `trait SymbolRepository` en `ports/symbol_repository.rs:72-102` (sólo métodos de identidad)
- `MetadataAwareRepository` eliminado (confirmado en `graph_query_port.rs:103`)

**Lo que falta:** verificar que `SymbolRepository` no tiene métodos de navegación mezclados. La separación se empezó pero no seAuditó completamente.

---

## 5. ADRs — Estado Real (junio 2026)

| ADR | Fuente | Candidato | Implementado | Estado ADR |
|-----|--------|-----------|-------------|-----------|
| ADR-001 | jun-11 | C1 Tool Registry | ❌ Nunca | PROPOSED |
| ADR-002 | jun-11 | C2 HandlerContext | ❌ Nunca | PROPOSED |
| ADR-003 | jun-11 | C3 WalkFilter | ❌ Nunca | PROPOSED |
| ADR-004 | jun-11 | C4 Schema/DTO | ❌ Nunca | PROPOSED |
| ADR-005 | jun-11 | C5 ReadMode | ❌ Nunca | PROPOSED |
| ADR-006 | jun-11 | C6 Mock Crate | ❌ Nunca | PROPOSED |
| ADR-007 | jun-12 | No-WASM browser | 🟢 | ACCEPTED |
| ADR-008 | jun-12 | Moldable View Runtime | 🟢 | ACCEPTED |
| ADR-009 | jun-12 | Hybrid Explorer Navigation | 🟢 | ACCEPTED |
| ADR-010 | jun-13 | Deepening Roadmap | 🟡 ~60% | PROPOSED |
| ADR-011 | jun-14 | C8 MCP Envelope | ❌ | PROPOSED |
| ADR-012 | jun-14 | C9 SessionHandler | ❌ | PROPOSED |
| ADR-013 | jun-14 | C10 Rust Verifier | ❌ | PROPOSED |
| ADR-014 | jun-14 | C11 dto Serde | ❌ | PROPOSED |

---

## 6. Criteria de Éxito — Realista

### ADR-010
- [ ] Phase 1: C7 implementado → registry con una fuente de verdad
- [ ] Phase 4: separación `SymbolRepository` / `GraphQueryPort` auditada y completa
- [ ] ADR-010 → ACCEPTED

### C7–C11
- [ ] C7 implementado
- [ ] C8 implementado
- [ ] C9 implementado (depende de C8)
- [ ] C10 implementado
- [ ] C11 implementado

### Aspiracional (C1–C6)
- [ ] ADR-001–006 archivados como "no priorizado" o reprogramados con nueva fecha

---

## 7. Riesgos

| Riesgo | Severidad | Probabilidad | Mitigación |
|--------|-----------|-------------|-----------|
| C7 rompe tests existentes de `available_views` | Media | Media | Tests ya esperan el formato nuevo; la regression test ya existe |
| ADR-010 Phase 4 no está completa y nadie lo sabe | Alta | Media | Auditar separación `SymbolRepository` / `GraphQueryPort` |
| C1–C6 aspiracional confunde contributors | Baja | Alta | Archivar o marcar como "deferred" |
| C9 depende de C8 — si C8 se complica, C9 se retrasa | Baja | Baja | C8 es mecánico (~150 LOC net negative) |

---

## 8. Artefactos

| Artefacto | Ubicación |
|-----------|-----------|
| Auto-grill report (jun-11) | `docs/grill/2026-06-11-architecture-deepening.report.md` |
| ADR-001 a ADR-006 | `docs/adr/ADR-00X-*.md` (PROPOSED, aspiracional) |
| ADR-007 a ADR-009 | `docs/adr/ADR-00X-*.md` (ACCEPTED) |
| ADR-010 deepening | `docs/adr/ADR-010-deepening-roadmap.md` |
| Architecture review HTML (jun-11) | `/tmp/architecture-review-cognicode-2026-06-11.html` |
| Architecture review HTML (jun-14) | `/tmp/architecture-review-2026-06-14.html` |
| Copia en change | `openspec/changes/view-seam-consolidation/reports/architecture-review-2026-06-14.html` |

---

*Documento reescrito el 2026-06-14: limpio de混杂, estados corregidos, C1-C6 marcados como aspiracional, ADR-010 desglosado por fase, C7-C11 integrados.*
