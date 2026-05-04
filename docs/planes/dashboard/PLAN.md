# CogniCode Dashboard — Plan de Implementación

> **Versión**: 1.0  
> **Tech Stack**: Rust + Leptos 0.7 + Tailwind v4 + CogniCode Core

---

## 1. Objetivo

Crear un dashboard web interactivo para CogniCode y CogniCode-Quality, ofreciendo:

- **Project Overview**: Ratings A-E (Reliability, Security, Maintainability), technical debt, quality gate status
- **Issues Browser**: Lista filtrable de issues con severidad, categoría, archivo, regla
- **Metrics Dashboard**: Gráficas de tendencias, distribución de issues, cobertura
- **Quality Gate**: Visualización y configuración de condiciones de calidad
- **Configuration**: Ajustes del proyecto, perfiles de reglas, exclusiones

---

## 2. Arquitectura

### 2.1 Modelo de Integración

**Embedded Library + Leptos CSR (Client-Side Rendering)**

```
┌─────────────────────────────────────────┐
│         cognicode-dashboard             │
│  ┌──────────┐  ┌─────────────────────┐  │
│  │ Leptos   │  │  Server Functions    │  │
│  │ Router   │◄─│  #[server]           │  │
│  │ CSR      │  │  Llamadas directas   │  │
│  └──────────┘  └──────────┬──────────┘  │
│                           │              │
├───────────────────────────┼──────────────┤
│  cognicode-quality ◄──────┤              │
│  cognicode-core    ◄──────┤ in-process   │
│  cognicode-axiom   ◄──────┤              │
└───────────────────────────┴──────────────┘
```

**Ventajas**:
- Zero latency (llamadas in-process, sin HTTP)
- Sin serialización innecesaria (mismos tipos Rust)
- Acceso directo a todos los DTOs y servicios
- CSR: despliegue simple (WASM + HTML estático)

### 2.2 Estructura del Crate

```
crates/cognicode-dashboard/
├── Cargo.toml
├── index.html                    # Entry point HTML
├── style/
│   └── main.css                  # Tailwind v4 + theme tokens
├── src/
│   ├── main.rs                   # hydrate(App)
│   ├── app.rs                    # Router + Layout Shell
│   ├── lib.rs                    # Re-exports
│   │
│   ├── pages/
│   │   ├── mod.rs
│   │   ├── dashboard.rs          # / — Overview
│   │   ├── issues.rs             # /issues — Issues browser
│   │   ├── issue_detail.rs       # /issues/:id — Issue detail
│   │   ├── metrics.rs            # /metrics — Metrics dashboard
│   │   ├── quality_gate.rs       # /quality-gate — Gate status/conf
│   │   ├── configuration.rs      # /configuration — Settings
│   │   └── not_found.rs          # 404
│   │
│   ├── components/
│   │   ├── mod.rs
│   │   ├── shell.rs              # Sidebar + Header + Outlet
│   │   ├── sidebar.rs            # Nav lateral
│   │   ├── header.rs             # Top bar + breadcrumb
│   │   ├── rating_card.rs        # Rating A-E display
│   │   ├── metric_card.rs        # Single metric with trend
│   │   ├── issue_table.rs        # Issues table (filter/sort)
│   │   ├── issue_row.rs          # Single issue row
│   │   ├── severity_badge.rs     # Severity colored badge
│   │   ├── gate_status_bar.rs    # Quality gate pass/fail bar
│   │   ├── gate_condition.rs     # Single condition display
│   │   ├── trend_chart.rs        # SVG sparkline chart
│   │   ├── bar_chart.rs          # SVG bar chart
│   │   ├── filter_bar.rs         # Filter controls
│   │   └── loading_spinner.rs    # Loading state
│   │
│   ├── api/
│   │   ├── mod.rs                # Server functions
│   │   ├── analysis.rs           # analyze_project, get_metrics
│   │   ├── issues.rs             # get_issues, get_issue
│   │   ├── quality_gate.rs       # get_gate, evaluate_gate
│   │   └── configuration.rs      # get/set config
│   │
│   └── state/
│       ├── mod.rs
│       └── app_state.rs          # Global reactive state
```

### 2.3 Cargo.toml

```toml
[package]
name = "cognicode-dashboard"
version = "0.1.0"
edition = "2021"

[dependencies]
leptos = { version = "0.7", features = ["csr"] }
leptos_router = "0.7"
leptos_meta = "0.7"
leptos_use = "0.14"

cognicode-quality = { path = "../cognicode-quality" }
cognicode-core = { path = "../cognicode-core" }
cognicode-axiom = { path = "../cognicode-axiom" }

serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
gloo-timers = "0.3"

[dev-dependencies]
wasm-bindgen-test = "0.3"

[profile.release]
opt-level = "s"
lto = true
```

