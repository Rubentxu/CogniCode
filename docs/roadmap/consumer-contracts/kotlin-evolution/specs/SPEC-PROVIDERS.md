# SPEC-K-PROV — Proveedores S0/S1/S2/S3 y degradación (K3–K4)

## Contrato general

**No reinventar** `PrecisionTier` (S0 Tree-sitter, S1 local resolver, S2 LSP, S3 SCIP reservado, S4 compiler IR reservado), `TieredOutcome`, `CompositeProvider`, `ProviderDiagnostic` y `AnalysisOutcome`. No introducir otro `EvidenceStore`, otra tabla de estados de outcome ni un nuevo motor de grafos.

- **K-PROV-01 MUST:** para cada `(language,operation)` producir `Advertised`, `ProbedUsable`, `Degraded(reason)`, `Unavailable`, `Unsupported(reason)`; no inferir soporte de una herramienta a partir de otra del mismo lenguaje.
- **K-PROV-02 MUST:** priorizar proveedor más preciso *que sirva el contrato requerido*, con fallback solo entre operaciones estructuradas, sin búsqueda de contenido ni lectura manual del repo. Cada resultado reporta tier real, provider, omisiones, warnings, precision y basis.
- **K-PROV-03 MUST:** Kotlin LSP es optativo; sin servidor externo el S0 sigue funcionando. No descargar herramientas, ejecutar Gradle ni dar permisos de red desde el core sin consentimiento.
- **K-PROV-04 MUST:** convertir posiciones de Tree-sitter byte offset / row en URIs y posiciones UTF-16 LSP sin errores de Unicode; inicialización, documento abierto/cerrado, refresh, cancelación y proceso colgado con límites.
- **K-PROV-05 MUST:** comprobar capability handshake (`definition`, `references`, `typeHierarchy`, `callHierarchy`) antes de anunciarla. Si un LSP alfa no devuelve call hierarchy, no promocionar esa operación como supported.
- **K-PROV-06 MUST:** con S3 habilitado, validar `index.scip` (formato, language, prefix/root, digest, version, revision, path escapes, duplicados, símbolos foráneos y orden determinista). No usar índices de otro commit como actuales ni degradar origen/precisión al reconciliar.
- **K-PROV-07 MUST:** predecibilidad: mismo corpus + misma revisión + mismo índice ⇒ misma identidad/relaciones; si hay proveedor divergente, registrar conflicto no sobreescribir silenciosamente.
- **K-PROV-08 MUST:** configuraciones del servidor Kotlin y de SCIP externas al checkout salvo decisión explicita; soporte no requiere cambios en `gradle.properties`/IDE de usuario.

## Selección de implementación

Primero K3: contrastar soporte real del servidor oficial JetBrains Kotlin LSP contra el runtime. En K4: comparar importación `scip-java` con corpus real y solo habilitar S3 cuando un consumidor gane precisión; no basarse en soporte hipotético. El soporte KMP/`expect/actual` y compiler IR quedan explícitamente FUERA hasta que la matriz y el proveedor puedan certificarlo.

## Cierre por consumidor

Prueba con cliente real de CLI/MCP: 1 consulta con LSP `S2` disponible; 1 con LSP ausente y AST devolviendo símbolos; 1 de conflicto de resolución `Ambiguous`; para SCIP, 1 índice correcto y 1 versión/raíz errónea rechazada. Todos en entorno efímero, resultados con estados y provenance.
