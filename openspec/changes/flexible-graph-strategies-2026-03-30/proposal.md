# Proposal: Flexible Graph Construction Strategies

## Intent

Implementar múltiples estrategias de construcción de grafo para optimización de rendimiento y flexibilidad según el caso de uso. Permitir desde consultas instantáneas sobre índice liviano hasta análisis profundo con grafo completo.

## Scope

### In Scope
1. **Lightweight Index** - Índice rápido símbolo→ubicaciones (sin edges)
2. **On-Demand Graph** - Grafo construido bajo demanda por consulta
3. **Per-File Graph** - Grafo local por archivo, mergeable
4. **Full Project Graph** - El actual (opcional, para backward compatibility)

### Out of Scope
- Persistencia en disco del índice
- Cache distribuido

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   GraphStrategy Trait                     │
├─────────────────────────────────────────────────────────┤
│ + build_index()          → LightweightIndex             │
│ + query_symbols()       → Vec<SymbolLocation>          │
│ + build_local_graph()   → CallGraph (single file)      │
│ + build_subgraph()      → CallGraph (scope query)      │
│ + build_full_graph()    → CallGraph (full project)     │
└─────────────────────────────────────────────────────────┘
           │              │              │           │
           ▼              ▼              ▼           ▼
    ┌──────────┐  ┌───────────┐  ┌──────────┐  ┌──────────┐
    │ LightIdx │  │OnDemand  │  │ PerFile  │  │  Full    │
    │ (fast)   │  │ (lazy)   │  │ (merge)  │  │ (cached) │
    └──────────┘  └───────────┘  └──────────┘  └──────────┘
```

## Benchmark Results

### Lightweight Index (symbol-only, no edges)
| Metric | Value |
|---------|-------|
| Build time | ~2.0s |
| Total symbols | 1996 |
| Total locations | 5659 |
| Find query | <1ms |

### On-Demand Graph (lazy, per-query)
| Query | First Call | Cached |
|-------|------------|--------|
| build_subgraph depth=2 | 2752ms | 38ms |
| build_subgraph callers | - | 38ms |

**Key insight**: Index se construye una vez, queries posteriores ~72x más rápidas.

## New Components

| Component | File | Purpose |
|-----------|------|---------|
| `LightweightIndex` | `lightweight_index.rs` | Índice rápido símbolo→ubicaciones |
| `SymbolIndex` | `symbol_index.rs` | Service wrapper con cache |
| `OnDemandGraphBuilder` | `on_demand_graph.rs` | Construcción lazy de grafos |
| `PerFileGraphCache` | `per_file_graph.rs` | Cache de grafos por archivo |
| `GraphStrategy` trait | `strategy.rs` | Interface unificada |

## Success Criteria

- [x] Lightweight index construido en < 3s para proyecto typical
- [x] On-demand query responde en < 100ms (cached)
- [x] Full graph disponible bajo demanda
- [x] Backward compatibility con API existente