---

## 3. Rutas y Pantallas

### 3.1 `/` — Dashboard Overview

**Componentes**:
- `QualityGateStatusBar` — Barra verde/roja con PASSED/FAILED
- `RatingCard` × 4 — Reliability, Security, Maintainability, Coverage
- `MetricCard` × 4 — Issues totales, Code Smells, Bugs, Vulnerabilities
- `SeverityBarChart` — Distribución por severidad
- `RecentIssuesTable` — Últimos 10 issues

**Data Flow**:
```
get_project_analysis() → ProjectAnalysis → signals → componentes
```

### 3.2 `/issues` — Issues Browser

**Componentes**:
- `FilterBar` — Filtros: severity, category, rule, file path, date
- `IssueTable` — Tabla con sort por columna
- `IssueRow` — Fila individual
- `Pagination` — 20 issues por página

**Data Flow**:
```
get_issues(filters) → Vec<IssueResult> → <For/> component
```

### 3.3 `/issues/:id` — Issue Detail

**Componentes**:
- Issue metadata (rule, severity, category)
- Code snippet (highlighted)
- Remediation suggestions
- Related issues

### 3.4 `/metrics` — Metrics Dashboard

**Componentes**:
- `TrendChart` × 3 — Issues over time, Complexity trend, Coverage trend
- `CategoryPieChart` — Distribution by category
- `MetricTable` — File-level metrics breakdown

### 3.5 `/quality-gate` — Quality Gate

**Componentes**:
- `GateStatusBar` — Overall pass/fail
- `GateCondition` × N — Each condition with ✅/❌
- Condition editor (add/remove/modify)
- Threshold configuration

### 3.6 `/configuration` — Settings

**Componentes**:
- Project name/path
- Rule profile selection
- Quality gate selection
- Exclusion patterns (glob)
- Analysis schedule

---

## 4. Componentes Detallados

### 4.1 `RatingCard`

```rust
#[component]
fn RatingCard(
    rating: String,       // "A" | "B" | "C" | "D" | "E"
    label: String,        // "Reliability"
    value: Option<String> // Optional numeric value
) -> impl IntoView {
    let bg = match rating.as_str() {
        "A" => "bg-accent-pale",
        "B" => "bg-accent-ocean",
        "C" => "bg-accent-sky",
        "D" => "bg-accent-sunset",
        "E" => "bg-accent-sunset",
        _ => "bg-surface",
    };
    let text_color = format!("text-rating-{}", rating.to_lowercase());

    view! {
        <div class={format!("rounded-xl p-6 text-center {}", bg)}>
            <span class={format!("text-display font-bold {}", text_color)}>
                {rating}
            </span>
            <p class="text-caption text-text-muted mt-2 tracking-wider uppercase">
                {label}
            </p>
            {value.map(|v| view! {
                <p class="text-body-sm text-text-primary mt-1">{v}</p>
            })}
        </div>
    }
}
```

### 4.2 `IssueTable`

```rust
#[component]
fn IssueTable(issues: ReadSignal<Vec<IssueResult>>) -> impl IntoView {
    view! {
        <div class="bg-canvas rounded-xl shadow-card overflow-hidden">
            // Header
            <div class="flex items-center gap-3 px-6 py-4 border-b border-border
                        text-caption font-bold text-text-muted uppercase tracking-wider">
                <span class="w-24">Severity</span>
                <span class="w-20">Rule</span>
                <span class="flex-1">Message</span>
                <span class="w-48">File</span>
                <span class="w-24">Line</span>
            </div>
            // Rows
            <For
                each=move || issues.get()
                key=|issue| (issue.rule_id.clone(), issue.file.clone(), issue.line)
                children=move |issue| {
                    view! { <IssueRow issue=issue.clone() /> }
                }
            />
        </div>
    }
}
```

### 4.3 `TrendChart` (SVG puro)

```rust
#[component]
fn TrendChart(
    data: Vec<f64>,
    width: u32,
    height: u32,
    color: &'static str,
) -> impl IntoView {
    let points = compute_svg_path(&data, width, height);
    let fill = format!("url(#gradient-{})", color);

    view! {
        <svg width=width height=height class="overflow-visible">
            <defs>
                <linearGradient id=format!("gradient-{}", color)
                    x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stop-color=color stop-opacity="0.3"/>
                    <stop offset="100%" stop-color=color stop-opacity="0"/>
                </linearGradient>
            </defs>
            <path d=points fill="none" stroke=color stroke-width="2"/>
            <path d=format!("{} V{} H0 Z", points, height) fill=fill/>
        </svg>
    }
}
```

---

## 5. API / Server Functions

### 5.1 Análisis de Proyecto

