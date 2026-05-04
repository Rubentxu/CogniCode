# CogniCode Dashboard — Roadmap

> **Status**: 📋 Planning  
> **Start**: Q2 2026  
> **Target**: MVP in 2 weeks

---

## Vision

> *Un dashboard que transforma datos de calidad de código en insights accionables, con la elegancia visual de Monday.com y la profundidad analítica de SonarQube.*

---

## Fases

```
Phase 1          Phase 2          Phase 3          Phase 4
[Scaffolding] → [Core UI]     → [Data+Pages]  → [Polish]
   2 días          4 días           5 días          3 días

MVP = Phase 1 + Phase 2 + Phase 3 + Phase 4 = 14 días
```

---

## 📅 Fase 1 — Scaffolding (Día 1-2)

### Objetivo: Proyecto compilando con layout base

| Día | Tarea | Status |
|-----|-------|--------|
| 1 | Crear `crates/cognicode-dashboard/` | ⬜ |
| 1 | Configurar `Cargo.toml` con dependencias (leptos, leptos_router, cognicode-*) | ⬜ |
| 1 | Configurar Tailwind v4 en `style/main.css` con `@theme` tokens | ⬜ |
| 1 | Crear `index.html` entry point | ⬜ |
| 2 | Crear `App` component con `<Router>` y rutas vacías | ⬜ |
| 2 | Crear `Shell` layout (sidebar 64px + main content) | ⬜ |
| 2 | Crear `Sidebar` con nav items (Dashboard, Issues, Metrics, Quality Gate) | ⬜ |
| 2 | Verificar `trunk serve` funciona | ⬜ |

**Deliverable**: Layout vacío navegable con sidebar, compilando en WASM.

---

## 📅 Fase 2 — Core Components (Día 3-6)

### Objetivo: Componentes visuales reutilizables implementados

| Día | Tarea | Status |
|-----|-------|--------|
| 3 | Crear `RatingCard` component (A-E con colores SonarQube) | ⬜ |
| 3 | Crear `MetricCard` component (icono + valor + tendencia) | ⬜ |
| 4 | Crear `SeverityBadge` component (BLOCKER/CRITICAL/MAJOR/MINOR/INFO) | ⬜ |
| 4 | Crear `GateStatusBar` component (barra verde/roja con PASSED/FAILED) | ⬜ |
| 5 | Crear `IssueRow` component (severity badge + message + file + line) | ⬜ |
| 5 | Crear `FilterBar` component (selects + search input) | ⬜ |
| 6 | Crear `LoadingSpinner` y `ErrorBoundary` wrappers | ⬜ |
| 6 | Test visual de todos los componentes con datos mock | ⬜ |

**Deliverable**: Storybook-like page con todos los componentes visibles.

---

## 📅 Fase 3 — Data & Pages (Día 7-11)

### Objetivo: Pantallas completas conectadas a CogniCode

| Día | Tarea | Status |
|-----|-------|--------|
| 7 | Implementar `api/analysis.rs` — `get_project_analysis()` server function | ⬜ |
| 7 | Crear `AppState` global con `RwSignal` | ⬜ |
| 8 | Implementar **Dashboard Page** (`/`) con RatingCards + GateStatus + RecentIssues | ⬜ |
| 8 | Implementar `api/issues.rs` — `get_issues()` con filtros | ⬜ |
| 9 | Implementar **Issues Page** (`/issues`) con IssueTable + FilterBar + Pagination | ⬜ |
| 9 | Implementar **Issue Detail** (`/issues/:id`) con code snippet | ⬜ |
| 10 | Implementar `TrendChart` y `BarChart` SVG components | ⬜ |
| 10 | Implementar **Metrics Page** (`/metrics`) con charts | ⬜ |
| 11 | Implementar `api/quality_gate.rs` — `get_quality_gate_status()` | ⬜ |
| 11 | Implementar **Quality Gate Page** (`/quality-gate`) con condiciones | ⬜ |

**Deliverable**: MVP funcional — dashboard, issues, metrics, quality gate.

---

## 📅 Fase 4 — Polish (Día 12-14)

### Objetivo: MVP pulido para producción

| Día | Tarea | Status |
|-----|-------|--------|
| 12 | Responsive design (mobile sidebar collapse) | ⬜ |
| 12 | Dark/light mode toggle con CSS variables | ⬜ |
| 13 | **Configuration Page** (`/configuration`) con settings | ⬜ |
| 13 | Error handling robusto + retry en server functions | ⬜ |
| 14 | Optimización: `opt-level = "s"`, LTO, bundle size check | ⬜ |
| 14 | Documentación (`README.md` en el crate) | ⬜ |

**Deliverable**: MVP completo listo para demo.

---

## 🚀 Post-MVP (v1.1+)

| Feature | Prioridad | Esfuerzo |
|---------|-----------|----------|
| Hot reload al cambiar archivos | HIGH | 3d |
| Historical trends (SQLite queries) | HIGH | 4d |
| Rule documentation viewer | MEDIUM | 2d |
| Export reports (PDF/JSON) | MEDIUM | 3d |
| Multi-project support | MEDIUM | 5d |
| Real-time analysis progress | LOW | 3d |
| Notification system | LOW | 4d |
| Team collaboration | LOW | 5d |

---

## 📊 Tracking

| Fase | Estado | Días | Progreso |
|------|--------|------|----------|
| Phase 1 — Scaffolding | ⬜ Planning | 2 | 0% |
| Phase 2 — Core UI | ⬜ Planning | 4 | 0% |
| Phase 3 — Data+Pages | ⬜ Planning | 5 | 0% |
| Phase 4 — Polish | ⬜ Planning | 3 | 0% |
| **TOTAL** | | **14** | **0%** |

---

## 🔗 Dependencias

```
Scaffolding → Core UI → Data+Pages → Polish
     │            │           │
     │            │           └── cognicode-quality API
     │            └── DESIGN.md tokens
     └── Tailwind v4 + Leptos 0.7
```

---

## 📝 Convenciones

- **Commits**: `feat(dashboard): mensaje` / `fix(dashboard): mensaje`
- **Branches**: `feat/dashboard-{fase}` desde `main`
- **Testing**: Cada componente con `#[cfg(test)] mod tests`
- **Docs**: Documentar props de componentes con `///` doc comments
