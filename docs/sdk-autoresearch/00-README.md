# CogniCode AutoResearch SDK

> **Versión**: 0.1.0 (draft) | **Fecha**: Mayo 2026 | **Autores**: CogniCode Team
>
> Framework en Rust para construir pipelines autónomos de mejora de software
> que operan sobre el ciclo de vida completo del desarrollo (SDLC), usando
> agentes de IA guiados por un harness de evaluación determinista.

---

## ¿Qué es AutoResearch SDK?

Un **kit de desarrollo de software** que permite a cualquier proyecto de software
ejecutar un bucle autónomo de auto-mejora, inspirado en el sistema
[autoresearch de Andrej Karpathy](https://github.com/karpathy/autoresearch).

El SDK implementa tres principios fundamentales:

1. **Harness de evaluación fijo (inmutable)**: Gates + Métricas componen un
   Health Score [0.0-1.0] que es la verdad absoluta del sistema. El agente
   nunca puede modificar cómo se calcula.

2. **Agente autónomo (NEVER STOP)**: Un LLM explora el código, formula hipótesis,
   implementa cambios mínimos, y decide keep/discard basándose únicamente en
   si el Health Score mejoró.

3. **Mejora del propio protocolo (SAGA + Meta)**: Cada 50 iteraciones, el sistema
   rebalancea los pesos del Health Score. Cada 200 iteraciones, analiza la
   eficiencia del protocolo y propone mejoras.

```
┌──────────────────────────────────────────────────────────┐
│                 THE KARPATHY LOOP                        │
│                                                          │
│   EVALUATE ──▶ ANALYZE ──▶ MODIFY ──▶ VALIDATE          │
│       ▲                                    │             │
│       │                                    ▼             │
│       └──── KEEP ◀── DECIDE ◀── EVALUATE ◀─┘             │
│                  DISCARD → ROLLBACK                      │
│                                                          │
│   Every 50 iters:  SAGA rebalances weights              │
│   Every 200 iters: META improves the protocol            │
└──────────────────────────────────────────────────────────┘
```

---

## Índice de Documentación

| # | Documento | Contenido |
|---|-----------|-----------|
| 00 | README (este documento) | Visión general y guía de inicio |
| 01 | [Core Concepts](01-core-concepts.md) | Abstracciones: Gates, Métricas, HealthScore, Pipeline |
| 02 | [Architecture](02-architecture.md) | Arquitectura en capas, traits, flujo de datos |
| 03 | [Gates Catalog](03-gates.md) | Catálogo de 10+ Gates deterministas por lenguaje |
| 04 | [Metrics Catalog](04-metrics.md) | Catálogo de 16+ Métricas (deterministas + LLM) |
| 05 | [Health Score](05-health-score.md) | Fórmula compuesta, pesos, SAGA rebalancing |
| 06 | [SDLC Mapping](06-sdlc-mapping.md) | Las 7 fases SDLC con autonomía por fase |
| 07 | [Multi-Agent Swarm](07-multi-agent-swarm.md) | Enjambre competitivo y orquestador |
| 08 | [Backlog Integration](08-backlog-integration.md) | Cómo el usuario alimenta ideas al circuito |
| 09 | [program.md Reference](09-program-md-reference.md) | Especificación del DSL de instrucciones |
| 10 | [Implementation Plan](10-implementation-plan.md) | Plan paso a paso en Rust |
| 11 | [Patterns Catalog](11-patterns-catalog.md) | Catálogo de patrones de diseño |
| 12 | [Agent Integration](12-agent-integration.md) | Integración MCP/Skills/Workflows con cualquier agente |
| — | [Diagrams](diagrams/) | Diagramas Mermaid de arquitectura y flujos |

---

## Para Quién es Este SDK

### Desarrolladores de aplicaciones
Integra el SDK en tu proyecto Rust. Configura los gates y métricas para tu
stack tecnológico. Escribe tu `program.md`. Deja que el agente trabaje de noche.

### Equipos de plataforma
Usa el SDK como base para construir un servicio de mejora continua de código.
Multi-proyecto, multi-lenguaje, multi-agente.

### Investigadores de IA
Estudia cómo los agentes autónomos exploran espacios de mejora de software.
El SDK proporciona trazabilidad completa y métricas deterministas.

---

## Instalación Rápida

```bash
# Añadir el SDK a tu proyecto Rust
cargo add cognicode-autoresearch-sdk

# O como servidor MCP standalone
cargo install cognicode-autoresearch-mcp
```

## Uso Mínimo

```rust
use cognicode_autoresearch_sdk::{
    harness::{Harness, HarnessConfig},
    pipeline::MaintenancePipeline,
};

fn main() -> anyhow::Result<()> {
    let config = HarnessConfig::for_rust_project(".")?;
    let harness = Harness::new(config)?;

    let state_before = harness.evaluate()?;
    println!("Health Score: {:.3}", state_before.health_score);

    Ok(())
}
```

## Con un Agente (vía MCP)

```bash
# Inicia el servidor MCP con herramientas de autoresearch
cognicode-mcp --features autoresearch &

# El agente carga la skill y ejecuta el bucle
opencode run --skill autoresearch-sdk \
  "Start the autonomous improvement loop. NEVER STOP."
```

---

## Principios de Diseño

1. **Determinismo**: Mismos inputs → mismo Health Score. Siempre.
2. **Separación**: El harness es inmutable. El código es mutable. Nunca se mezclan.
3. **Simplicidad**: Una mejora marginal que añade complejidad no vale la pena.
4. **Trazabilidad**: Cada experimento deja registro en git + TSV.
5. **Agnosticismo**: Funciona con cualquier agente que hable MCP.
6. **Evolución**: El propio protocolo de mejora puede mejorar.

---

## Inspiración y Créditos

- **Andrej Karpathy** — [autoresearch](https://github.com/karpathy/autoresearch):
  el bucle autónomo original para investigación de LLMs
- **SAGA** — Agente Auto-Evolutivo con arquitectura de dos niveles
- **Deep Researcher Agent** — Validación de viabilidad económica (500+ experimentos, $0.08/día)
- **autoautoresearch** — El concepto de meta-agente que mejora al agente
- **SonarQube / SQALE** — Modelo de Technical Debt Ratio y calidad de código
- **CogniCode** — Las 30 herramientas MCP de análisis de código que alimentan el SDK

---

## Estado del Proyecto

- [x] Investigación y diseño conceptual
- [x] Arquitectura del SDK documentada
- [x] Catálogo de Gates y Métricas definido
- [x] Integración MCP/Skills/Workflows diseñada
- [ ] Implementación Fase 1: SDK Core (traits + gates básicos)
- [ ] Implementación Fase 2: Métricas avanzadas (SOLID, connascence, smells)
- [ ] Implementación Fase 3: SDLC Pipelines
- [ ] Implementación Fase 4: Multi-agente y Meta-agente

---

> *"La investigación es ahora enteramente el dominio de enjambres autónomos de
> agentes de IA. Este repositorio es la historia de cómo todo empezó."*
> — @karpathy, Marzo 2026
