# CogniCode Diagram — Roadmap

## Vision

CogniCode Diagram es un crate de diagramacion inferida y reverse engineering que genera automaticamente diagramas C4 Model, UML y de arquitectura a partir del analisis estatico del codigo fuente. Aprovecha la infraestructura existente de `cognicode-core` (CallGraph, tree-sitter, Symbol) para inferir relaciones y generar diagramas en multiples formatos sin intervencion manual.

## Objetivos Estrategicos

1. **Reverse engineering automatico** — desde codigo fuente a diagramas C4 completos
2. **Multi-formato** — Mermaid, PlantUML, Structurizr DSL, SVG, D2
3. **4 niveles C4** — Context, Container, Component, Code
4. **Layout profesional** — Sugiyama con puertos, nodos compuestos
5. **Integracion MCP** — tools consumibles por agentes AI
6. **Multi-lenguaje** — Soporte para Rust, TypeScript, Python, Go
7. **IA integrada** — Resumenes y explicaciones en lenguaje natural
8. **Dashboard web** — Visualizacion interactiva con auto-update

## Phases

### Phase 1 — Foundation + L4 Code (Semanas 1-2)
**Entregable**: Crate skeleton, tipos C4, diagrama de clases UML (L4)

- Crate `cognicode-diagram` con estructura modular
- Tipos C4: `Person`, `SoftwareSystem`, `Container`, `Component`, `CodeElement`
- Inferencia L4 Code: CallGraph → UML class diagram
- Motor de reglas UML (composicion, herencia, agregacion, dependencia)
- Render Mermaid class diagram
- MCP tool: `generate_c4_code`
- Tests unitarios e integracion

**Detalle**: [PLAN-FASE1.md](./PLAN-FASE1.md)

### Phase 2 — L3 Component + L2 Container (Semanas 3-4)
**Entregable**: Diagramas de componentes y containers

- Inferencia L3: modulos → Components, DependencyType → relationships
- Inferencia L2: Cargo.toml bins/libs → Containers, inter-crate deps → relationships
- Parseadores de config: Cargo.toml, package.json
- Render Mermaid para L2 y L3
- MCP tools: `generate_c4_components`, `generate_c4_containers`
- Tests con proyecto CogniCode como fixture

**Detalle**: [PLAN-FASE2.md](./PLAN-FASE2.md)

### Phase 3 — L1 Context + Structurizr DSL + Full C4 (Semanas 5-6)
**Entregable**: Diagrama de contexto completo + generacion Structurizr DSL

- Inferencia L1: deteccion de actores, sistemas externos desde deps
- Heuristicas de I/O (HTTP, DB, CLI, env vars)
- Generador de Structurizr DSL (.dsl)
- Vistas C4 completas: SystemContext, Container, Component, Dynamic
- Render PlantUML con macros C4
- MCP tools: `generate_c4_context`, `generate_c4_dynamic`
- Meta-tool: `reverse_engineer_c4` (pipeline completo)

**Detalle**: [PLAN-FASE3.md](./PLAN-FASE3.md)

### Phase 4 — Layout Engine + SVG Nativo (Semanas 7-8)
**Entregable**: Motor de layout propio con puertos y SVG

- Wrapper de `rust-sugiyama` con extension de puertos
- Asignacion de puertos por tipo de relacion
- Nodos compuestos/jerarquicos (compound nodes)
- Enrutado ortogonal de aristas
- Render SVG nativo con temas (reutiliza themes de `mermaid/mod.rs`)
- Cache de layouts (incremental con FileManifest)
- Optimizacion: paralelismo con rayon

**Detalle**: [PLAN-FASE4.md](./PLAN-FASE4.md)

### Phase 5 — Polish + Deploy + Extras (Semanas 9-10)
**Entregable**: Produccion-ready

- Diagramas de deployment (desde Dockerfile, docker-compose)
- ER diagrams (para proyectos con DB schemas)
- Export D2 lang
- Integracion con dashboard `cognicode-dashboard`
- Benchmarking de rendimiento
- Documentacion de API publica

**Detalle**: [PLAN-FASE5.md](./PLAN-FASE5.md)

---

### Phase 6 — Advanced Diagram Features (Semanas 11-14)
**Entregable**: Diagramas UML avanzados e inferencia multi-lenguaje

- Diagramas de secuencia (desde call graph traversal)
- State Machine diagrams (desde enums y transiciones)
- Activity diagrams (flujo de control)
- Inferencia TypeScript/JavaScript (L1-L3)
- Soporte multi-lenguaje (Rust + TypeScript + Python)
- AI Diagram Summarization (explicaciones en lenguaje natural)
- Diagram Diff & Versioning (comparar versiones)

**Detalle**: [PLAN-FASE6.md](./PLAN-FASE6.md)

---

### Phase 7 — Dashboard Integration (Semanas 15-17)
**Entregable**: Seccion de diagramas integrada en CogniCode Dashboard

- Diagram API endpoints (REST per diagram types)
- DiagramViewer component (zoom, pan, theme, export)
- Diagrams page (`/diagrams`) con generacion interactiva
- Diagram comparison view (side-by-side diff)
- Real-time updates (file watcher + WebSocket)
- Diagram sharing (URLs, embed, export SVG/PNG)
- Navigation integration (sidebar, breadcrumbs, deep linking)

**Detalle**: [PLAN-FASE7.md](./PLAN-FASE7.md)

