# CogniCode Dashboard — Manual de Uso

> Versión 1.0 | Mayo 2026 | CogniCode Team

---

## Índice

1. [¿Qué es CogniCode Dashboard?](#1-qué-es-cognicode-dashboard)
2. [Instalación y arranque](#2-instalación-y-arranque)
3. [Arquitectura](#3-arquitectura)
4. [Pantallas y funcionalidad](#4-pantallas-y-funcionalidad)
5. [Gestión de proyectos](#5-gestión-de-proyectos)
6. [Integración con SDD](#6-integración-con-sdd-agent-teams)
7. [Agents, Skills y Prompts](#7-agents-skills-y-prompts)
8. [Tests E2E](#8-tests-e2e)
9. [Referencia API](#9-referencia-api)
10. [FAQ](#10-faq)

---

## 1. ¿Qué es CogniCode Dashboard?

**CogniCode Dashboard** es una interfaz web (WebAssembly + Rust) que permite
visualizar, gestionar y analizar la calidad de código de múltiples proyectos
que usan CogniCode.

Es la **capa UI** del ecosistema CogniCode. No duplica funcionalidad:
- **Lee** directamente de las bases de datos SQLite que CogniCode ya genera
  (`.cognicode/cognicode.db`) en cada proyecto.
- **Indexa** múltiples proyectos en una sola vista.
- **Visualiza** tendencias históricas desde `analysis_runs`.
- **Gestiona** issues con seguimiento (open → fixed).

### Características principales

| Feature | Descripción |
|---------|-------------|
| 🏠 Dashboard | Overview: ratings A-E, technical debt, quality gate status |
| 📂 Projects | Registro multi-proyecto estilo SonarQube |
| 🐛 Issues | Browser de issues con filtros (severidad, categoría, archivo) y paginación |
| 📊 Metrics | Gráficos de tendencias SVG, distribución por severidad, Clean as You Code |
| 🚦 Quality Gate | Visualización y edición de condiciones de calidad |
| ⚙️ Configuration | Ajustes del proyecto, perfiles de reglas, ruta |
| 🌓 Dark Mode | Toggle light/dark con persistencia visual |
| 📱 Responsive | Sidebar colapsable en móvil (< 768px) |

---

## 2. Instalación y arranque

### Requisitos

- **Rust** 1.80+ con target `wasm32-unknown-unknown`
- **Trunk** 0.21+ (`cargo install trunk`)
- **Node.js** 20+ (solo para tests e2e)
- Un proyecto analizado con CogniCode (que tenga `.cognicode/cognicode.db`)

### Instalación rápida

```bash
# 1. Clonar el repo
cd CogniCode

# 2. Instalar WASM target
rustup target add wasm32-unknown-unknown

# 3. Construir frontend WASM
cd crates/cognicode-dashboard
trunk build --no-default-features
cp -r style dist/style/

# 4. Construir servidor
cargo build --bin cognicode-dashboard-server

# 5. Arrancar
DIST_DIR=dist cargo run --bin cognicode-dashboard-server
```

El servidor estará en `http://localhost:3000`.

### Variables de entorno

| Variable | Default | Descripción |
|----------|---------|-------------|
| `DIST_DIR` | `dist` | Directorio con los archivos WASM compilados |
| `PORT` | `3000` | Puerto del servidor |
| `RUST_LOG` | `info` | Nivel de logging |

### Verificación

```bash
curl http://localhost:3000/health
# Debe devolver: OK

curl http://localhost:3000/ | head -3
# Debe devolver: <!DOCTYPE html>...
```

---

## 3. Arquitectura

```
┌──────────────────────────────────────────────────────────┐
│                   cognicode-dashboard                     │
│                                                          │
│  ┌──────────────────┐      ┌───────────────────────────┐ │
│  │  Frontend WASM    │      │  Server (Axum)            │ │
│  │  (Leptos 0.8 CSR) │◄────►│  Puerto 3000              │ │
│  │  Trunk build       │ HTTP │  API REST + Static Files │ │
│  └──────────────────┘      └───────────┬───────────────┘ │
│                                        │                  │
│                    ┌───────────────────┴───────────────┐  │
│                    │  cognicode-db::QualityStore        │  │
│                    │  (SOLO LECTURA)                    │  │
│                    └───────────┬───────────────────────┘  │
│                                │                          │
│  ┌─────────────────────────────▼───────────────────────┐  │
│  │          .cognicode/cognicode.db (SQLite)            │  │
│  │  ┌──────────────┐ ┌──────────┐ ┌──────────────────┐ │  │
│  │  │analysis_runs │ │ issues   │ │ baselines        │ │  │
│  │  │(historial)   │ │(tracking)│ │ (comparación)    │ │  │
│  │  └──────────────┘ └──────────┘ └──────────────────┘ │  │
│  └─────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘

El dashboard NO crea bases de datos nuevas.
Lee de las BBDD que CogniCode ya genera en cada proyecto.
```

### Stack técnico

| Capa | Tecnología |
|------|-----------|
| Frontend | Leptos 0.8 (CSR), Rust → WASM |
| CSS | CSS custom properties (Refero/Monday.com tokens) |
| Server | Axum 0.7, Tokio, Tower-HTTP |
| Persistencia | SQLite via `cognicode-db` (rusqlite) |
| UI Toolkit | Componentes propios (RatingCard, SeverityBadge, TrendChart...) |
| Tests | Playwright 1.52, @playwright/test |

---

## 4. Pantallas y funcionalidad

### 4.1 Dashboard (`/`)

![Dashboard](images/01-dashboard.png)

Panel principal con:
- **Project Path Input**: Ruta del proyecto a analizar
- **Run Analysis**: Lanza un análisis de calidad (usa `cognicode-quality` in-process)
- **Project Ratings**: Tarjetas A-E para Reliability, Security, Maintainability, Coverage
- **Metrics Grid**: Issues totales, Code Smells, Bugs, Vulnerabilities
- **Technical Debt**: Tiempo estimado de remediación
- **Recent Issues**: Últimos issues encontrados
- **Gate Status**: Barra verde/roja con PASSED/FAILED

### 4.2 Projects (`/projects`)

![Projects](images/02-projects.png)

Lista de proyectos estilo SonarQube:
- **Rating badge** (A-E) con color
- **Gate status** (PASSED/FAILED)
- **Métricas**: Issues, Debt, Files Changed, History Runs
- **Último análisis**: Timestamp del último `analysis_run`

**Registrar un proyecto nuevo:**

![Register Form](images/03-register-form.png)

1. Click en **"+ Add Project"**
2. Introduce **nombre** y **ruta absoluta** del proyecto
3. El dashboard detecta automáticamente si existe `.cognicode/cognicode.db`
4. Click en **Register**

El proyecto debe haber sido analizado previamente con CogniCode para mostrar datos.

### 4.3 Issues (`/issues`)

![Issues](images/04-issues.png)

Browser de issues con:
- **Filtros**: Severidad (Blocker→Info), Categoría (Reliability, Security...), búsqueda por archivo
- **Botón Apply**: Aplica los filtros seleccionados
- **Paginación**: Prev/Next, "Showing X of Y issues"
- **Issue Table**: Regla, mensaje, archivo, línea
- **Click en issue** → navega a `/issues/:id` (detalle)

### 4.4 Metrics (`/metrics`)

![Metrics](images/05-metrics.png)

Dashboard de métricas:
- **Overview**: Lines of Code, Functions, Code Smells, Technical Debt
- **Trend Charts**: Gráficos SVG de tendencia para Issues, Code Smells, Bugs
- **Severity Distribution**: Barras de porcentaje por severidad
- **Incremental Analysis**: Files total/changed/reused
- **Clean as You Code**: Indicador de blockers en código nuevo

### 4.5 Quality Gate (`/quality-gate`)

![Quality Gate](images/06-quality-gate.png)

Visualización del quality gate:
- **Gate Status Bar**: PASSED/FAILED con color
- **Conditions Table**: Status, Condition, Metric, Operator, Threshold
- **Edit Conditions**: Modo edición con botón Remove por condición

![Edit Conditions](images/07-edit-conditions.png)

Modo edición:
- **Remove**: Elimina una condición (UI, requiere API server-side para persistir)
- **Add Condition**: Formulario con métrica, operador y umbral
- **Gate Summary**: Total conditions, passing count

### 4.6 Configuration (`/configuration`)

![Configuration](images/08-configuration.png)

Ajustes del proyecto:
- **Project Path**: Ruta editable del proyecto
- **Run Analysis**: Dispara un análisis completo desde configuración

### 4.7 Dark Mode

![Dark Mode](images/09-dark-mode.png)

Toggle en el footer del sidebar:
- **Dark Mode** → cambia a tema oscuro
- **Light Mode** → vuelve al tema claro
- Persistencia visual inmediata (variables CSS reactivas)

### 4.8 Sidebar y Navegación

![Sidebar](images/10-sidebar.png)

- **6 items**: Projects, Dashboard, Issues, Metrics, Quality Gate, Configuration
- **Responsive**: En móvil (< 768px), sidebar colapsa a menú hamburguesa
- **Dark Mode toggle**: Abajo del todo, con icono luna/sol

---

## 5. Gestión de proyectos

### 5.1 Flujo completo

```
1. Analizar proyecto con CogniCode
   cd my-project && cognicode-quality analyze

2. Se genera .cognicode/cognicode.db automáticamente
   my-project/
   ├── .cognicode/
   │   └── cognicode.db    ← SQLite con analysis_runs, issues, baselines
   └── src/...

3. Registrar en el dashboard
   - Abrir http://localhost:3000/projects
   - "+ Add Project" → nombre + ruta → Register

4. Visualizar datos
   - El dashboard lee analysis_runs (historial)
   - Muestra issues con seguimiento (open/fixed)
   - Tendencias desde la BBDD existente

5. Re-analizar cuando quieras
   - Dashboard → Run Analysis (desde /dashboard o /configuration)
   - O desde CLI: cognicode-quality analyze
   - Los nuevos datos aparecen automáticamente en el dashboard
```

### 5.2 API de proyectos

| Método | Endpoint | Descripción |
|--------|----------|-------------|
| `POST` | `/api/projects/register` | Registra un proyecto `{name, path}` |
| `GET` | `/api/projects` | Lista todos los proyectos registrados |
| `GET` | `/api/projects/:id/history` | Historial de `analysis_runs` (30 runs) |

**Ejemplo: Registrar desde curl**

```bash
curl -X POST http://localhost:3000/api/projects/register \
  -H "Content-Type: application/json" \
  -d '{"name": "Mi Proyecto", "path": "/ruta/absoluta/al/proyecto"}'
```

**Respuesta:**
```json
{
  "id": "/ruta/absoluta/al/proyecto",
  "name": "Mi Proyecto",
  "has_cognicode_db": true,
  "last_analysis": "2026-05-05T12:16:51.187Z",
  "total_issues": 15,
  "rating": "C",
  "debt_minutes": 300,
  "history_count": 3
}
```

---

## 6. Integración con SDD (Agent Teams)

CogniCode Dashboard se integra en el flujo **Spec-Driven Development (SDD)**
de Agent Teams como **herramienta de visualización y monitoreo continuo**.

### 6.1 Rol en el ecosistema SDD

```
┌─────────────────────────────────────────────────────────────┐
│                    Agent Teams SDD                           │
│                                                             │
│  Orchestrator ──→ sdd-explore ──→ sdd-propose               │
│       │                              │                      │
│       │    ┌─────────────────────────┘                      │
│       │    │                                                │
│       ▼    ▼                                                │
│  ┌──────────────┐    ┌──────────────────┐                  │
│  │ sdd-apply    │    │ CogniCode Quality│                  │
│  │ (implementa) │───►│ (analiza código) │                  │
│  └──────────────┘    └────────┬─────────┘                  │
│                               │                             │
│                               ▼                             │
│                    ┌──────────────────┐                     │
│                    │ .cognicode/      │                     │
│                    │ cognicode.db     │ ← SQLite            │
│                    └────────┬─────────┘                     │
│                             │                               │
│                             ▼                               │
│                    ┌──────────────────┐                     │
│                    │ DASHBOARD        │ ← Visualización     │
│                    │ (lee BBDD)       │                     │
│                    └──────────────────┘                     │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Uso en fases SDD

| Fase SDD | Uso del Dashboard | Beneficio |
|----------|-------------------|-----------|
| **sdd-explore** | Ver issues existentes antes de modificar | Saber qué código es problemático |
| **sdd-propose** | Consultar `analysis_runs` históricos | Ver tendencia de calidad del proyecto |
| **sdd-design** | Validar que el diseño no introduce nueva deuda | Baseline de métricas actuales |
| **sdd-apply** | Monitorizar tras cada cambio | Ver issues nuevos vs fixed |
| **sdd-verify** | Comparar métricas antes/después | `diff_vs_baseline` desde la BBDD |
| **sdd-archive** | Snapshot final de calidad | Documentar mejora/regresión |

### 6.3 Prompt para el Orchestrator

Cuando el orchestrator necesita verificar la calidad del código:

```
/sdd-verify mi-cambio

# Durante la verificación, el orchestrator consulta el dashboard:
POST /api/projects/register  → asegura que el proyecto está indexado
GET  /api/projects/:id/history → compara métricas antes/después del cambio
POST /api/quality-gate        → verifica si el gate pasa
```

### 6.4 Ejemplo de flujo SDD con Dashboard

```
1. Orchestrator: /sdd-new "refactor-auth-module"

2. sdd-explore: CogniCode analiza auth module
   → Resultados en .cognicode/cognicode.db
   → Dashboard muestra: 23 issues, 3 blockers, rating C

3. sdd-propose: Propone refactor con impacto estimado

4. sdd-apply: Implementa cambios en auth module
   → CogniCode re-analiza (changed_only=true)
   → Dashboard actualiza: 12 issues (-11), 0 blockers, rating B

5. sdd-verify: Verifica implementación
   → Consulta dashboard: diff_vs_baseline = -11 issues, debt -45min
   → Reporte: "Calidad mejoró de C a B, 11 issues resueltos"

6. sdd-archive: Archiva el cambio
   → Snapshot de métricas guardado en analysis_runs
```

---

## 7. Agents, Skills y Prompts

### 7.1 Skill: `cognicode-dashboard`

El dashboard está disponible como skill para Agent Teams. Se activa cuando:

- El orchestrator necesita visualizar métricas de calidad
- Se requiere monitoreo post-implementación
- Se pide "mostrar dashboard", "ver métricas", "quality gate status"

**Skill configurada en:** `.claude/skills/cognicode-dashboard/SKILL.md`

### 7.2 Prompts recomendados

**Para el Orchestrator:**

```
/sdd-verify refactor-auth

Después de verificar, consulta el dashboard para ver el diff de métricas:
- Registra el proyecto si no está indexado
- Compara los últimos 2 analysis_runs
- Reporta el delta de issues, debt y rating
```

**Para el usuario:**

```
"Muéstrame el dashboard del proyecto actual"
"¿Cuál es el quality gate status?"
"¿Cuántos blockers tiene el proyecto?"
"Compara las métricas antes y después del cambio"
"Registra este proyecto en el dashboard"
```

### 7.3 Prompts de mantenimiento

```bash
# Re-analizar un proyecto desde el dashboard
curl -X POST http://localhost:3000/api/analysis \
  -H "Content-Type: application/json" \
  -d '{"project_path": "/ruta/al/proyecto", "quick": true, "changed_only": true}'

# Ver historial de análisis
curl http://localhost:3000/api/projects/ruta%2Fal%2Fproyecto/history | jq

# Listar proyectos registrados
curl http://localhost:3000/api/projects | jq
```

---

## 8. Tests E2E

La batería de tests cubre **61 casos** en 15 áreas:

```bash
# Ejecutar todos los tests
npx playwright test --config=tests/e2e/playwright.config.js

# Solo tests de UI
npx playwright test --config=tests/e2e/playwright.config.js --grep "3a|4a|5a|6a|7a|8a|9a"

# Solo tests de API
npx playwright test --config=tests/e2e/playwright.config.js --grep "^1"

# Con reporte HTML
npx playwright show-report tests/e2e/report/html
```

### Estructura de tests

```
tests/e2e/
├── dashboard.spec.js         # 61 tests — batería completa
├── playwright.config.js      # Config: chromium, 1400x900, auto webServer
├── suite.js                  # Suite independiente (13 tests con screenshots)
├── playwright-setup.js       # Setup helper
└── report/                   # Screenshots + JSON + HTML report
```

---

## 9. Referencia API

### Endpoints

| Método | Ruta | Body | Respuesta |
|--------|------|------|-----------|
| `GET` | `/health` | — | `OK` |
| `GET` | `/` | — | `index.html` (SPA) |
| `POST` | `/api/analysis` | `{project_path, quick?, changed_only?}` | `AnalysisSummaryDto` |
| `POST` | `/api/issues` | `{project_path, severity?, category?, file_filter?, page?, page_size?}` | `IssuesResponseDto` |
| `POST` | `/api/metrics` | `{project_path}` | `ProjectMetricsDto` |
| `POST` | `/api/quality-gate` | `{project_path}` | `QualityGateResultDto` |
| `POST` | `/api/ratings` | `{project_path}` | `ProjectRatingsDto` |
| `POST` | `/api/validate-path` | `{project_path}` | `PathValidationDto` |
| `GET` | `/api/projects` | — | `ProjectListDto` |
| `POST` | `/api/projects/register` | `{name, path}` | `ProjectInfoDto` |
| `GET` | `/api/projects/:id/history` | — | `ProjectHistoryDto` |
| `GET` | `/api/config` | — | `DashboardConfigDto` |
| `POST` | `/api/config` | `DashboardConfigDto` | `()` |
| `GET` | `/api/rule-profiles` | — | `[RuleProfileDto]` |

### Códigos de estado

| Código | Significado |
|--------|-------------|
| `200` | Éxito |
| `404` | Ruta de proyecto no encontrada |
| `409` | Proyecto ya registrado (duplicado) |
| `500` | Error interno del servidor |

---

## 10. FAQ

**¿Necesito tener Tailwind instalado?**

No. El CSS es standalone (custom properties + utility classes). No requiere
Tailwind CLI ni PostCSS.

**¿Por qué el análisis tarda 60-120s?**

CogniCode analiza cada archivo del proyecto. Con `quick=true` solo se analizan
issues Blocker y Critical. Con `changed_only=true` solo archivos modificados.
La primera vez analiza todos; las siguientes son rápidas (cache de archivos).

**¿El dashboard modifica la BBDD de CogniCode?**

No. Solo LEE de `.cognicode/cognicode.db`. Las escrituras las hace
`cognicode-quality` durante el análisis.

**¿Puedo usar el dashboard con proyectos que no son Rust?**

Sí. CogniCode analiza múltiples lenguajes. El dashboard solo requiere que el
proyecto tenga `.cognicode/cognicode.db` (generado por `cognicode-quality`).

**¿Cómo añado un proyecto sin BBDD existente?**

1. Ejecuta `cognicode-quality analyze` en el proyecto primero
2. Esto genera `.cognicode/cognicode.db`
3. Luego regístralo en el dashboard

**¿El dashboard funciona sin el servidor?**

No. El frontend WASM necesita el servidor para:
- Servir los archivos estáticos (HTML, CSS, JS, WASM)
- Proporcionar la API REST
- Leer las BBDD de los proyectos

---

## 📸 Galería de screenshots

Todas las capturas están en [`docs/images/`](images/):

| Archivo | Pantalla |
|---------|----------|
| `01-dashboard.png` | Dashboard principal con ratings y métricas |
| `02-projects.png` | Lista de proyectos estilo SonarQube |
| `03-register-form.png` | Formulario de registro de proyecto |
| `04-issues.png` | Browser de issues con filtros |
| `05-metrics.png` | Dashboard de métricas con gráficos |
| `06-quality-gate.png` | Quality Gate con tabla de condiciones |
| `07-edit-conditions.png` | Modo edición de condiciones |
| `08-configuration.png` | Página de configuración |
| `09-dark-mode.png` | Dashboard en modo oscuro |
| `10-sidebar.png` | Sidebar con navegación |
| `11-issue-detail.png` | Detalle de un issue |