```rust
#[server]
pub async fn get_project_analysis(
    project_path: String,
) -> Result<ProjectAnalysisDto, ServerFnError> {
    let handler = QualityAnalysisHandler::new(PathBuf::from(&project_path))
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    let params = AnalyzeProjectParams {
        project_path: PathBuf::from(&project_path),
        quick: false,
        max_duration_secs: Some(120),
        changed_only: true,
    };

    let result = handler.analyze_project_impl(params)
        .map_err(|e| ServerFnError::ServerError(e.to_string()))?;

    // Map to frontend DTOs
    Ok(ProjectAnalysisDto::from(result))
}
```

### 5.2 Issues con Filtros

```rust
#[server]
pub async fn get_issues(
    project_path: String,
    severity: Option<String>,
    category: Option<String>,
    rule_id: Option<String>,
    file_filter: Option<String>,
    page: Option<usize>,
    page_size: Option<usize>,
) -> Result<PaginatedIssuesDto, ServerFnError> {
    // Filter and paginate issues from latest analysis
}
```

### 5.3 Quality Gate

```rust
#[server]
pub async fn get_quality_gate_status(
    project_path: String,
) -> Result<QualityGateResultDto, ServerFnError> {
    // Evaluate quality gate on latest metrics
}

#[server]
pub async fn update_quality_gate(
    project_path: String,
    conditions: Vec<GateConditionDto>,
) -> Result<(), ServerFnError> {
    // Update gate configuration and persist
}
```

---

## 6. Estado Global

### 6.1 AppState

```rust
#[derive(Clone)]
pub struct AppState {
    pub project_path: RwSignal<String>,
    pub analysis: RwSignal<Option<ProjectAnalysisDto>>,
    pub loading: RwSignal<bool>,
    pub error: RwSignal<Option<String>>,
    pub last_updated: RwSignal<Option<DateTime<Utc>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            project_path: RwSignal::new(String::new()),
            analysis: RwSignal::new(None),
            loading: RwSignal::new(false),
            error: RwSignal::new(None),
            last_updated: RwSignal::new(None),
        }
    }

    pub async fn refresh(&self) {
        self.loading.set(true);
        self.error.set(None);

        match get_project_analysis(self.project_path.get()).await {
            Ok(data) => {
                self.analysis.set(Some(data));
                self.last_updated.set(Some(Utc::now()));
            }
            Err(e) => {
                self.error.set(Some(e.to_string()));
            }
        }

        self.loading.set(false);
    }
}
```

---

## 7. Roadmap de Implementación

### Fase 1 — Scaffolding (día 1-2)
- [ ] Crear `crates/cognicode-dashboard/` con Cargo.toml
- [ ] Configurar Tailwind v4 con theme tokens
- [ ] Crear `App` component con `Router`
- [ ] Implementar `Shell` layout (Sidebar + Header)
- [ ] Implementar navegación entre rutas

### Fase 2 — Core Components (día 3-4)
- [ ] `RatingCard` component
- [ ] `MetricCard` component
- [ ] `SeverityBadge` component
- [ ] `GateStatusBar` component
- [ ] `LoadingSpinner` component

### Fase 3 — Dashboard Page (día 5-6)
- [ ] API: `get_project_analysis()`
- [ ] Implementar página `/` con grid layout
- [ ] Integrar RatingCards, MetricCards
- [ ] Integrar GateStatusBar

### Fase 4 — Issues Page (día 7-8)
- [ ] API: `get_issues()` con filtros
- [ ] `IssueTable` + `IssueRow` components
- [ ] `FilterBar` component con severity/category/rule
- [ ] Paginación

### Fase 5 — Metrics & Charts (día 9-10)
- [ ] `TrendChart` SVG component
- [ ] `BarChart` SVG component
- [ ] Dashboard de métricas con charts
- [ ] Tendencia de issues over time

### Fase 6 — Quality Gate (día 11-12)
- [ ] API: `get_quality_gate_status()`
- [ ] `GateCondition` component con ✅/❌
- [ ] Editor de condiciones
- [ ] Configuración de thresholds

### Fase 7 — Polish & Deploy (día 13-14)
- [ ] Responsive design
- [ ] Dark/light mode toggle
- [ ] Error boundaries
- [ ] Loading states
- [ ] Documentation

---

## 8. Riesgos y Mitigaciones

| Riesgo | Impacto | Mitigación |
|--------|---------|------------|
| Análisis lento bloquea UI | Alto | `changed_only: true`, async loading, progress bar |
| Cambios en API de calidad | Medio | Wrapper DTOs internos |
| Tailwind v4 breaking changes | Bajo | Pin version en package.json |
| WASM binary demasiado grande | Medio | `opt-level = "s"`, `lto = true`, code splitting |
| CORS issues en deployment | Bajo | Configurar correctamente en Axum/Actix |
