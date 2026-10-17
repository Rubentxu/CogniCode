# SPEC-K-REL — Relaciones Kotlin/Java y semántica del grafo (K2)

## Ontología y política de precisión

- **K-REL-01 MUST:** distinguir declaración, referencia, llamada observada, destino resuelto, herencia, implementación, extensión y tipo; no inferir `RuntimeMessage` de `StaticCalls`.
- **K-REL-02 MUST:** cada arista almacena `CallSite` (ruta/snapshot/span), `from SymbolIdentity`, `to TargetResolution`, `kind`, `provider`, `precisionTier`, `confidence`/incertidumbre y `reason` si no pudo resolverse; preferir campos de evidencia existentes, no duplicar modelos.
- **K-REL-03 MUST:** función pura de resolución considera scope, imports, propietario, tipo de receptor, arity/firma, overloads y visibilidad cuando hay datos. Ante varios destinos posibles: `Ambiguous`, no primera coincidencia ni heurística de proximidad de archivo como evidencia semántica.
- **K-REL-04 MUST:** `get_implementors` no confunde clases Kotlin, interfaces Java, delegación `by`, `object` y `companion`; si falta información devuelve `Partial/Unknown`.
- **K-REL-05 MUST:** `suspend`/coroutines/Flow no prueban runtime ordering, transporte de red, procesos ni scheduler; negar una proyección temporal a partir del callgraph.
- **K-REL-06 MUST:** resolver llamadas Kotlin↔Java sobre identidad compartida o diagnóstico explícito de no resolución; imports wildcard/top-level Kotlin a JVM `*Kt` no permiten enlazar sin prueba.
- **K-REL-07 MUST:** el mismo conjunto de hechos conduce a grafo idéntico full/per_file/on_demand a igual versión/alcance; cualquier pérdida de relaciones debe aparecer en cobertura.

## ADTs sugeridas (Rust)

```rust
enum TargetResolution { Resolved(SymbolIdentity), Ambiguous(Vec<SymbolIdentity>), External(ExternalRef), Unresolved(Reason) }
enum RelationKind { Calls, Constructs, Extends, Implements, References, Overrides, Imports }
struct RelationObservation { source: SymbolIdentity, site: SourceSpan, target: TargetResolution, kind: RelationKind, evidence: EvidenceRef }
fn resolve(c: &CallObservation, symbols: &SymbolTable, imports: &ImportIndex, types: &TypeFacts) -> TargetResolution;
```

Sustituir por ADTs actuales si equivalentes. `resolve` no hace IO, LSP, graph writes, ni llamadas MCP; los adaptadores aportan hechos. La proyección gráfica solo acepta `Resolved` respaldadas y presenta las otras como incertidumbre consultable.

## Mini-corpus del hito

Un paquete Kotlin `a.Service.run()` y otro `b.Service.run()`, una llamada con receiver concreto, una con tipo desconocido, overloads `fun save(Int)`/`fun save(String)`, extension function, Java `JHandler.invoke()` llamada desde Kotlin y viceversa, coroutine y `Flow`. El oráculo de destinos exactos se define a mano **antes** del algoritmo; no se calcula desde CogniCode.

**Prueba negativa obligatoria:** si el algoritmo enlaza el homónimo equivocado, debe fallar. La prueba positiva confirma al menos una arista Kotlin↔Java; MCP y CLI devuelven igual ontología.
