# ADR-K004 — Proveedores Kotlin optativos S2/S3 e inteligencia agent-first

**Estado:** PROPOSED. **Aplica a:** K3–K4. **Relacionados:** `docs/adr/ADR-016-renderer-neutral-semantic-projections.md`, `docs/adr/ADR-018-evidence-gated-product-operability.md`, `docs/adr/ADR-034-cognicode-distribution-package.md`.

## Contexto

`PrecisionTier` ya define S0 (Tree-sitter), S1 (local resolver), S2 (LSP) y reserva S3 (SCIP/LSIF), S4 (compiler IR). `CompositeProvider`, `TieredOutcome` y `ProviderDiagnostic` aportan fallbacks y provenance. No hay configuración Kotlin en `Language::lsp_server_binary` del baseline. El catálogo público anuncia herramientas que requieren grafo; algunas no requieren grafo. El agente puede perder tiempo construyéndolo sin pregunta concreta.

## Decisión propuesta

- Añadir Kotlin **como descriptor/probe** en proveedor LSP S2 existente, condicionado a handshake y capabilities por operación. Proveedor opcional, sin JDK/Gradle obligatorios, con cancelación y timeouts. Contador de tier real y motivos de degradación en outputs públicos.
- Importador SCIP S3 **condicional** al resultado medido de la UAT K3. Si no supera la precisión ofrecida, `NOT_TRIGGERED`: nunca sustituir un proveedor útil por índice costoso de producir.
- Proyección de evidencia `Question → Symbols/Edges → Provenance → Coverage → FollowUps` compartida entre CLI/MCP, sin duplicar routers semánticos ni crear un daemon propio. La herramienta seleccionada depende de la pregunta y sus prerequisitos reales.
- No usar documentación, search_content o read_file(raw) para rellenar aristas cuando falla LSP/SCIP. `Unresolved` es éxito honesto de capacidad parcial, no error que se maquilla.

## Alternativas

Implementar PSI/K2 en Rust propio: gran coste y acoplamiento. Descargar LSP automáticamente al arrancar: inseguro, eleva latencia y rompe offline. Introducir nuevo store/ontología de inteligencia: duplicidad sin ganancia. Poner todo en MCP handler: viola la dirección hexagonal.

## Aceptación

Un caso LSP S2 real y otro con servidor ausente que cae limpiamente a S0; si se habilita S3, resolver ambigüedad adicional y rechazar índice stale. Una AAT con cliente real MCP demuestra elección de consultas estructuradas sin lectura raw y un diagrama que no eleva `calls` a proceso. La decisión de S3 debe sustentarse en consumición real antes de iniciar código.
