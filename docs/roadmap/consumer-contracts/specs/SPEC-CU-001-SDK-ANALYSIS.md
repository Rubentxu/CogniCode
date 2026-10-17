# SPEC-CU-001 — API pública estable de análisis para consumidores

**Estado:** PROPOSED · **Owner:** `cognicode-core` · **Bloque:** CU-1 · **ADR:** `ADR-CU-001`

## Capacidad del consumidor

Cualquier consumer Rust debe solicitar análisis en un workspace y obtener información suficiente para decidir qué hacer, sin importar `infrastructure`, `mcp::handlers` ni reconstruir un grafo diferente. Mantener retrocompatibilidad con `AnalysisBasis`, `Analyze`, `AnalysisOp`, `AnalysisOutcome<T>`, `WorkspaceSession` y `CallGraphProjectionPort` existentes. La implementación mínima favorece una fachada *en la crate actual*; no una crate SDK nueva por defecto.

## Contratos normativos

- **CU-001.1** Consulta inmutable: `request: { workspace, scope, profile, capabilities, budget, cancellation }` con smart constructors que validan y normalizan. `scope` y opciones viven como ADTs, no strings libres.
- **CU-001.2** Resultado: `Complete(value, provenance) | Partial(value, nonempty_gaps, provenance) | Unknown(reason) | Unsupported(capability, reason) | Failed(error)` con `coverage` explícita y sin `unwrap_or_default()` ocultando errores. Reusar `AnalysisOutcome` upstream; **no duplicar ese ADT**. Si su representación permite `Partial(empty)` en deserialización, validator lo rechaza.
- **CU-001.3** La identidad de símbolo preserva nombre cualificado, file path, rango de bytes y precisión según `interface/symbol_identity.rs`. Dos símbolos homónimos son distintos.
- **CU-001.4** Vista de grafo usa `CallGraphProjectionPort` y `CallGraph` únicos. No recalcular `CallGraphAnalyzer` por símbolo cuando la métrica es global. Distinguir `GraphMetric` de `SymbolMetric`.
- **CU-001.5** `Capabilities` se resuelven desde el producto construido y sus Cargo features. Un proveedor que no puede crear un grafo debe devolver `Unsupported` o `Partial` con gaps, nunca `Complete(empty_graph)`.
- **CU-001.6** El consumidor puede identificar versión **del proveedor** (`cognicode-core` + feature set + schema) por metadatos originados en build real, no `CARGO_PKG_VERSION` del consumer.
- **CU-001.7** No romper `assurance-evidence/v1`, ni `product/tools.json`, ni versiones publicadas de CLI/MCP sin bump + compatibilidad N−1.

## Boundary propuesto (ilustrativo, no código para copiar sin inspección)

```rust
// Preferir los tipos existentes; no introducir un segundo AnalysisOutcome.
trait ConsumerAnalysis: Send + Sync {
    fn analyze(&self, basis: &AnalysisBasis, request: ConsumerRequest)
        -> AnalysisOutcome<ConsumerView>;
}
// La implementación I/O adapta AnalysisService; los presentadores solo traducen DTOs.
```

## Casos de aceptación

- Proyecto Rust con 2 archivos y 2 funciones homónimas: identidades diferentes y ubicación correcta.
- Proyecto sin backend grafo para la capability solicitada: `Unsupported/Partial`; no `Complete`.
- Graph snapshot idéntico para los 3 canales con la misma base, configuración y revisión.
- Consumidor Guard obtiene `CallGraph` y complejidad sin dependencias `interface::mcp`.

**Prueba permanente mínima:** constructor/ley de resultado (`Partial` no vacío) y contract test sobre el adapter público; E2E funcional real es temporal CU-1. 

**Upstream:** `crates/cognicode-core/src/application/analysis_basis.rs`, `src/domain/ports/call_graph_projection.rs`, `src/interface/symbol_identity.rs`; `odd/tasks/b2-shared-analysis-capability-contracts.md`, `docs/adr/ADR-029-callgraph-projection-port-seam.md`.
