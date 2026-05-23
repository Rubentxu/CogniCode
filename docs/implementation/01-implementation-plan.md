# Plan de Implementación — CogniCode Studio

Plan de implementación por fases para migrar desde el dashboard actual (Leptos + Axum monolítico)
hacia CogniCode Studio (React + Axum BFF + enfoque híbrido).

## Arquitectura objetivo

```
cognicode-studio-web (React SPA)
        ↕ REST/SSE/WebSocket
cognicode-studio-bff (Axum BFF)
        ↕ stdio / internal calls
cognicode-mcp | cognicode-signals-mcp | cognicode-research-mcp
        ↕
cognicode-db (SQLite)
```

## Dependencias entre fases

```
Fase 1 ──→ Fase 2 ──→ Fase 3 ──→ Fase 4 ──→ Fase 5
  │           │           │           │           │
  ▼           ▼           ▼           ▼           ▼
Extract      Create      Migrate     Add         Deprecate
cognicode-   Studio     Visual      Research    cognicode-
visual       BFF        Models      Features    dashboard
```

---

## Fase 1: Extraer cognicode-visual

**Objetivo**: Crear el crate `cognicode-visual` con los modelos visuales computables.

### Tasks

- [ ] Crear `crates/cognicode-visual/Cargo.toml`
- [ ] Definir enum `VisualModel` (Graph, Matrix, Timeline, Board, Canvas, Tree, Flow)
- [ ] Definir struct `VisualResponse<T>` con data, visual, visual_level, actions
- [ ] Implementar `GraphModel` con nodos y edges
- [ ] Implementar `MatrixModel` con rows/cols/cells
- [ ] Implementar `TimelineModel` con eventos
- [ ] Implementar `BoardModel` con columns/cards
- [ ] Implementar `VisualAction` con capability-aware scopes
- [ ] Crear layout algorithms básicos (force-directed para grafos)
- [ ] Añadir al workspace `Cargo.toml`

### Criteria de	done
- `cognicode-visual` compila
- Modelos son serializables a JSON
- BFF puede importar y usar los modelos

### Dependencias
- Ninguna (greenfield)

---

## Fase 2: Crear cognicode-studio-bff

**Objetivo**: Extraer el servidor Axum del dashboard actual a un crate separado.

### Tasks

- [ ] Crear `crates/cognicode-studio-bff/Cargo.toml`
- [ ] Copiar lógica de servidor de `cognicode-dashboard/src/server.rs`
- [ ] Extraer handlers a módulos separados (`handlers/`)
- [ ] Implementar `visual.rs` que usa `cognicode-visual`
- [ ] Implementar `/api/visual/*` endpoints que computan VisualModels
- [ ] Implementar SSE handler para streaming de progreso
- [ ] Implementar WebSocket handler para live updates
- [ ] Implementar `/api/capabilities` (service discovery)
- [ ] Configuración declarativa en `config.rs`
- [ ] Mantener compatibilidad con el cliente existente del dashboard

### Criteria de	done
- BFF levanta y sirve endpoints REST/SSE/WS
- `/api/visual/graph` devuelve GraphModel serializado
- `/api/capabilities` devuelve capacidades disponibles

### Dependencias
- Fase 1 completada (necesita `cognicode-visual`)

---

## Fase 3: Migrar UI — Scaffold React

**Objetivo**: Crear el scaffold de `cognicode-studio-web` con React + Vite + Tailwind.

### Tasks

- [ ] Crear proyecto Vite + React + TypeScript
- [ ] Configurar Tailwind CSS v4 con `@tailwindcss/vite`
- [ ] Instalar shadcn/ui y configurar componentes base
- [ ] Configurar Phosphor Icons
- [ ] Crear estructura de directorios (`components/ui`, `components/visual`, `pages`, `stores`)
- [ ] Implementar layout base (Sidebar, Header, Main content area)
- [ ] Configurar React Router con lazy loading
- [ ] Implementar theme provider con dark mode
- [ ] Crear `api.ts` con cliente Axios + React Query setup
- [ ] Implementar stores Zustand por dominio

### Criteria de	done
- App levanta en `http://localhost:5173`
- Dark theme aplicado por defecto
- Sidebar con navegación funcional
- shadcn/ui components disponibles

### Dependencias
- Fase 2 completada (BFF debe estar corriendo para probar)

---

## Fase 4: Migrar UI — Componentes de Visual Thinking

**Objetivo**: Implementar los componentes visuales que renderizan VisualModels.

### Tasks

