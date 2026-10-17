# ADR-K001 — Universo de escaneo y cobertura por alcance

**Estado:** PROPOSED. **Aplica a:** K0. **No sustituye:** `docs/prf/specs/SPEC-ANALYSIS.md`, `docs/adr/ADR-029-callgraph-projection-port-seam.md`, `docs/roadmap/ROADMAP.md`.

## Contexto y evidencia del código

`PerFileStrategy::build_full_graph_report` usa un filtro literal de extensiones `rs|py|js|ts`; otras estrategias consultan `Language::from_extension` y parte del full graph sin reporte descarta errores mediante `continue`. Existen `BuildStatus::Complete/Partial` y `SkipReason`, pero la primera versión del reporte puede parecer completa respecto de su *lista filtrada*, no del universo del proyecto. En Kotlin, esto es especialmente peligroso: archivos `.kt/.kts` son invisibles antes de K1.

## Decisión propuesta

1. Una única selección pura de archivos, con un `ScanScope` explícito y `FileDecision` tipado. Adaptadores de descubrimiento devuelven **todos** los caminos relevantes y registran los rechazados. El parser no puede determinar por sí solo que no existen fuentes relevantes.
2. El agregador de `CoverageReport` comprueba inventario exhaustivo + completitud de elegibles, y distingue `CompleteForScope` de cobertura total del checkout. Los DTO públicos siguen el tipo de outcome vigente; las variantes internas no constituyen otro `AnalysisOutcome`.
3. Full/per_file/on-demand consumen el mismo clasificador, no un nuevo registry. Desacoplar elección de ficheros de construcción de grafo y persistencia. `SkipReason` conserva causa y provenance.
4. Un fichero no soportado queda explícito como excluido; no genera aristas ni se devuelve como parser válido. Si el consumidor reclama arquitectura completa, la falta se propaga hasta su consulta.

## Alternativas rechazadas

- Añadir `kt` a la lista de cuatro extensiones: corrige el caso aparente, mantiene el doble origen y oculta lenguajes adicionales.
- Convertir todo `Unsupported` en error fatal: impediría valor parcial y navegación Rust en repositorios mixtos.
- Reescribir en otro graph store: trabajo sin beneficio de consumidor.

## Consecuencias y aceptación

Cambio acotado en seams de selección/coverage, con riesgo sobre tests que asumían `Complete`; clasificar tests obsoletos antes de modificar producto. Probar un `.kt` omitido y `full/per_file` diferenciales; salida CLI/MCP reporta la omisión sin falsa completitud. ADR se acepta en K0 por review de código y usuario autorizado; hasta entonces `PROPOSED`.
