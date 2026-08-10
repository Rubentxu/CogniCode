# ADR-029 — CallGraphProjectionPort Seam

**Estado**: ACEPTADO (seam implementado en `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs`; trait `CallGraphProjectionPort` consumido por descriptores analíticos y servicios; factory `project_call_graph()` en producción; verificado por e29-3-port-abstraction-audit v0.80.0 + 2026-08-03)
**Fecha**: 2026-08-03
**Decisores**: CogniCode Architecture Team
**Contexto**: e29-3-port-abstraction-audit, branch `feat/e29-3-port-abstraction-audit` @ `1a32063e`

---

## Resumen ejecutivo

El `CallGraphProjection` (petgraph-backed) se expone como un rasgo de dominio `CallGraphProjectionPort` para que los descriptores de analytics y servicios dependan de una abstracción en lugar del tipo concreto de infraestructura. Esto enforced la regla hexagonal: dominio y aplicación NO importan `crate::infrastructure::*`.

---

## Contexto

En el estado previo a e29-3, 14 archivos de dominio/analytics (`domain/analytics/*_descriptor.rs` y `application/services/{graph_analytics,graph_insights,impact_analysis}.rs`) importaban directamente `crate::infrastructure::graph::CallGraphProjection`. Esto viola la dirección de dependencia hexagonal: el dominio conoce a la infraestructura.

El problema se resolvió en e29-2-semantic-projection-kernel con la introducción del rasgo `CallGraphProjectionPort` en `domain/ports/`. e29-3 audita y corrige los 14 call sites para usar `&dyn CallGraphProjectionPort` en lugar del tipo concreto.

---

## Decisión

El rasgo `CallGraphProjectionPort` se declara en `crates/cognicode-core/src/domain/ports/call_graph_projection.rs` con las operaciones exactas que necesitan los 14 call sites:

- `build_adjacency()` → construye la proyección adjacency list
- `node_count()` → número de nodos
- `symbol_index()` → mapa de `symbol_id → index` (antes `id_to_index`)

La fábrica libre `project_call_graph(&CallGraph) → Arc<dyn CallGraphProjectionPort>` permanece en el módulo de infraestructura (construcción es detalle de infraestructura, no del dominio).

---

## Alternativas consideradas

1. **Trait en dominio con método de construcción**: Un `impl CallGraphProjectionPort` con un método `project(&self, graph: &CallGraph)` — pero esto rompería la object-safety (`dyn` no funciona con métodos que reciben `impl Trait`).

2. **Tipo associated en el trait**: asociar el tipo del grafo al trait — añade complejidad innecesaria sin beneficio.

3. **Mantener la dependencia directa de infraestructura**: rechazado — viola ADR-028 §Port Constraints y la regla hexagonal.

---

## Consecuencias

**Positivas**:
- Dominio y aplicación son independientes de la implementación de petgraph
- La abstracción del grafo call-graph está lista para futuros backends (LadybugGraphProjection)
- 14 call sites migran a dependencia contraída (`&dyn CallGraphProjectionPort`)

**Negativas**:
- Un nivel más de direccionamiento indirecto (`Arc::clone` en lugar de `Arc::clone` del tipo concreto — costo despreciable)
- La fábrica libre (`project_call_graph`) requiere que los callers tengan `Arc<dyn CallGraphProjectionPort>` — aceptable dado el uso en initialization path

**Mitigaciones**:
- W1 (factory-in-ports) documentado como ACCEPTABLE-WITH-DOC — la fábrica es una fn libre, no un método en el trait
- El rasgo es `Send + Sync + 'static` (object-safe)

---

## Referencias

- [ADR-028 port abstraction](./ADR-028-ladybugdb-port-abstraction-architecture.md)
- e29-3-port-abstraction-audit delta spec: `openspec/changes/e29-3-port-abstraction-audit/specs/port-layer-hexagon/spec.md`
- Spec sync: `openspec/specs/ports/spec.md`
- engram: `sddk/e29-3-port-abstraction-audit/jurisprudence-d1` (jurisprudence candidate)
