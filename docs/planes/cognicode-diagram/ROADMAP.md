# CogniCode Diagram — Roadmap

## Vision

CogniCode Diagram es un crate de diagramacion inferida y reverse engineering que genera automaticamente diagramas C4 Model, UML y de arquitectura a partir del analisis estatico del codigo fuente. Aprovecha la infraestructura existente de `cognicode-core` (CallGraph, tree-sitter, Symbol) para inferir relaciones y generar diagramas en multiples formatos sin intervencion manual.

## Objetivos Estrategicos

1. **Reverse engineering automatico** — desde codigo fuente a diagramas C4 completos
2. **Multi-formato** — Mermaid, PlantUML, Structurizr DSL, SVG, D2
3. **4 niveles C4** — Context, Container, Component, Code
4. **Layout profesional** — Sugiyama con puertos, nodos compuestos
5. **Integracion MCP** — tools consumibles por agentes AI

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
- Diagramas de secuencia (desde call graph traversal)
- ER diagrams (para proyectos con DB schemas)
- Export D2 lang
- Integracion con dashboard `cognicode-dashboard`
- Benchmarking de rendimiento
- Documentacion de API publica

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

## Dependencias entre Fases

```
F1 (Foundation) ──→ F2 (Component/Container) ──→ F3 (Context/DSL)
                                                      │
F1 ──────────────────────────────────────────→ F4 (Layout Engine)
                                                      │
                                                 F5 (Polish)
```

F4 puede empezar en paralelo con F2/F3 si se trabaja solo el layout. F3 requiere F2 para las vistas completas.

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
- [MCP-TOOLS.md](./MCP-TOOLS.md) — Especificacion de MCP tools
- [INVESTIGACION.md](./INVESTIGACION.md) — Hallazgos, crates, referencias