## Milestones

| Milestone | Fase | Criterio de aceptacion |
|---|---|---|
| M1: Primer diagrama | F1 | `generate_c4_code` produce Mermaid class diagram correcto para 1 archivo Rust |
| M2: Componentes | F2 | `generate_c4_components` produce diagrama de componentes de cognicode-core |
| M3: Containers | F2 | `generate_c4_containers` produce diagrama de containers del workspace CogniCode |
| M4: Contexto | F3 | `generate_c4_context` detecta AI Agent, Developer, SQLite, OTel como actores/externals |
| M5: DSL | F3 | Structurizr DSL generado es parseable por structurizr-rs sin errores |
| M6: Full C4 | F3 | `reverse_engineer_c4` genera las 4 vistas C4 para CogniCode en <5s |
| M7: SVG Nativo | F4 | Layout con puertos produce SVG legible para diagrama de 50+ nodos |
| M8: Produccion | F5 | Todas las tools MCP funcionan con proyectos Rust, Python, Go, TypeScript |
| M9: Sequence | F6 | `generate_sequence_diagram` produce Mermaid sequence diagram valido |
| M10: State Machine | F6 | Detecta estados desde enum y genera state diagram |
| M11: Activity | F6 | Activity diagram con fork/join para paralelismo |
| M12: TypeScript | F6 | `reverse_engineer_c4` funciona con proyecto Next.js |
| M13: Multi-lang | F6 | Workspace Rust+TS genera container diagram unificado |
| M14: AI Summary | F6 | LLM genera resumen comprensible del diagrama |
| M15: Diff | F6 | `diff_diagrams` muestra changeset visual |
| M16: API | F7 | POST /api/diagrams/c4 devuelve Mermaid valido en <2s |
| M17: Viewer | F7 | DiagramViewer renderiza con zoom/pan funcional |
| M18: Page | F7 | Pagina /diagrams funcional |
| M19: Compare | F7 | Side-by-side diff con highlighting |
| M20: Live | F7 | Auto-update en <5s al cambiar codigo |
| M21: Export | F7 | Export SVG/PNG funcional |
| M22: Dashboard | F7 | Integracion completa en sidebar + navegacion |

## Dependencias entre Fases

```
F1 (Foundation) ──→ F2 (Component/Container) ──→ F3 (Context/DSL)
                                                      │
F1 ──────────────────────────────────────────→ F4 (Layout Engine)
                                                      │
                                                 F5 (Polish)
                                                      │
                                    ┌────────────────┴────────────────┐
                                    │                                 │
                              F6 (Advanced Features)           F7 (Dashboard Integration)
                                    │                                 │
                                    └──────────────┬──────────────────┘
                                                   │
                                              F7 ← F6
```

- F4 puede empezar en paralelo con F2/F3 si se trabaja solo el layout
- F6 puede empezar despues de F5 (necesita cognicode-diagram estable)
- F7 depende de F6 para los tipos de diagramas avanzados

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|---|---|---|---|
| `rust-sugiyama` no soporta puertos | Alta | Medio | Implementar capa de puertos propia sobre sus coordenadas |
| Structurizr DSL cambia de spec | Baja | Bajo | Seguir version estable, tests de regresion |
| Inferencia L1 produce falsos positivos | Medio | Medio | Sistema de confianza (confidence score) + override manual |
| Rendimiento en proyectos grandes (>10k simbolos) | Medio | Alto | Cache incremental + rayon + lazy evaluation |
| `structurizr-rs` no publica crates en crates.io | Medio | Bajo | Generar DSL como texto, render propio |

## Metricas de Exito

- **Cobertura de inferencia**: >80% de relaciones reales detectadas en F2
- **Correccion de layout**: 0 cruces de aristas en grafos aciclicos con <20 nodos
- **Velocidad**: <2s para generar diagrama C4 completo de CogniCode (~500 simbolos)
- **Formatos**: ≥3 formatos de output (Mermaid, PlantUML, SVG)
- **Adopcion**: Las tools MCP son usables por al menos 2 agentes AI diferentes

## Documentacion Relacionada

- [ARQUITECTURA.md](./ARQUITECTURA.md) — Estructura del crate, modulos, flujo de datos
- [PLAN-FASE1.md](./PLAN-FASE1.md) — Fase 1: Foundation + L4 Code
- [PLAN-FASE2.md](./PLAN-FASE2.md) — Fase 2: L3 Component + L2 Container
- [PLAN-FASE3.md](./PLAN-FASE3.md) — Fase 3: L1 Context + Structurizr DSL
- [PLAN-FASE4.md](./PLAN-FASE4.md) — Fase 4: Layout Engine + SVG Nativo
- [PLAN-FASE5.md](./PLAN-FASE5.md) — Fase 5: Polish + Deploy + Extras
- [PLAN-FASE6.md](./PLAN-FASE6.md) — Fase 6: Advanced Diagram Features
- [PLAN-FASE7.md](./PLAN-FASE7.md) — Fase 7: Dashboard Integration
- [MCP-TOOLS.md](./MCP-TOOLS.md) — Especificacion de MCP tools
- [INVESTIGACION.md](./INVESTIGACION.md) — Hallazgos, crates, referencias
- [Dashboard Plan](../../dashboard/PLAN.md) — Plan del dashboard web
- [Dashboard README](../../dashboard/README.md) — Manual de uso del dashboard
