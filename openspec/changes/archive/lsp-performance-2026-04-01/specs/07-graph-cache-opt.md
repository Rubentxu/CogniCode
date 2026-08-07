# Spec: GraphCache — Reduce Clone Overhead

## Requirements
- Evaluar si `RwLock<CallGraph>` es mejor que `ArcSwap<CallGraph>` para el patrón de uso actual
- Si se mantiene ArcSwap: al menos hacer batch updates (acumular eventos y aplicar en batch)
- `apply_events()` debe evitar clone cuando sea posible

## Scenarios
- apply_events con 1 evento: no clona todo el grafo
- apply_events con 100 eventos: clona una sola vez, no 100

## Constraints
- Thread safety mantenido
- Tests existentes pasan