- [ ] Crear `GraphCanvas.tsx` (wrapper de React Flow)
- [ ] Crear `Timeline.tsx` (wrapper de timeline)
- [ ] Crear `Board.tsx` (wrapper de Kanban con dnd-kit)
- [ ] Crear `MatrixChart.tsx` (wrapper de Nivo)
- [ ] Crear `FreeCanvas.tsx` (wrapper de Konva)
- [ ] Crear `CausalChain.tsx` (grafo dirigido para cadenas causales)
- [ ] Implementar `visual-adapters.ts` que transforma JSON → componentes
- [ ] Implementar Signal List con severity badges
- [ ] Implementar Research Board con estado cards
- [ ] Implementar Metric Cards con sparklines

### Criteria de	done
- GraphCanvas renderiza un grafo interactivo
- Board permite drag-and-drop de cards
- Signal List muestra severity con colores correctos
- VisualModels del BFF se renderizan correctamente

### Dependencias
- Fase 3 completada
- Fase 2 completada (para probar con BFF real)

---

## Fase 5: Migrar UI — Pages

**Objetivo**: Migrar las páginas existentes del dashboard a React.

### Tasks

- [ ] Migrar Dashboard page → Explorer overview
- [ ] Migrar Projects page → Project switcher
- [ ] Migrar Issues page → Signals list
- [ ] Migrar Metrics page → Metrics dashboard
- [ ] Migrar Quality Gate → Capability status
- [ ] Implementar nuevas pages: Research, Evidence Graph, Validation Queue
- [ ] Conectar todas las pages al BFF

### Criteria de	done
- Pages existentes funcionan en React
- APIs del BFF consumidas correctamente
- Navegación entre pages funcional

### Dependencias
- Fase 4 completada

---

## Fase 6: Migrar funcionalidad de dominio

**Objetivo**: Migrar la lógica de dominio a los nuevos crates.

### Tasks

- [ ] Extraer `cognicode-signals` desde `cognicode-quality`
- [ ] Extraer `cognicode-evidence` (nuevo crate)
- [ ] Extraer `cognicode-research` desde `cognicode-core`
- [ ] Crear MCPs separados: `cognicode-signals-mcp`, `cognicode-research-mcp`
- [ ] Implementar migración de schema de BBDD
- [ ] Actualizar BFF para mediar los MCPs separados

### Criteria de	done
- MCPs funcionan como binarios separados
- BDD tiene schema para signals, evidence, research
- BFF media correctamente entre servicios

### Dependencias
- Fase 5 completada
- Decisiones de arquitectura de dominio implementadas

---

## Fase 7: Deprecar cognicode-dashboard

**Objetivo**: Retirar el dashboard antiguo una vez Studio esté completo.

### Tasks

- [ ] Verificar que todas las features del dashboard están en Studio
- [ ] Actualizar documentación y README
- [ ] Archivar/remover `cognicode-dashboard`
- [ ] Limpiar imports y referencias
- [ ] Actualizar workspace Cargo.toml

### Criteria de	done
- `cognicode-dashboard` eliminado o deprecado
- Studio tiene todas las features del dashboard
- Documentación actualizada

### Dependencias
- Fase 6 completada

---

## Roadmap visual

```
Mes 1-2: Fase 1 (cognicode-visual)
Mes 2-3: Fase 2 (Studio BFF)
Mes 3-4: Fase 3 (Scaffold React) + Fase 4 (Visual components)
Mes 4-5: Fase 5 (Pages migration)
Mes 5-6: Fase 6 (Domain extraction)
Mes 6+:   Fase 7 (Deprecation)
```

## Criteria de entrada a cada fase

| Fase | Criteria de entrada |
|------|---------------------|
| 1 | Ninguno |
| 2 | Fase 1 completada |
| 3 | Fase 2 funcional (BFF sirve endpoints básicos) |
| 4 | Fase 3 completada + BFF corriendo |
| 5 | Fase 4 completada |
| 6 | Fase 5 completada |
| 7 | Todas las fases anteriores completadas |

## Risks y mitigaciones

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Migration de Leptos → React toma más tiempo | Alto | Medio | Mantener dashboard funcionando hasta que Studio esté completo |
| Visual models son más complejos de renderizar | Medio | Medio | Empezar con GraphCanvas simple, iterar |
| BFF se convierte en bottleneck | Medio | Bajo | Diseñar BFF stateless donde sea posible |
| Schema migrations rompen data existente | Alto | Bajo | Backup de BDD antes de migraciones, testing exhaustivo |

## Metrics de éxito

- [ ] Studio levanta sin errores
- [ ] BFF responde en < 100ms para endpoints básicos
- [ ] GraphCanvas renderiza 100+ nodos sin lag
- [ ] Signal list muestra 1000+ items con virtualización
- [ ] Dark theme consistente en todas las pages
- [ ] Lighthouse score > 80 para Studio web
